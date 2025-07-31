use axum::{
    extract::{Extension, Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    files::{FileUpload, FileListQuery, FileMetadata},
    middleware::auth::AuthUser,
    models::files::{FileUploadRequest},
    validation::{ContextValidatable, middleware::extract_validation_context, SecurityValidator},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct FileUploadQuery {
    pub item_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct FileUploadResponse {
    pub id: Uuid,
    pub filename: String,
    pub original_filename: String,
    pub content_type: String,
    pub size: u64,
    pub created_at: String,
    pub item_id: Option<u64>,
}

impl From<FileMetadata> for FileUploadResponse {
    fn from(metadata: FileMetadata) -> Self {
        Self {
            id: metadata.id,
            filename: metadata.filename,
            original_filename: metadata.original_filename,
            content_type: metadata.content_type,
            size: metadata.size,
            created_at: metadata.created_at.to_rfc3339(),
            item_id: metadata.item_id,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FileListResponse {
    pub files: Vec<FileUploadResponse>,
    pub total: u64,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

pub async fn upload_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    auth_user: Option<Extension<AuthUser>>,
    Query(query): Query<FileUploadQuery>,
    mut multipart: Multipart,
) -> Result<Json<FileUploadResponse>> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let mut file_upload: Option<FileUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        AppError::BadRequest(format!("Failed to read multipart field: {}", e))
    })? {
        let name = field.name().unwrap_or("").to_string();
        
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::BadRequest("Missing filename".to_string()))?
                .to_string();

            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            let data = field.bytes().await.map_err(|e| {
                AppError::BadRequest(format!("Failed to read file data: {}", e))
            })?;

            let file_request = FileUploadRequest {
                filename: filename.clone(),
                content_type: content_type.clone(),
                size: data.len() as u64,
                description: None,
                tags: None,
            };

            let user_id = auth_user.as_ref().map(|Extension(user)| user.user_id as u64);
            let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
                std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
            });
            let context = extract_validation_context(&headers, &addr, user_id, None);
            
            let validation_result = file_request.validate_with_context(&context);
            if !validation_result.is_valid {
                return Err(AppError::FileValidation(format!(
                    "File validation failed: {}",
                    serde_json::to_string(&validation_result.errors).unwrap_or_default()
                )));
            }

            let security_result = SecurityValidator::validate_file_upload_security(
                &filename,
                &content_type,
                &data,
            );
            
            if !security_result.is_valid {
                return Err(AppError::SecurityValidation(format!(
                    "File security validation failed: {}",
                    serde_json::to_string(&security_result.errors).unwrap_or_default()
                )));
            }

            let uploaded_by = match auth_user.as_ref() {
                Some(Extension(user)) => user.user_id as u64,
                None => 0,
            };

            file_upload = Some(FileUpload {
                original_filename: filename,
                content_type,
                data: data.to_vec(),
                uploaded_by,
                item_id: query.item_id,
            });
            break;
        }
    }

    let upload = file_upload.ok_or_else(|| {
        AppError::BadRequest("No file found in request".to_string())
    })?;

    let metadata = file_manager.store_file(upload).await?;
    
    if let Some(ws_manager) = &state.websocket_manager {
        let message = serde_json::json!({
            "type": "file_uploaded",
            "file": {
                "id": metadata.id,
                "filename": metadata.original_filename,
                "size": metadata.size,
                "item_id": metadata.item_id
            }
        });
        let event = crate::websocket::WebSocketEvent::Custom(message);
        ws_manager.broadcast(event).await;
    }

    Ok(Json(metadata.into()))
}

pub async fn serve_file(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Response> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let (metadata, data) = file_manager
        .get_file_data(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".to_string()))?;

    let mut headers = HeaderMap::new();
    
    headers.insert(
        header::CONTENT_TYPE,
        metadata.content_type.parse().unwrap_or_else(|_| {
            "application/octet-stream".parse().unwrap()
        }),
    );
    
    headers.insert(
        header::CONTENT_LENGTH,
        data.len().to_string().parse().unwrap(),
    );
    
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600".parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, data).into_response())
}

pub async fn get_file_info(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Json<FileUploadResponse>> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let metadata = file_manager
        .get_file_metadata(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".to_string()))?;

    Ok(Json(metadata.into()))
}

pub async fn download_file(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Response> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let (metadata, data) = file_manager
        .get_file_data(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".to_string()))?;

    let mut headers = HeaderMap::new();
    
    headers.insert(
        header::CONTENT_TYPE,
        metadata.content_type.parse().unwrap_or_else(|_| {
            "application/octet-stream".parse().unwrap()
        }),
    );
    
    headers.insert(
        header::CONTENT_LENGTH,
        data.len().to_string().parse().unwrap(),
    );
    
    let disposition = format!(
        "attachment; filename=\"{}\"",
        metadata.original_filename.replace('"', "\\\"")
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        disposition.parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, data).into_response())
}

pub async fn list_files(
    State(state): State<AppState>,
    Query(query): Query<FileListQuery>,
) -> Result<Json<FileListResponse>> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let files = file_manager.list_files(query.clone()).await?;
    let total = file_manager.count_files(query.clone()).await?;

    let response = FileListResponse {
        files: files.into_iter().map(|f| f.into()).collect(),
        total,
        limit: query.limit,
        offset: query.offset,
    };

    Ok(Json(response))
}

pub async fn delete_file(
    State(state): State<AppState>,
    auth_user: Option<Extension<AuthUser>>,
    Path(file_id): Path<Uuid>,
) -> Result<StatusCode> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let metadata = file_manager
        .get_file_metadata(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".to_string()))?;

    if let Some(Extension(user)) = auth_user {
        if metadata.uploaded_by != user.user_id as u64 && user.role != crate::auth::models::UserRole::Admin {
            return Err(AppError::Authorization(
                "You don't have permission to delete this file".to_string(),
            ));
        }
    } else if metadata.uploaded_by != 0 {
        return Err(AppError::Unauthorized);
    }

    file_manager.delete_file(file_id).await?;
    
    if let Some(ws_manager) = &state.websocket_manager {
        let message = serde_json::json!({
            "type": "file_deleted",
            "file_id": file_id
        });
        let event = crate::websocket::WebSocketEvent::Custom(message);
        ws_manager.broadcast(event).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AssociateFileRequest {
    pub item_id: Option<u64>,
}

pub async fn associate_file_with_item(
    State(state): State<AppState>,
    auth_user: Option<Extension<AuthUser>>,
    Path(file_id): Path<Uuid>,
    Json(req_body): Json<AssociateFileRequest>,
) -> Result<Json<serde_json::Value>> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let metadata = file_manager
        .get_file_metadata(file_id)
        .await?
        .ok_or_else(|| AppError::NotFound("File not found".to_string()))?;

    if let Some(Extension(user)) = auth_user {
        if metadata.uploaded_by != user.user_id as u64 && user.role != crate::auth::models::UserRole::Admin {
            return Err(AppError::Authorization(
                "You don't have permission to modify this file".to_string(),
            ));
        }
    } else if metadata.uploaded_by != 0 {
        return Err(AppError::Unauthorized);
    }

    if let Some(item_id) = req_body.item_id {
        let item_exists = match state.item_service.get_item(item_id).await {
            Ok(_) => true,
            Err(AppError::NotFound(_)) => false,
            Err(e) => return Err(e),
        };

        if !item_exists {
            return Err(AppError::BadRequest(
                "Item not found".to_string(),
            ));
        }
    }

    let updated_metadata = file_manager
        .associate_with_item(file_id, req_body.item_id)
        .await?;

    let response = serde_json::json!({
        "success": true,
        "data": updated_metadata,
        "message": "File associated with item successfully"
    });

    Ok(Json(response))
}

pub async fn get_item_files(
    State(state): State<AppState>,
    Path(item_id): Path<u64>,
) -> Result<Json<Vec<FileUploadResponse>>> {
    let file_manager = state
        .file_manager
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let _item = state.item_service.get_item(item_id).await?;

    let files = file_manager.get_files_by_item(item_id).await?;

    Ok(Json(files.into_iter().map(|f| f.into()).collect()))
}
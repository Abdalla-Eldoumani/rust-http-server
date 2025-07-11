//! HTTP route handlers for all standard methods

use crate::{
    error::{AppError, Result},
    models::request::{ApiResponse, FormPayload, JsonPayload},
    AppState,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_root))
        .route("/health", get(handle_health))
        .route("/api/items", get(handle_get_items).post(handle_post_item))
        .route(
            "/api/items/:id",
            get(handle_get_item)
                .put(handle_put_item)
                .delete(handle_delete_item)
                .patch(handle_patch_item),
        )
        .route("/api/form", axum::routing::post(handle_form_submit))
        .route("/api/head", axum::routing::head(handle_head))
        .route("/api/options", axum::routing::options(handle_options))
}

async fn handle_root(State(state): State<AppState>) -> impl IntoResponse {
    Json(ApiResponse::success(serde_json::json!({
        "app": state.app_name,
        "version": state.version,
        "message": "Welcome to the Rust HTTP Server"
    })))
}

async fn handle_health() -> impl IntoResponse {
    Json(ApiResponse::success(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().timestamp()
    })))
}

#[derive(Debug, Serialize, Deserialize)]
struct Item {
    id: u64,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn handle_get_items(Query(params): Query<ItemsQuery>) -> Result<impl IntoResponse> {
    info!("GET /api/items - limit: {:?}, offset: {:?}", params.limit, params.offset);
    
    let items = vec![
        Item {
            id: 1,
            name: "Item 1".to_string(),
            description: Some("First item".to_string()),
        },
        Item {
            id: 2,
            name: "Item 2".to_string(),
            description: None,
        },
    ];

    Ok(Json(ApiResponse::success(items)))
}

async fn handle_get_item(Path(id): Path<u64>) -> Result<impl IntoResponse> {
    info!("GET /api/items/{}", id);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    if id > 100 {
        return Err(AppError::NotFound(format!("Item {} not found", id)));
    }

    let item = Item {
        id,
        name: format!("Item {}", id),
        description: Some(format!("Description for item {}", id)),
    };

    Ok(Json(ApiResponse::success(item)))
}

async fn handle_post_item(Json(payload): Json<JsonPayload>) -> Result<impl IntoResponse> {
    info!("POST /api/items - payload: {:?}", payload);
    
    if payload.message.is_empty() {
        return Err(AppError::BadRequest("Message cannot be empty".to_string()));
    }

    let new_item = Item {
        id: 123,
        name: payload.message,
        description: payload.data.and_then(|d| d.as_str().map(String::from)),
    };

    Ok((StatusCode::CREATED, Json(ApiResponse::success(new_item))))
}

async fn handle_put_item(
    Path(id): Path<u64>,
    Json(payload): Json<JsonPayload>,
) -> Result<impl IntoResponse> {
    info!("PUT /api/items/{} - payload: {:?}", id, payload);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    let updated_item = Item {
        id,
        name: payload.message,
        description: payload.data.and_then(|d| d.as_str().map(String::from)),
    };

    Ok(Json(ApiResponse::success(updated_item)))
}

async fn handle_delete_item(Path(id): Path<u64>) -> Result<impl IntoResponse> {
    info!("DELETE /api/items/{}", id);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    Ok((
        StatusCode::NO_CONTENT,
        Json(ApiResponse::success(serde_json::json!({
            "deleted": id
        }))),
    ))
}

async fn handle_patch_item(
    Path(id): Path<u64>,
    Json(patch): Json<HashMap<String, serde_json::Value>>,
) -> Result<impl IntoResponse> {
    info!("PATCH /api/items/{} - patch: {:?}", id, patch);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    let patched_item = Item {
        id,
        name: patch
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Patched Item")
            .to_string(),
        description: patch.get("description").and_then(|v| v.as_str().map(String::from)),
    };

    Ok(Json(ApiResponse::success(patched_item)))
}

async fn handle_form_submit(Form(form): Form<FormPayload>) -> Result<impl IntoResponse> {
    info!("Form submission - name: {}, email: {}", form.name, form.email);
    
    if form.email.is_empty() || !form.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": "Form submitted successfully",
        "data": {
            "name": form.name,
            "email": form.email,
            "message": form.message,
            "timestamp": chrono::Utc::now().timestamp()
        }
    }))))
}

async fn handle_head() -> impl IntoResponse {
    info!("HEAD /api/head");
    
    let mut headers = HeaderMap::new();
    headers.insert("X-Custom-Header", "HEAD-Response".parse().unwrap());
    headers.insert("X-Resource-Count", "42".parse().unwrap());
    
    (StatusCode::OK, headers)
}

async fn handle_options() -> impl IntoResponse {
    info!("OPTIONS /api/options");
    
    let mut headers = HeaderMap::new();
    headers.insert("Allow", "GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS".parse().unwrap());
    headers.insert("X-Accepted-Methods", "ALL".parse().unwrap());
    
    (StatusCode::OK, headers, Json(ApiResponse::success(serde_json::json!({
        "methods": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
        "description": "Available HTTP methods for this endpoint"
    }))))
}
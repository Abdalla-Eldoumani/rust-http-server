//! HTTP route handlers for all standard methods

use crate::{
    error::{AppError, Result},
    models::request::{ApiResponse, FormPayload, JsonPayload},
    store::Item,
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
        .route("/api/stats", get(handle_stats))
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
        "message": "Welcome to the Rust HTTP Server",
        "endpoints": {
            "health": "/health",
            "stats": "/api/stats",
            "items": "/api/items",
            "item": "/api/items/{id}",
            "form": "/api/form"
        }
    })))
}

async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.store.get_stats().unwrap_or_default();
    
    Json(ApiResponse::success(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().timestamp(),
        "store_stats": stats
    })))
}

async fn handle_stats(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let stats = state.store.get_stats()?;
    Ok(Json(ApiResponse::success(stats)))
}

#[derive(Debug, Deserialize)]
struct ItemsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn handle_get_items(
    State(state): State<AppState>,
    Query(params): Query<ItemsQuery>
) -> Result<impl IntoResponse> {
    info!("GET /api/items - limit: {:?}, offset: {:?}", params.limit, params.offset);
    
    let items = state.store.get_items(params.limit, params.offset)?;
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "items": items,
        "count": items.len(),
        "limit": params.limit,
        "offset": params.offset.unwrap_or(0)
    }))))
}

async fn handle_get_item(
    State(state): State<AppState>,
    Path(id): Path<u64>
) -> Result<impl IntoResponse> {
    info!("GET /api/items/{}", id);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    let item = state.store.get_item(id)?;
    Ok(Json(ApiResponse::success(item)))
}

#[derive(Debug, Deserialize)]
struct CreateItemRequest {
    name: String,
    description: Option<String>,
    tags: Option<Vec<String>>,
    metadata: Option<serde_json::Value>,
}

async fn handle_post_item(
    State(state): State<AppState>,
    Json(payload): Json<CreateItemRequest>
) -> Result<impl IntoResponse> {
    info!("POST /api/items - name: {}", payload.name);
    
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name cannot be empty".to_string()));
    }
    
    if payload.name.len() > 100 {
        return Err(AppError::BadRequest("Name too long (max 100 characters)".to_string()));
    }

    let item = state.store.create_item(
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    )?;

    Ok((StatusCode::CREATED, Json(ApiResponse::success(item))))
}

async fn handle_put_item(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(payload): Json<CreateItemRequest>,
) -> Result<impl IntoResponse> {
    info!("PUT /api/items/{} - name: {}", id, payload.name);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }
    
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name cannot be empty".to_string()));
    }

    let item = state.store.update_item(
        id,
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    )?;

    Ok(Json(ApiResponse::success(item)))
}

async fn handle_delete_item(
    State(state): State<AppState>,
    Path(id): Path<u64>
) -> Result<impl IntoResponse> {
    info!("DELETE /api/items/{}", id);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    state.store.delete_item(id)?;
    
    Ok((
        StatusCode::NO_CONTENT,
        Json(ApiResponse::success(serde_json::json!({
            "message": "Item deleted successfully",
            "deleted_id": id
        }))),
    ))
}

async fn handle_patch_item(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Json(patch): Json<HashMap<String, serde_json::Value>>,
) -> Result<impl IntoResponse> {
    info!("PATCH /api/items/{} - updates: {:?}", id, patch);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }
    
    if patch.is_empty() {
        return Err(AppError::BadRequest("No updates provided".to_string()));
    }

    let item = state.store.patch_item(id, patch)?;
    Ok(Json(ApiResponse::success(item)))
}

async fn handle_form_submit(
    State(state): State<AppState>,
    Form(form): Form<FormPayload>
) -> Result<impl IntoResponse> {
    info!("Form submission - name: {}, email: {}", form.name, form.email);
    
    if form.email.is_empty() || !form.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }
    
    let item_name = format!("Form submission from {}", form.name);
    let metadata = serde_json::json!({
        "source": "form",
        "email": form.email,
        "message": form.message,
        "submitted_at": chrono::Utc::now().to_rfc3339()
    });
    
    let item = state.store.create_item(
        item_name,
        Some(format!("Submitted by {} ({})", form.name, form.email)),
        vec!["form-submission".to_string()],
        Some(metadata)
    )?;

    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": "Form submitted successfully",
        "created_item": item
    }))))
}

async fn handle_head(State(state): State<AppState>) -> impl IntoResponse {
    info!("HEAD /api/head");
    
    let stats = state.store.get_stats().unwrap_or_default();
    let item_count = stats.get("total_items").and_then(|v| v.as_u64()).unwrap_or(0);
    
    let mut headers = HeaderMap::new();
    headers.insert("X-Custom-Header", "HEAD-Response".parse().unwrap());
    headers.insert("X-Total-Items", item_count.to_string().parse().unwrap());
    headers.insert("X-Api-Version", state.version.parse().unwrap());
    
    (StatusCode::OK, headers)
}

async fn handle_options() -> impl IntoResponse {
    info!("OPTIONS /api/options");
    
    let mut headers = HeaderMap::new();
    headers.insert("Allow", "GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS".parse().unwrap());
    headers.insert("X-Accepted-Methods", "ALL".parse().unwrap());
    
    (StatusCode::OK, headers, Json(ApiResponse::success(serde_json::json!({
        "methods": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
        "description": "Available HTTP methods for this endpoint",
        "api_info": {
            "version": "1.0",
            "endpoints": {
                "items": {
                    "list": "GET /api/items",
                    "create": "POST /api/items",
                    "get": "GET /api/items/:id",
                    "update": "PUT /api/items/:id",
                    "patch": "PATCH /api/items/:id",
                    "delete": "DELETE /api/items/:id"
                },
                "form": "POST /api/form",
                "stats": "GET /api/stats",
                "health": "GET /health"
            }
        }
    }))))
}
//! HTTP route handlers for all standard methods

use crate::{
    error::{AppError, Result},
    handlers::files,
    models::{
        request::{ApiResponse, FormPayload},
        items::{CreateItemRequest, ItemListQuery, ItemExportQuery},
    },
    validation::{ValidationContext, ContextValidatable, middleware::extract_validation_context},
    AppState,
};
use axum::{
    extract::{Form, Path, Query, State, Request, FromRequest},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Html},
    routing::get,
    Json, Router,
    body::Body,
};
use serde::{Deserialize};
use std::collections::HashMap;
use tracing::info;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_root))
        .route("/health", get(crate::handlers::health::handle_health))
        .route("/health/:component", get(crate::handlers::health::handle_component_health))
        .route("/ready", get(crate::handlers::health::handle_readiness))
        .route("/live", get(crate::handlers::health::handle_liveness))
        .route("/dashboard", get(handle_dashboard))
        .route("/test", get(handle_test_page))
        .route("/websocket-test", get(handle_websocket_test))
        .route("/api/stats", get(handle_stats))
        .route("/api/metrics", get(crate::handlers::metrics::handle_enhanced_metrics))
        .route("/api/system/metrics", get(crate::handlers::metrics::handle_system_metrics))
        .route("/api/performance/metrics", get(crate::handlers::metrics::handle_performance_metrics))
        .route("/api/system/alerts", get(crate::handlers::metrics::handle_resource_alerts))
        .route("/api/health/history", get(crate::handlers::metrics::handle_health_history))
        .route("/api/items", get(handle_get_items).post(handle_post_item))
        .route("/api/items/search", get(handle_search_items))
        .route("/api/items/export", get(handle_export_items))
        .route(
            "/api/items/:id",
            get(handle_get_item)
                .put(handle_put_item)
                .delete(handle_delete_item)
                .patch(handle_patch_item),
        )

        // API v1
        .route("/api/v1/items", get(handle_get_items).post(handle_post_item))
        .route("/api/v1/items/search", get(handle_search_items))
        .route("/api/v1/items/export", get(handle_export_items))
        .route(
            "/api/v1/items/:id",
            get(handle_get_item)
                .put(handle_put_item)
                .delete(handle_delete_item)
                .patch(handle_patch_item),
        )

        // API v2
        .route("/api/v2/items", get(handle_get_items_v2).post(handle_post_item_v2))
        .route("/api/v2/items/search", get(handle_search_items))
        .route("/api/v2/items/export", get(handle_export_items))
        .route(
            "/api/v2/items/:id",
            get(handle_get_item_v2)
                .put(handle_put_item_v2)
                .delete(handle_delete_item)
                .patch(handle_patch_item),
        )
        .route("/api/form", axum::routing::post(handle_form_submit))
        .route("/api/head", axum::routing::head(handle_head))
        .route("/api/options", axum::routing::options(handle_options))
        .route("/ws", axum::routing::get(crate::websocket::websocket_handler))
        .nest("/auth", crate::handlers::auth::create_auth_routes_with_middleware())
        .nest("/api/files", create_file_routes())
        .nest("/api/jobs", create_job_routes())
        .nest("/api/cache", create_cache_routes())
}

async fn handle_root(State(state): State<AppState>) -> impl IntoResponse {
    let mut endpoints = serde_json::json!({
        "health": "/health",
        "stats": "/api/stats",
        "items": "/api/items",
        "search": "/api/items/search",
        "item": "/api/items/{id}",
        "form": "/api/form"
    });

    if state.file_manager.is_some() {
        endpoints["files"] = serde_json::json!({
            "upload": "/api/files/upload",
            "serve": "/api/files/{id}/serve",
            "info": "/api/files/{id}/info",
            "download": "/api/files/{id}/download",
            "delete": "/api/files/{id}",
            "list": "/api/files",
            "associate": "/api/files/{id}/associate",
            "item_files": "/api/files/item/{id}"
        });
    }

    if state.websocket_manager.is_some() {
        endpoints["websocket"] = serde_json::Value::String("/ws".to_string());
    }

    if state.auth_service.is_some() {
        endpoints["auth"] = serde_json::json!({
            "register": "/auth/register",
            "login": "/auth/login",
            "refresh": "/auth/refresh",
            "logout": "/auth/logout",
            "me": "/auth/me",
            "users": "/auth/users/{id}"
        });
    }

    if state.job_queue.is_some() {
        endpoints["jobs"] = serde_json::json!({
            "submit": "/api/jobs",
            "list": "/api/jobs",
            "stats": "/api/jobs/stats",
            "cleanup": "/api/jobs/cleanup",
            "bulk_import": "/api/jobs/bulk-import",
            "bulk_export": "/api/jobs/bulk-export",
            "get": "/api/jobs/{id}",
            "status": "/api/jobs/{id}/status",
            "cancel": "/api/jobs/{id}/cancel",
            "retry": "/api/jobs/{id}/retry"
        });
    }

    if state.cache_manager.is_some() {
        endpoints["cache"] = serde_json::json!({
            "stats": "/api/cache/stats",
            "health": "/api/cache/health",
            "clear": "/api/cache/clear",
            "invalidate": "/api/cache/invalidate"
        });
    }

    Json(ApiResponse::success(serde_json::json!({
        "app": state.app_name,
        "version": state.version,
        "message": "Welcome to the Rust HTTP Server",
        "authentication_enabled": state.auth_service.is_some(),
        "websocket_enabled": state.websocket_manager.is_some(),
        "endpoints": endpoints
    })))
}

async fn handle_stats(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let stats = state.item_service.get_stats().await?;
    Ok(Json(ApiResponse::success(stats)))
}

// Using ItemListQuery from models instead and Using a custom SearchQuery for backward compatibility with existing search functionality
#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    tags: Option<String>,
    created_after: Option<String>,
    created_before: Option<String>,
    updated_after: Option<String>,
    updated_before: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    fuzzy: Option<bool>,
    created_by: Option<i64>,
    min_relevance: Option<f64>,
    limit: Option<u64>,
    offset: Option<u64>,
}

impl ContextValidatable for SearchQuery {
    fn validate_with_context(&self, _context: &ValidationContext) -> crate::validation::ValidationResult {
        let mut result = crate::validation::ValidationResult::success();
        
        if let Some(q) = &self.q {
            if q.len() > 500 {
                result.add_error("q", "Search query is too long");
            }
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(q) {
                result.add_error("q", "Search query contains invalid characters");
            }
        }
        
        if let Some(tags) = &self.tags {
            let tag_list: Vec<&str> = tags.split(',').collect();
            if tag_list.len() > 20 {
                result.add_error("tags", "Too many tags in search");
            }
            for tag in tag_list {
                if let Err(_) = crate::validation::rules::validate_no_sql_injection(tag.trim()) {
                    result.add_error("tags", "Tag contains invalid characters");
                }
            }
        }
        
        if let Some(sort_by) = &self.sort_by {
            let allowed_fields = ["name", "created_at", "updated_at", "relevance"];
            if !allowed_fields.contains(&sort_by.as_str()) {
                result.add_error("sort_by", "Invalid sort field");
            }
        }
        
        if let Some(sort_order) = &self.sort_order {
            if !["asc", "desc"].contains(&sort_order.to_lowercase().as_str()) {
                result.add_error("sort_order", "Sort order must be 'asc' or 'desc'");
            }
        }
        
        if let Some(limit) = self.limit {
            if limit > 1000 {
                result.add_error("limit", "Limit cannot exceed 1000");
            }
        }
        
        result
    }
}

async fn handle_search_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Query(params): Query<SearchQuery>
) -> Result<impl IntoResponse> {
    info!("GET /api/items/search - query: {:?}", params);
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = params.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Search query validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    if state.search_engine.is_none() {
        let limit = params.limit.unwrap_or(50).min(100) as usize;
        let offset = params.offset.unwrap_or(0) as usize;
        
        let items = state.item_service.get_items(Some(limit), Some(offset)).await?;
        
        let filtered_items = if let Some(ref tags_str) = params.tags {
            let search_tags: Vec<String> = tags_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            
            if !search_tags.is_empty() {
                items.into_iter()
                    .filter(|item| {
                        item.tags.iter().any(|tag| search_tags.contains(tag))
                    })
                    .collect()
            } else {
                items
            }
        } else {
            items
        };
        
        return Ok(Json(ApiResponse::success(serde_json::json!({
            "items": filtered_items.iter().map(|item| serde_json::json!({
                "item": item,
                "matched_fields": ["name", "description", "tags"],
                "relevance_score": 1.0
            })).collect::<Vec<_>>(),
            "total_count": filtered_items.len(),
            "offset": offset,
            "limit": limit,
            "has_more": false,
            "query": {
                "text": params.q,
                "tags": params.tags,
                "sort_by": params.sort_by,
                "sort_order": params.sort_order,
                "fuzzy": params.fuzzy.unwrap_or(false)
            }
        }))));
    }
    
    let search_engine = state.search_engine.as_ref().unwrap();
    let mut search_query = crate::search::SearchQuery::new();
    
    if let Some(ref text) = params.q {
        if !text.trim().is_empty() {
            search_query = search_query.with_text(text.clone());
        }
    }
    
    if let Some(ref tags_str) = params.tags {
        let tags: Vec<String> = tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !tags.is_empty() {
            search_query = search_query.with_tags(tags);
        }
    }
    
    if let Some(created_after) = params.created_after {
        let start_date = chrono::DateTime::parse_from_rfc3339(&created_after)
            .map_err(|_| AppError::BadRequest("Invalid created_after date format".to_string()))?
            .with_timezone(&chrono::Utc);
        
        let end_date = if let Some(created_before) = params.created_before {
            Some(chrono::DateTime::parse_from_rfc3339(&created_before)
                .map_err(|_| AppError::BadRequest("Invalid created_before date format".to_string()))?
                .with_timezone(&chrono::Utc))
        } else {
            None
        };
        
        search_query = search_query.with_created_date_range(Some(start_date), end_date);
    } else if let Some(created_before) = params.created_before {
        let end_date = chrono::DateTime::parse_from_rfc3339(&created_before)
            .map_err(|_| AppError::BadRequest("Invalid created_before date format".to_string()))?
            .with_timezone(&chrono::Utc);
        search_query = search_query.with_created_date_range(None, Some(end_date));
    }
    
    if let Some(updated_after) = params.updated_after {
        let start_date = chrono::DateTime::parse_from_rfc3339(&updated_after)
            .map_err(|_| AppError::BadRequest("Invalid updated_after date format".to_string()))?
            .with_timezone(&chrono::Utc);
        
        let end_date = if let Some(updated_before) = params.updated_before {
            Some(chrono::DateTime::parse_from_rfc3339(&updated_before)
                .map_err(|_| AppError::BadRequest("Invalid updated_before date format".to_string()))?
                .with_timezone(&chrono::Utc))
        } else {
            None
        };
        
        search_query = search_query.with_updated_date_range(Some(start_date), end_date);
    } else if let Some(updated_before) = params.updated_before {
        let end_date = chrono::DateTime::parse_from_rfc3339(&updated_before)
            .map_err(|_| AppError::BadRequest("Invalid updated_before date format".to_string()))?
            .with_timezone(&chrono::Utc);
        search_query = search_query.with_updated_date_range(None, Some(end_date));
    }
    
    let sort_field = match params.sort_by.as_deref() {
        Some("name") => crate::search::SortField::Name,
        Some("created_at") => crate::search::SortField::CreatedAt,
        Some("updated_at") => crate::search::SortField::UpdatedAt,
        Some("relevance") => crate::search::SortField::Relevance,
        _ => crate::search::SortField::CreatedAt,
    };
    
    let sort_order = match params.sort_order.as_deref() {
        Some("asc") => crate::search::SortOrder::Asc,
        Some("desc") => crate::search::SortOrder::Desc,
        _ => crate::search::SortOrder::Desc,
    };
    
    search_query = search_query.with_sort(sort_field, sort_order);
    
    if let Some(fuzzy) = params.fuzzy {
        search_query = search_query.with_fuzzy(fuzzy);
    }
    
    if let Some(created_by) = params.created_by {
        search_query = search_query.with_created_by(created_by);
    }
    
    if let Some(min_relevance) = params.min_relevance {
        search_query = search_query.with_min_relevance(min_relevance);
    }
    
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);
    search_query = search_query.with_pagination(offset, limit);
    
    let search_result = match search_engine.search(&search_query).await {
        Ok(result) => result,
        Err(_) => {
            let limit = params.limit.unwrap_or(50).min(100) as usize;
            let offset = params.offset.unwrap_or(0) as usize;
            
            let items = state.item_service.get_items(Some(limit), Some(offset)).await?;
            
            let filtered_items = if let Some(ref tags_str) = params.tags {
                let search_tags: Vec<String> = tags_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if !search_tags.is_empty() {
                    items.into_iter()
                        .filter(|item| {
                            item.tags.iter().any(|tag| search_tags.contains(tag))
                        })
                        .collect()
                } else {
                    items
                }
            } else {
                items
            };
            
            return Ok(Json(ApiResponse::success(serde_json::json!({
                "items": filtered_items.iter().map(|item| serde_json::json!({
                    "item": item,
                    "matched_fields": ["name", "description", "tags"],
                    "relevance_score": 1.0
                })).collect::<Vec<_>>(),
                "total_count": filtered_items.len(),
                "offset": offset,
                "limit": limit,
                "has_more": false,
                "query": {
                    "text": params.q,
                    "tags": params.tags,
                    "sort_by": params.sort_by,
                    "sort_order": params.sort_order,
                    "fuzzy": params.fuzzy.unwrap_or(false)
                }
            }))));
        }
    };
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "items": search_result.items,
        "total_count": search_result.total_count,
        "offset": search_result.offset,
        "limit": search_result.limit,
        "has_more": search_result.has_more,
        "query": {
            "text": params.q,
            "tags": params.tags,
            "sort_by": params.sort_by,
            "sort_order": params.sort_order,
            "fuzzy": params.fuzzy.unwrap_or(false)
        }
    }))))
}

async fn handle_get_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Query(params): Query<ItemListQuery>
) -> Result<impl IntoResponse> {
    info!("GET /api/items - page_size: {:?}, page: {:?}", params.page_size, params.page);
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = params.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Query validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    let page_size = params.page_size.unwrap_or(50) as usize;
    let page = params.page.unwrap_or(1);
    let offset = ((page - 1) * page_size as u32) as usize;
    
    tracing::debug!("Pagination: page={}, page_size={}, offset={}", page, page_size, offset);
    
    let items = state.item_service.get_items(Some(page_size), Some(offset)).await
        .map_err(|e| {
            tracing::error!("Failed to get items: page={}, page_size={}, offset={}, error={:?}", page, page_size, offset, e);
            e
        })?;
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "items": items,
        "count": items.len(),
        "page_size": page_size,
        "page": page,
        "offset": offset,
        "source": if state.item_service.is_using_database() { "database" } else { "memory" }
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

    let item = state.item_service.get_item(id).await?;
    Ok(Json(ApiResponse::success(item)))
}

async fn handle_post_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    payload: crate::extractors::UnicodeJson<CreateItemRequest>
) -> Result<impl IntoResponse> {
    let crate::extractors::UnicodeJson(payload) = payload;
    info!("POST /api/items - name: {}", payload.name);
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = payload.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }

    let item = state.item_service.create_item(
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    ).await?;

    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }

    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemCreated(item.clone());
        ws_manager.broadcast(event).await;
    }

    Ok((StatusCode::CREATED, Json(ApiResponse::success(item))))
}

async fn handle_put_item(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    payload: crate::extractors::UnicodeJson<CreateItemRequest>,
) -> Result<impl IntoResponse> {
    let crate::extractors::UnicodeJson(payload) = payload;
    info!("PUT /api/items/{} - name: {}", id, payload.name);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = payload.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }

    let item = state.item_service.update_item(
        id,
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    ).await?;

    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_item_cache(id);
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }

    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemUpdated(item.clone());
        ws_manager.broadcast(event).await;
    }

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

    state.item_service.delete_item(id).await?;
    
    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_item_cache(id);
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }
    
    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemDeleted(id);
        ws_manager.broadcast(event).await;
    }
    
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

    let item = state.item_service.patch_item(id, patch).await?;
    
    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_item_cache(id);
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }
    
    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemUpdated(item.clone());
        ws_manager.broadcast(event).await;
    }
    
    Ok(Json(ApiResponse::success(item)))
}

async fn handle_form_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    request: Request<Body>
) -> Result<impl IntoResponse> {
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let content_type = headers
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");
    
    let form = if content_type.contains("application/json") {
        let Json(form_data) = Json::<FormPayload>::from_request(request, &state).await
            .map_err(|_| AppError::BadRequest("Invalid JSON format".to_string()))?;
        form_data
    } else {
        let Form(form_data) = Form::<FormPayload>::from_request(request, &state).await
            .map_err(|_| AppError::BadRequest("Invalid form data".to_string()))?;
        form_data
    };
    
    info!("Form submission ({}) - name: {}, email: {}", 
          if content_type.contains("application/json") { "JSON" } else { "Form" },
          form.name, form.email);
    info!("Form submission - name: {}, email: {}", form.name, form.email);
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = form.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Form validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    let item_name = format!("Form submission from {}", form.name);
    let metadata = serde_json::json!({
        "source": "form",
        "email": form.email,
        "message": form.message,
        "submitted_at": chrono::Utc::now().to_rfc3339()
    });
    
    let item = state.item_service.create_item(
        item_name,
        Some(format!("Submitted by {} ({})", form.name, form.email)),
        vec!["form-submission".to_string()],
        Some(metadata)
    ).await?;

    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }

    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemCreated(item.clone());
        ws_manager.broadcast(event).await;
    }

    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": "Form submitted successfully",
        "created_item": item
    }))))
}

async fn handle_head(State(state): State<AppState>) -> impl IntoResponse {
    info!("HEAD /api/head");
    
    let stats = state.item_service.get_stats().await.unwrap_or_default();
    let item_count = stats.get("total_items").and_then(|v| v.as_u64()).unwrap_or(0);
    
    let mut headers = HeaderMap::new();
    headers.insert("X-Custom-Header", "HEAD-Response".parse().unwrap());
    headers.insert("X-Total-Items", item_count.to_string().parse().unwrap());
    headers.insert("X-Api-Version", state.version.parse().unwrap());
    headers.insert("X-Data-Source", 
        if state.item_service.is_using_database() { "database" } else { "memory" }
            .parse().unwrap());
    
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

async fn handle_test_page() -> impl IntoResponse {
    Html(r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Test Page</title>
        <style>
            body { font-family: Arial; background: #f0f0f0; padding: 20px; }
            .test { background: white; padding: 20px; border-radius: 8px; }
        </style>
    </head>
    <body>
        <div class="test">
            <h1>Test Page Working!</h1>
            <p>If you can see this styled page, HTML serving is working.</p>
            <button onclick="testAPI()">Test API Call</button>
            <div id="result"></div>
        </div>
        <script>
            console.log('JavaScript is working!');
            async function testAPI() {
                try {
                    const response = await fetch('/api/metrics');
                    const data = await response.json();
                    document.getElementById('result').innerHTML = 
                        '<pre>' + JSON.stringify(data.data, null, 2) + '</pre>';
                } catch (error) {
                    document.getElementById('result').innerHTML = 'Error: ' + error.message;
                }
            }
        </script>
    </body>
    </html>
    "#)
}

async fn handle_websocket_test() -> impl IntoResponse {
    Html(r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8" />
            <meta name="viewport" content="width=device-width, initial-scale=1.0" />
            <title>Elite WebSocket Testing Suite</title>
            <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css" rel="stylesheet">
            <style>
            :root {
                --primary-bg: #0f172a;
                --secondary-bg: #1e293b;
                --accent-color: #60a5fa;
                --success-color: #10b981;
                --warning-color: #f59e0b;
                --error-color: #ef4444;
                --text-primary: #f1f5f9;
                --text-secondary: #94a3b8;
                --border-color: #334155;
            }

            [data-theme="light"] {
                --primary-bg: #f8fafc;
                --secondary-bg: #ffffff;
                --accent-color: #3b82f6;
                --success-color: #059669;
                --warning-color: #d97706;
                --error-color: #dc2626;
                --text-primary: #1e293b;
                --text-secondary: #64748b;
                --border-color: #e2e8f0;
            }

            * {
                box-sizing: border-box;
                margin: 0;
                padding: 0;
            }

            body {
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
                background: linear-gradient(135deg, var(--primary-bg) 0%, var(--secondary-bg) 100%);
                color: var(--text-primary);
                min-height: 100vh;
                padding: 20px;
                position: relative;
            }

            body::before {
                content: '';
                position: fixed;
                top: 0;
                left: 0;
                width: 100%;
                height: 100%;
                background: radial-gradient(circle at 20% 80%, rgba(120, 119, 198, 0.1) 0%, transparent 50%),
                        radial-gradient(circle at 80% 20%, rgba(255, 119, 198, 0.1) 0%, transparent 50%);
                pointer-events: none;
                z-index: -1;
            }

            .container {
                max-width: 1400px;
                margin: 0 auto;
                display: grid;
                grid-template-columns: 1fr 1fr;
                gap: 20px;
                height: calc(100vh - 40px);
            }

            .panel {
                background: rgba(30, 41, 59, 0.8);
                backdrop-filter: blur(10px);
                border: 1px solid var(--border-color);
                border-radius: 16px;
                padding: 24px;
                overflow: hidden;
                display: flex;
                flex-direction: column;
            }

            .panel-header {
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 20px;
                padding-bottom: 16px;
                border-bottom: 1px solid var(--border-color);
            }

            .panel-title {
                font-size: 1.5rem;
                font-weight: 700;
                background: linear-gradient(to right, var(--accent-color), #a78bfa);
                -webkit-background-clip: text;
                -webkit-text-fill-color: transparent;
                display: flex;
                align-items: center;
                gap: 10px;
            }

            .theme-toggle {
                position: fixed;
                top: 20px;
                right: 20px;
                z-index: 1000;
                background: var(--secondary-bg);
                border: 1px solid var(--border-color);
                border-radius: 50px;
                padding: 8px;
                display: flex;
                gap: 4px;
                backdrop-filter: blur(10px);
            }

            .theme-btn {
                padding: 8px 12px;
                border: none;
                border-radius: 20px;
                background: transparent;
                color: var(--text-secondary);
                cursor: pointer;
                transition: all 0.3s ease;
                font-size: 0.875rem;
            }

            .theme-btn.active {
                background: var(--accent-color);
                color: white;
            }

            .status-bar {
                display: flex;
                gap: 12px;
                margin-bottom: 20px;
                flex-wrap: wrap;
            }

            .status-badge {
                padding: 8px 16px;
                border-radius: 20px;
                font-size: 0.875rem;
                font-weight: 600;
                display: flex;
                align-items: center;
                gap: 8px;
                transition: all 0.3s ease;
            }

            .status-connected {
                background: linear-gradient(135deg, var(--success-color), #059669);
                color: white;
                animation: pulse 2s infinite;
            }

            .status-disconnected {
                background: linear-gradient(135deg, var(--error-color), #dc2626);
                color: white;
            }

            .status-info {
                background: rgba(96, 165, 250, 0.2);
                color: var(--accent-color);
                border: 1px solid rgba(96, 165, 250, 0.3);
            }

            @keyframes pulse {
                0%, 100% { opacity: 1; }
                50% { opacity: 0.8; }
            }

            .controls-grid {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                gap: 12px;
                margin-bottom: 20px;
            }

            .btn {
                padding: 12px 20px;
                border: none;
                border-radius: 12px;
                font-weight: 600;
                cursor: pointer;
                transition: all 0.3s ease;
                display: flex;
                align-items: center;
                justify-content: center;
                gap: 8px;
                font-size: 0.875rem;
                position: relative;
                overflow: hidden;
            }

            .btn::before {
                content: '';
                position: absolute;
                top: 0;
                left: -100%;
                width: 100%;
                height: 100%;
                background: linear-gradient(90deg, transparent, rgba(255,255,255,0.2), transparent);
                transition: left 0.5s;
            }

            .btn:hover::before {
                left: 100%;
            }

            .btn:disabled {
                opacity: 0.5;
                cursor: not-allowed;
            }

            .btn-primary {
                background: linear-gradient(135deg, var(--accent-color), #3b82f6);
                color: white;
            }

            .btn-success {
                background: linear-gradient(135deg, var(--success-color), #059669);
                color: white;
            }

            .btn-danger {
                background: linear-gradient(135deg, var(--error-color), #dc2626);
                color: white;
            }

            .btn-warning {
                background: linear-gradient(135deg, var(--warning-color), #d97706);
                color: white;
            }

            .input-group {
                display: flex;
                gap: 8px;
                margin-bottom: 16px;
            }

            .input {
                flex: 1;
                padding: 12px 16px;
                border: 1px solid var(--border-color);
                border-radius: 8px;
                background: var(--secondary-bg);
                color: var(--text-primary);
                font-size: 0.875rem;
            }

            .input:focus {
                outline: none;
                border-color: var(--accent-color);
                box-shadow: 0 0 0 3px rgba(96, 165, 250, 0.1);
            }

            .messages-container {
                flex: 1;
                display: flex;
                flex-direction: column;
                min-height: 0;
            }

            .messages {
                flex: 1;
                overflow-y: auto;
                border: 1px solid var(--border-color);
                border-radius: 12px;
                padding: 16px;
                background: var(--primary-bg);
                font-family: 'Consolas', 'Monaco', monospace;
                font-size: 13px;
                line-height: 1.5;
            }

            .messages::-webkit-scrollbar {
                width: 8px;
            }

            .messages::-webkit-scrollbar-track {
                background: var(--primary-bg);
                border-radius: 4px;
            }

            .messages::-webkit-scrollbar-thumb {
                background: var(--border-color);
                border-radius: 4px;
            }

            .message {
                margin: 8px 0;
                padding: 12px 16px;
                border-radius: 8px;
                border-left: 4px solid var(--accent-color);
                background: var(--secondary-bg);
                animation: slideIn 0.3s ease;
                position: relative;
            }

            .message.sent {
                border-left-color: var(--success-color);
                background: rgba(16, 185, 129, 0.1);
            }

            .message.received {
                border-left-color: var(--accent-color);
                background: rgba(96, 165, 250, 0.1);
            }

            .message.error {
                border-left-color: var(--error-color);
                background: rgba(239, 68, 68, 0.1);
            }

            .message.system {
                border-left-color: var(--warning-color);
                background: rgba(245, 158, 11, 0.1);
            }

            @keyframes slideIn {
                from { transform: translateX(-10px); opacity: 0; }
                to { transform: translateX(0); opacity: 1; }
            }

            .message-header {
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 8px;
                font-size: 0.75rem;
                color: var(--text-secondary);
            }

            .message-type {
                font-weight: 600;
                text-transform: uppercase;
                letter-spacing: 0.05em;
            }

            .message-time {
                opacity: 0.7;
            }

            .message-content {
                color: var(--text-primary);
                word-break: break-word;
            }

            .json-content {
                background: var(--primary-bg);
                border-radius: 6px;
                padding: 12px;
                margin-top: 8px;
                overflow-x: auto;
            }

            .stats-grid {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
                gap: 12px;
                margin-bottom: 20px;
            }

            .stat-card {
                background: rgba(96, 165, 250, 0.1);
                border: 1px solid rgba(96, 165, 250, 0.2);
                border-radius: 12px;
                padding: 16px;
                text-align: center;
            }

            .stat-value {
                font-size: 1.5rem;
                font-weight: 700;
                color: var(--accent-color);
                margin-bottom: 4px;
            }

            .stat-label {
                font-size: 0.75rem;
                color: var(--text-secondary);
                text-transform: uppercase;
                letter-spacing: 0.05em;
            }

            .test-scenarios {
                display: grid;
                gap: 8px;
                margin-bottom: 20px;
            }

            .scenario-btn {
                padding: 10px 16px;
                background: rgba(30, 41, 59, 0.5);
                border: 1px solid var(--border-color);
                border-radius: 8px;
                color: var(--text-primary);
                cursor: pointer;
                transition: all 0.3s ease;
                text-align: left;
                font-size: 0.875rem;
            }

            .scenario-btn:hover {
                background: rgba(96, 165, 250, 0.1);
                border-color: var(--accent-color);
            }

            .scenario-btn:disabled {
                opacity: 0.5;
                cursor: not-allowed;
            }

            @media (max-width: 1200px) {
                .container {
                grid-template-columns: 1fr;
                height: auto;
                }
            }
            </style>
        </head>
        <body>
            <div class="theme-toggle">
            <button class="theme-btn active" data-theme="dark">
                <i class="fas fa-moon"></i>
            </button>
            <button class="theme-btn" data-theme="light">
                <i class="fas fa-sun"></i>
            </button>
            </div>

            <div class="container">
            <div class="panel">
                <div class="panel-header">
                <h1 class="panel-title">
                    <i class="fas fa-plug"></i>
                    WebSocket Control Center
                </h1>
                </div>

                <div class="status-bar">
                <div id="connectionStatus" class="status-badge status-disconnected">
                    <i class="fas fa-times-circle"></i>
                    <span>Disconnected</span>
                </div>
                <div id="connectionInfo" class="status-badge status-info">
                    <i class="fas fa-info-circle"></i>
                    <span id="connectionId">No Connection</span>
                </div>
                <div id="uptimeInfo" class="status-badge status-info">
                    <i class="fas fa-clock"></i>
                    <span id="uptime">00:00:00</span>
                </div>
                </div>

                <div class="input-group">
                <input 
                    type="text" 
                    id="wsUrl" 
                    class="input" 
                    value="ws://localhost:3000/ws" 
                    placeholder="WebSocket URL"
                />
                <input 
                    type="text" 
                    id="authToken" 
                    class="input" 
                    placeholder="JWT Token (optional)"
                />
                </div>

                <div class="controls-grid">
                <button id="connectBtn" class="btn btn-success">
                    <i class="fas fa-plug"></i>
                    Connect
                </button>
                <button id="disconnectBtn" class="btn btn-danger" disabled>
                    <i class="fas fa-times"></i>
                    Disconnect
                </button>
                <button id="getTokenBtn" class="btn btn-primary">
                    <i class="fas fa-key"></i>
                    Get Auth Token
                </button>
                <button id="clearBtn" class="btn btn-warning">
                    <i class="fas fa-trash"></i>
                    Clear Messages
                </button>
                </div>

                <div class="stats-grid">
                <div class="stat-card">
                    <div class="stat-value" id="messagesSent">0</div>
                    <div class="stat-label">Sent</div>
                </div>
                <div class="stat-card">
                    <div class="stat-value" id="messagesReceived">0</div>
                    <div class="stat-label">Received</div>
                </div>
                <div class="stat-card">
                    <div class="stat-value" id="errorsCount">0</div>
                    <div class="stat-label">Errors</div>
                </div>
                <div class="stat-card">
                    <div class="stat-value" id="latency">0ms</div>
                    <div class="stat-label">Latency</div>
                </div>
                </div>

                <h3 style="margin-bottom: 16px; color: var(--text-primary);">
                <i class="fas fa-flask"></i> Test Scenarios
                </h3>
                <div class="test-scenarios">
                <button class="scenario-btn" data-test="ping" disabled>
                    <i class="fas fa-satellite-dish"></i> Send Ping/Pong Test
                </button>
                <button class="scenario-btn" data-test="create-item" disabled>
                    <i class="fas fa-plus"></i> Create Item (triggers ItemCreated event)
                </button>
                <button class="scenario-btn" data-test="update-item" disabled>
                    <i class="fas fa-edit"></i> Update Item (triggers ItemUpdated event)
                </button>
                <button class="scenario-btn" data-test="delete-item" disabled>
                    <i class="fas fa-trash"></i> Delete Item (triggers ItemDeleted event)
                </button>
                <button class="scenario-btn" data-test="metrics" disabled>
                    <i class="fas fa-chart-line"></i> Request Metrics Update
                </button>
                <button class="scenario-btn" data-test="job-test" disabled>
                    <i class="fas fa-cogs"></i> Create Background Job
                </button>
                <button class="scenario-btn" data-test="stress-test" disabled>
                    <i class="fas fa-tachometer-alt"></i> Stress Test (100 messages)
                </button>
                <button class="scenario-btn" data-test="custom-message" disabled>
                    <i class="fas fa-code"></i> Send Custom JSON Message
                </button>
                </div>

                <div id="customMessagePanel" style="display: none; margin-top: 16px;">
                <textarea 
                    id="customMessage" 
                    class="input" 
                    rows="4" 
                    placeholder='{"type": "Ping"}'
                    style="resize: vertical; font-family: monospace;"
                ></textarea>
                <div style="margin-top: 8px;">
                    <button id="sendCustomBtn" class="btn btn-primary">
                    <i class="fas fa-paper-plane"></i>
                    Send Custom Message
                    </button>
                </div>
                </div>
            </div>

            <div class="panel">
                <div class="panel-header">
                <h2 class="panel-title">
                    <i class="fas fa-comments"></i>
                    Live Messages
                </h2>
                <div style="display: flex; gap: 8px;">
                    <button id="autoScrollBtn" class="btn btn-primary" style="padding: 6px 12px; font-size: 0.75rem;">
                    <i class="fas fa-arrow-down"></i>
                    Auto Scroll
                    </button>
                    <button id="exportBtn" class="btn btn-primary" style="padding: 6px 12px; font-size: 0.75rem;">
                    <i class="fas fa-download"></i>
                    Export
                    </button>
                </div>
                </div>

                <div class="messages-container">
                <div id="messages" class="messages"></div>
                </div>
            </div>
            </div>

            <script>
            class WebSocketTester {
                constructor() {
                this.ws = null;
                this.connectionId = null;
                this.connectTime = null;
                this.stats = {
                    messagesSent: 0,
                    messagesReceived: 0,
                    errorsCount: 0,
                    latency: 0
                };
                this.autoScroll = true;
                this.theme = 'dark';
                this.pingStartTime = null;
                this.lastItemId = null;
                
                this.initializeElements();
                this.initializeEventListeners();
                this.initializeTheme();
                this.startUptimeTimer();
                }

                initializeElements() {
                this.elements = {
                    connectionStatus: document.getElementById('connectionStatus'),
                    connectionId: document.getElementById('connectionId'),
                    uptime: document.getElementById('uptime'),
                    
                    wsUrl: document.getElementById('wsUrl'),
                    authToken: document.getElementById('authToken'),
                    connectBtn: document.getElementById('connectBtn'),
                    disconnectBtn: document.getElementById('disconnectBtn'),
                    getTokenBtn: document.getElementById('getTokenBtn'),
                    clearBtn: document.getElementById('clearBtn'),
                    
                    messagesSent: document.getElementById('messagesSent'),
                    messagesReceived: document.getElementById('messagesReceived'),
                    errorsCount: document.getElementById('errorsCount'),
                    latency: document.getElementById('latency'),
                    
                    messages: document.getElementById('messages'),
                    autoScrollBtn: document.getElementById('autoScrollBtn'),
                    exportBtn: document.getElementById('exportBtn'),
                    
                    customMessage: document.getElementById('customMessage'),
                    sendCustomBtn: document.getElementById('sendCustomBtn'),
                    customMessagePanel: document.getElementById('customMessagePanel')
                };
                }

                initializeEventListeners() {
                this.elements.connectBtn.addEventListener('click', () => this.connect());
                this.elements.disconnectBtn.addEventListener('click', () => this.disconnect());
                this.elements.getTokenBtn.addEventListener('click', () => this.getAuthToken());
                this.elements.clearBtn.addEventListener('click', () => this.clearMessages());
                
                this.elements.autoScrollBtn.addEventListener('click', () => this.toggleAutoScroll());
                this.elements.exportBtn.addEventListener('click', () => this.exportMessages());
                this.elements.sendCustomBtn.addEventListener('click', () => this.sendCustomMessage());
                
                document.querySelectorAll('.scenario-btn').forEach(btn => {
                    btn.addEventListener('click', () => this.runTestScenario(btn.dataset.test));
                });
                
                document.querySelectorAll('.theme-btn').forEach(btn => {
                    btn.addEventListener('click', () => this.setTheme(btn.dataset.theme));
                });
                
                document.querySelector('[data-test="custom-message"]').addEventListener('click', () => {
                    this.elements.customMessagePanel.style.display = 
                    this.elements.customMessagePanel.style.display === 'none' ? 'block' : 'none';
                });
                }

                initializeTheme() {
                const savedTheme = localStorage.getItem('ws-tester-theme') || 'dark';
                this.setTheme(savedTheme);
                }

                setTheme(theme) {
                this.theme = theme;
                document.documentElement.setAttribute('data-theme', theme);
                localStorage.setItem('ws-tester-theme', theme);
                
                document.querySelectorAll('.theme-btn').forEach(btn => {
                    btn.classList.toggle('active', btn.dataset.theme === theme);
                });
                }

                startUptimeTimer() {
                setInterval(() => {
                    if (this.connectTime) {
                    const uptime = Date.now() - this.connectTime;
                    this.elements.uptime.textContent = this.formatUptime(uptime);
                    }
                }, 1000);
                }

                formatUptime(ms) {
                const seconds = Math.floor(ms / 1000);
                const hours = Math.floor(seconds / 3600);
                const minutes = Math.floor((seconds % 3600) / 60);
                const secs = seconds % 60;
                return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
                }

                async connect() {
                const wsUrl = this.elements.wsUrl.value;
                const token = this.elements.authToken.value;
                
                let fullUrl = wsUrl;
                if (token) {
                    fullUrl += `?token=${encodeURIComponent(token)}`;
                }
                
                this.addMessage(`Connecting to ${fullUrl}...`, 'system');
                
                try {
                    this.ws = new WebSocket(fullUrl);
                    
                    this.ws.onopen = (event) => {
                    this.connectTime = Date.now();
                    this.addMessage('✅ WebSocket connection established', 'system');
                    this.updateConnectionStatus(true);
                    };
                    
                    this.ws.onmessage = (event) => {
                    this.stats.messagesReceived++;
                    this.updateStats();
                    
                    try {
                        const data = JSON.parse(event.data);
                        this.handleMessage(data);
                    } catch (e) {
                        this.addMessage(`📨 Raw message: ${event.data}`, 'received');
                    }
                    };
                    
                    this.ws.onclose = (event) => {
                    this.addMessage(`❌ Connection closed (Code: ${event.code}, Reason: ${event.reason || 'No reason'})`, 'system');
                    this.updateConnectionStatus(false);
                    this.connectTime = null;
                    };
                    
                    this.ws.onerror = (error) => {
                    this.stats.errorsCount++;
                    this.updateStats();
                    this.addMessage(`🚨 WebSocket error: ${error.message || 'Unknown error'}`, 'error');
                    };
                    
                } catch (error) {
                    this.addMessage(`🚨 Failed to connect: ${error.message}`, 'error');
                }
                }

                disconnect() {
                if (this.ws) {
                    this.ws.close(1000, 'User initiated disconnect');
                    this.ws = null;
                }
                }

                handleMessage(data) {
                const messageType = data.type || 'Unknown';
                let messageContent = '';
                
                switch (messageType) {
                    case 'Connected':
                    this.connectionId = data.data?.connection_id;
                    this.elements.connectionId.textContent = `ID: ${this.connectionId?.substring(0, 8)}...`;
                    messageContent = `🔗 Connected with ID: ${this.connectionId}`;
                    break;
                    
                    case 'Pong':
                    if (this.pingStartTime) {
                        const latency = Date.now() - this.pingStartTime;
                        this.stats.latency = latency;
                        this.updateStats();
                        messageContent = `🏓 Pong received (${latency}ms)`;
                        this.pingStartTime = null;
                    } else {
                        messageContent = '🏓 Pong received';
                    }
                    break;
                    
                    case 'ItemCreated':
                    this.lastItemId = data.data?.id;
                    messageContent = `📦 Item Created: ${data.data?.name} (ID: ${data.data?.id})`;
                    break;
                    
                    case 'ItemUpdated':
                    messageContent = `📝 Item Updated: ${data.data?.name} (ID: ${data.data?.id})`;
                    break;
                    
                    case 'ItemDeleted':
                    messageContent = `🗑️ Item Deleted: ID ${data.data?.id}`;
                    break;
                    
                    case 'MetricsUpdate':
                    messageContent = `📊 Metrics Update: ${data.data?.total_requests} total requests`;
                    break;
                    
                    case 'JobStarted':
                    messageContent = `⚙️ Job Started: ${data.data?.job_type} (ID: ${data.data?.id})`;
                    break;
                    
                    case 'JobCompleted':
                    messageContent = `✅ Job Completed: ${data.data?.job_type} (ID: ${data.data?.id})`;
                    break;
                    
                    case 'JobFailed':
                    messageContent = `❌ Job Failed: ${data.data?.job_type} (ID: ${data.data?.id})`;
                    break;
                    
                    case 'Error':
                    messageContent = `🚨 Server Error: ${data.data?.message}`;
                    break;
                    
                    default:
                    messageContent = `📨 ${messageType}`;
                }
                
                this.addMessage(messageContent, 'received', data);
                }

                sendMessage(message) {
                if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                    const jsonMessage = JSON.stringify(message);
                    this.ws.send(jsonMessage);
                    this.stats.messagesSent++;
                    this.updateStats();
                    return true;
                }
                return false;
                }

                async runTestScenario(testType) {
                switch (testType) {
                    case 'ping':
                    await this.testPing();
                    break;
                    case 'create-item':
                    await this.testCreateItem();
                    break;
                    case 'update-item':
                    await this.testUpdateItem();
                    break;
                    case 'delete-item':
                    await this.testDeleteItem();
                    break;
                    case 'metrics':
                    await this.testMetrics();
                    break;
                    case 'job-test':
                    await this.testBackgroundJob();
                    break;
                    case 'stress-test':
                    await this.testStressTest();
                    break;
                }
                }

                async testPing() {
                this.pingStartTime = Date.now();
                const success = this.sendMessage({ type: 'Ping' });
                if (success) {
                    this.addMessage('🏓 Ping sent', 'sent');
                } else {
                    this.addMessage('❌ Failed to send ping - not connected', 'error');
                }
                }

                async testCreateItem() {
                try {
                    const response = await fetch('/api/items', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        name: `WebSocket Test Item ${Date.now()}`,
                        description: 'Created from Elite WebSocket Testing Suite',
                        tags: ['websocket', 'test', 'elite']
                    })
                    });
                    
                    if (response.ok) {
                    const result = await response.json();
                    this.addMessage(`📦 Item created via API: ${result.data?.name}`, 'sent');
                    } else {
                    this.addMessage(`❌ Failed to create item: ${response.status}`, 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Error creating item: ${error.message}`, 'error');
                }
                }

                async testUpdateItem() {
                if (!this.lastItemId) {
                    this.addMessage('❌ No item to update. Create an item first.', 'error');
                    return;
                }
                
                try {
                    const response = await fetch(`/api/items/${this.lastItemId}`, {
                    method: 'PUT',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        name: `Updated Item ${Date.now()}`,
                        description: 'Updated from Elite WebSocket Testing Suite',
                        tags: ['websocket', 'test', 'updated']
                    })
                    });
                    
                    if (response.ok) {
                    this.addMessage(`📝 Item updated via API: ID ${this.lastItemId}`, 'sent');
                    } else {
                    this.addMessage(`❌ Failed to update item: ${response.status}`, 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Error updating item: ${error.message}`, 'error');
                }
                }

                async testDeleteItem() {
                if (!this.lastItemId) {
                    this.addMessage('❌ No item to delete. Create an item first.', 'error');
                    return;
                }
                
                try {
                    const response = await fetch(`/api/items/${this.lastItemId}`, {
                    method: 'DELETE'
                    });
                    
                    if (response.ok) {
                    this.addMessage(`🗑️ Item deleted via API: ID ${this.lastItemId}`, 'sent');
                    this.lastItemId = null;
                    } else {
                    this.addMessage(`❌ Failed to delete item: ${response.status}`, 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Error deleting item: ${error.message}`, 'error');
                }
                }

                async testMetrics() {
                try {
                    const response = await fetch('/api/metrics');
                    if (response.ok) {
                    this.addMessage('📊 Metrics requested via API', 'sent');
                    } else {
                    this.addMessage(`❌ Failed to get metrics: ${response.status}`, 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Error getting metrics: ${error.message}`, 'error');
                }
                }

                async testBackgroundJob() {
                try {
                    const response = await fetch('/api/jobs', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        job_type: 'test_job',
                        payload: { message: 'WebSocket test job', timestamp: Date.now() }
                    })
                    });
                    
                    if (response.ok) {
                    const result = await response.json();
                    this.addMessage(`⚙️ Background job created: ${result.data?.id}`, 'sent');
                    } else {
                    this.addMessage(`❌ Failed to create job: ${response.status}`, 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Error creating job: ${error.message}`, 'error');
                }
                }

                async testStressTest() {
                this.addMessage('🚀 Starting stress test (100 ping messages)...', 'system');
                
                for (let i = 0; i < 100; i++) {
                    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                    this.sendMessage({ type: 'Ping', sequence: i + 1 });
                    await new Promise(resolve => setTimeout(resolve, 10));
                    } else {
                    this.addMessage('❌ Connection lost during stress test', 'error');
                    break;
                    }
                }
                
                this.addMessage('✅ Stress test completed', 'system');
                }

                async getAuthToken() {
                try {
                    const response = await fetch('/auth/login', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        username_or_email: 'testuser',
                        password: 'MyVerySecureP@ssw0rd2025!'
                    })
                    });
                    
                    if (response.ok) {
                    const result = await response.json();
                    this.elements.authToken.value = result.access_token;
                    this.addMessage('🔑 Auth token retrieved successfully', 'system');
                    } else {
                    this.addMessage('❌ Failed to get auth token. Try registering first.', 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Error getting auth token: ${error.message}`, 'error');
                }
                }

                sendCustomMessage() {
                try {
                    const messageText = this.elements.customMessage.value.trim();
                    if (!messageText) {
                    this.addMessage('❌ Custom message is empty', 'error');
                    return;
                    }
                    
                    const message = JSON.parse(messageText);
                    const success = this.sendMessage(message);
                    
                    if (success) {
                    this.addMessage(`📤 Custom message sent: ${messageText}`, 'sent');
                    } else {
                    this.addMessage('❌ Failed to send custom message - not connected', 'error');
                    }
                } catch (error) {
                    this.addMessage(`❌ Invalid JSON in custom message: ${error.message}`, 'error');
                }
                }

                addMessage(content, type = 'system', rawData = null) {
                const messageEl = document.createElement('div');
                messageEl.className = `message ${type}`;
                
                const headerEl = document.createElement('div');
                headerEl.className = 'message-header';
                headerEl.innerHTML = `
                    <span class="message-type">${type}</span>
                    <span class="message-time">${new Date().toLocaleTimeString()}</span>
                `;
                
                const contentEl = document.createElement('div');
                contentEl.className = 'message-content';
                contentEl.textContent = content;
                
                messageEl.appendChild(headerEl);
                messageEl.appendChild(contentEl);
                
                if (rawData) {
                    const jsonEl = document.createElement('div');
                    jsonEl.className = 'json-content';
                    jsonEl.textContent = JSON.stringify(rawData, null, 2);
                    messageEl.appendChild(jsonEl);
                }
                
                this.elements.messages.appendChild(messageEl);
                
                if (this.autoScroll) {
                    this.elements.messages.scrollTop = this.elements.messages.scrollHeight;
                }
                }

                updateConnectionStatus(connected) {
                const statusEl = this.elements.connectionStatus;
                const scenarioBtns = document.querySelectorAll('.scenario-btn');
                
                if (connected) {
                    statusEl.innerHTML = '<i class="fas fa-check-circle"></i><span>Connected</span>';
                    statusEl.className = 'status-badge status-connected';
                    this.elements.connectBtn.disabled = true;
                    this.elements.disconnectBtn.disabled = false;
                    scenarioBtns.forEach(btn => btn.disabled = false);
                } else {
                    statusEl.innerHTML = '<i class="fas fa-times-circle"></i><span>Disconnected</span>';
                    statusEl.className = 'status-badge status-disconnected';
                    this.elements.connectBtn.disabled = false;
                    this.elements.disconnectBtn.disabled = true;
                    scenarioBtns.forEach(btn => btn.disabled = true);
                    this.elements.connectionId.textContent = 'No Connection';
                    this.elements.uptime.textContent = '00:00:00';
                }
                }

                updateStats() {
                this.elements.messagesSent.textContent = this.stats.messagesSent;
                this.elements.messagesReceived.textContent = this.stats.messagesReceived;
                this.elements.errorsCount.textContent = this.stats.errorsCount;
                this.elements.latency.textContent = `${this.stats.latency}ms`;
                }

                toggleAutoScroll() {
                this.autoScroll = !this.autoScroll;
                this.elements.autoScrollBtn.innerHTML = this.autoScroll 
                    ? '<i class="fas fa-arrow-down"></i> Auto Scroll'
                    : '<i class="fas fa-pause"></i> Manual';
                }

                clearMessages() {
                this.elements.messages.innerHTML = '';
                this.stats = { messagesSent: 0, messagesReceived: 0, errorsCount: 0, latency: 0 };
                this.updateStats();
                }

                exportMessages() {
                const messages = Array.from(this.elements.messages.children).map(msg => ({
                    type: msg.querySelector('.message-type').textContent,
                    time: msg.querySelector('.message-time').textContent,
                    content: msg.querySelector('.message-content').textContent,
                    rawData: msg.querySelector('.json-content')?.textContent
                }));
                
                const exportData = {
                    timestamp: new Date().toISOString(),
                    stats: this.stats,
                    connectionId: this.connectionId,
                    messages: messages
                };
                
                const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `websocket-test-${new Date().toISOString().split('T')[0]}.json`;
                document.body.appendChild(a);
                a.click();
                document.body.removeChild(a);
                URL.revokeObjectURL(url);
                
                this.addMessage('📥 Messages exported successfully', 'system');
                }
            }

            const tester = new WebSocketTester();
            
            tester.addMessage('🚀 Elite WebSocket Testing Suite initialized', 'system');
            tester.addMessage('💡 Connect to start testing WebSocket functionality', 'system');
            </script>
        </body>
        </html>
    "#)
}

async fn handle_dashboard() -> impl IntoResponse {
    Html(r#"
    <!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Rust HTTP Server - Dashboard</title>
        <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
        <script src="https://cdn.jsdelivr.net/npm/chartjs-adapter-date-fns/dist/chartjs-adapter-date-fns.bundle.min.js"></script>
        <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css" rel="stylesheet">
        <style>
            * {
                box-sizing: border-box;
                margin: 0;
                padding: 0;
            }
            
            body {
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
                background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
                color: #e2e8f0;
                min-height: 100vh;
                overflow-x: hidden;
                position: relative;
            }
            
            body::before {
                content: '';
                position: fixed;
                top: 0;
                left: 0;
                width: 100%;
                height: 100%;
                background: radial-gradient(circle at 20% 80%, rgba(120, 119, 198, 0.1) 0%, transparent 50%),
                           radial-gradient(circle at 80% 20%, rgba(255, 119, 198, 0.1) 0%, transparent 50%),
                           radial-gradient(circle at 40% 40%, rgba(16, 185, 129, 0.05) 0%, transparent 50%);
                pointer-events: none;
                z-index: -1;
                animation: backgroundShift 20s ease-in-out infinite;
            }
            
            @keyframes backgroundShift {
                0%, 100% { opacity: 1; }
                50% { opacity: 0.8; }
            }
            
            :root {
                --primary-bg: #0f172a;
                --secondary-bg: #1e293b;
                --accent-color: #60a5fa;
                --success-color: #10b981;
                --warning-color: #f59e0b;
                --error-color: #ef4444;
                --text-primary: #f1f5f9;
                --text-secondary: #94a3b8;
                --border-color: #334155;
            }
            
            [data-theme="light"] {
                --primary-bg: #f8fafc;
                --secondary-bg: #ffffff;
                --accent-color: #3b82f6;
                --success-color: #059669;
                --warning-color: #d97706;
                --error-color: #dc2626;
                --text-primary: #1e293b;
                --text-secondary: #64748b;
                --border-color: #e2e8f0;
            }
            
            .theme-toggle {
                position: fixed;
                top: 20px;
                right: 20px;
                z-index: 1000;
                background: var(--secondary-bg);
                border: 1px solid var(--border-color);
                border-radius: 50px;
                padding: 8px;
                display: flex;
                gap: 4px;
                backdrop-filter: blur(10px);
                box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
            }
            
            .theme-btn {
                padding: 8px 12px;
                border: none;
                border-radius: 20px;
                background: transparent;
                color: var(--text-secondary);
                cursor: pointer;
                transition: all 0.3s ease;
                font-size: 0.875rem;
            }
            
            .theme-btn.active {
                background: var(--accent-color);
                color: white;
                box-shadow: 0 4px 12px rgba(96, 165, 250, 0.3);
            }
            
            .alert-panel {
                position: fixed;
                top: 80px;
                right: 20px;
                width: 320px;
                max-height: 400px;
                overflow-y: auto;
                z-index: 999;
                display: none;
            }
            
            .alert-item {
                background: var(--secondary-bg);
                border: 1px solid var(--border-color);
                border-radius: 12px;
                padding: 16px;
                margin-bottom: 12px;
                backdrop-filter: blur(10px);
                animation: slideInRight 0.3s ease;
                position: relative;
                overflow: hidden;
            }
            
            .alert-item::before {
                content: '';
                position: absolute;
                left: 0;
                top: 0;
                bottom: 0;
                width: 4px;
                background: var(--error-color);
            }
            
            .alert-item.warning::before { background: var(--warning-color); }
            .alert-item.success::before { background: var(--success-color); }
            
            @keyframes slideInRight {
                from { transform: translateX(100%); opacity: 0; }
                to { transform: translateX(0); opacity: 1; }
            }
            
            .system-map {
                background: var(--secondary-bg);
                border: 1px solid var(--border-color);
                border-radius: 16px;
                padding: 24px;
                margin-bottom: 30px;
                position: relative;
                overflow: hidden;
            }
            
            .system-node {
                position: absolute;
                width: 60px;
                height: 60px;
                border-radius: 50%;
                display: flex;
                align-items: center;
                justify-content: center;
                font-size: 1.5rem;
                color: white;
                cursor: pointer;
                transition: all 0.3s ease;
                box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
            }
            
            .system-node:hover {
                transform: scale(1.1);
                box-shadow: 0 8px 30px rgba(0, 0, 0, 0.3);
            }
            
            .system-connection {
                position: absolute;
                height: 2px;
                background: linear-gradient(90deg, var(--accent-color), transparent);
                animation: dataFlow 2s linear infinite;
            }
            
            @keyframes dataFlow {
                0% { background-position: -100% 0; }
                100% { background-position: 100% 0; }
            }
            
            .performance-heatmap {
                display: grid;
                grid-template-columns: repeat(24, 1fr);
                gap: 2px;
                margin: 20px 0;
            }
            
            .heatmap-cell {
                aspect-ratio: 1;
                border-radius: 2px;
                transition: all 0.2s ease;
                cursor: pointer;
            }
            
            .heatmap-cell:hover {
                transform: scale(1.2);
                z-index: 10;
                position: relative;
            }
            
            .analytics-panel {
                background: var(--secondary-bg);
                border: 1px solid var(--border-color);
                border-radius: 16px;
                padding: 24px;
                margin-bottom: 30px;
            }
            
            .insight-card {
                background: rgba(96, 165, 250, 0.1);
                border: 1px solid rgba(96, 165, 250, 0.2);
                border-radius: 12px;
                padding: 16px;
                margin: 12px 0;
                position: relative;
            }
            
            .insight-card::before {
                content: '🧠';
                position: absolute;
                top: -8px;
                left: 16px;
                background: var(--secondary-bg);
                padding: 4px 8px;
                border-radius: 20px;
                font-size: 0.875rem;
            }
            
            .live-requests {
                background: var(--secondary-bg);
                border: 1px solid var(--border-color);
                border-radius: 16px;
                padding: 24px;
                height: 300px;
                overflow: hidden;
                position: relative;
            }
            
            .request-flow {
                position: absolute;
                width: 4px;
                height: 4px;
                background: var(--accent-color);
                border-radius: 50%;
                animation: requestFlow 3s linear infinite;
                box-shadow: 0 0 10px var(--accent-color);
            }
            
            @keyframes requestFlow {
                0% { transform: translateX(0) translateY(0); opacity: 1; }
                100% { transform: translateX(300px) translateY(100px); opacity: 0; }
            }
            
            .container {
                max-width: 1400px;
                margin: 0 auto;
                padding: 20px;
            }
            
            .header {
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 30px;
                padding-bottom: 20px;
                border-bottom: 1px solid #334155;
            }
            
            h1 {
                font-size: 2.5rem;
                font-weight: 700;
                background: linear-gradient(to right, #60a5fa, #a78bfa);
                -webkit-background-clip: text;
                -webkit-text-fill-color: transparent;
                display: flex;
                align-items: center;
                gap: 10px;
            }
            
            .status-badge {
                background: #10b981;
                color: white;
                padding: 4px 12px;
                border-radius: 20px;
                font-size: 0.875rem;
                font-weight: 500;
                animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
            }
            
            @keyframes pulse {
                0%, 100% { opacity: 1; }
                50% { opacity: .8; }
            }
            
            .metrics-grid {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                gap: 20px;
                margin-bottom: 30px;
            }
            
            .metric-card {
                background: rgba(30, 41, 59, 0.8);
                backdrop-filter: blur(10px);
                border: 1px solid rgba(51, 65, 85, 0.5);
                border-radius: 16px;
                padding: 24px;
                transition: all 0.3s ease;
                position: relative;
                overflow: hidden;
            }
            
            .metric-card::before {
                content: '';
                position: absolute;
                top: 0;
                left: 0;
                right: 0;
                height: 3px;
                background: linear-gradient(90deg, #60a5fa, #a78bfa);
                transform: scaleX(0);
                transition: transform 0.3s ease;
            }
            
            .metric-card:hover::before {
                transform: scaleX(1);
            }
            
            .metric-card:hover {
                border-color: rgba(96, 165, 250, 0.5);
                transform: translateY(-4px);
                box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
                background: rgba(30, 41, 59, 0.9);
            }
            
            .metric-header {
                display: flex;
                justify-content: space-between;
                align-items: flex-start;
                margin-bottom: 12px;
            }
            
            .metric-icon {
                width: 40px;
                height: 40px;
                border-radius: 10px;
                display: flex;
                align-items: center;
                justify-content: center;
                font-size: 1.2rem;
                margin-bottom: 16px;
            }
            
            .metric-trend {
                display: flex;
                align-items: center;
                gap: 4px;
                font-size: 0.75rem;
                font-weight: 500;
            }
            
            .trend-up { color: #10b981; }
            .trend-down { color: #ef4444; }
            .trend-neutral { color: #94a3b8; }
            
            .metric-label {
                font-size: 0.875rem;
                color: #94a3b8;
                margin-bottom: 8px;
                text-transform: uppercase;
                letter-spacing: 0.05em;
            }
            
            .metric-value {
                font-size: 2rem;
                font-weight: 700;
                color: #f1f5f9;
                line-height: 1;
            }
            
            .metric-value.success { color: #10b981; }
            .metric-value.error { color: #ef4444; }
            .metric-value.warning { color: #f59e0b; }
            .metric-value.info { color: #60a5fa; }
            
            .charts-row {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(500px, 1fr));
                gap: 20px;
                margin-bottom: 30px;
            }
            
            .chart-container {
                background: #1e293b;
                border: 1px solid #334155;
                border-radius: 12px;
                padding: 24px;
                height: 400px;
                display: flex;
                flex-direction: column;
            }
            
            .chart-title {
                font-size: 1.25rem;
                font-weight: 600;
                margin-bottom: 20px;
                color: #f1f5f9;
            }
            
            .chart-wrapper {
                flex: 1;
                position: relative;
                min-height: 0;
            }
            
            canvas {
                position: absolute !important;
                top: 0;
                left: 0;
                width: 100% !important;
                height: 100% !important;
            }
            
            .endpoint-container {
                background: #1e293b;
                border: 1px solid #334155;
                border-radius: 12px;
                padding: 24px;
                max-height: 500px;
                overflow-y: auto;
            }
            
            .endpoint-container::-webkit-scrollbar {
                width: 8px;
            }
            
            .endpoint-container::-webkit-scrollbar-track {
                background: #0f172a;
                border-radius: 4px;
            }
            
            .endpoint-container::-webkit-scrollbar-thumb {
                background: #475569;
                border-radius: 4px;
            }
            
            .endpoint-container::-webkit-scrollbar-thumb:hover {
                background: #64748b;
            }
            
            .endpoint-item {
                display: flex;
                justify-content: space-between;
                align-items: center;
                padding: 16px;
                border-bottom: 1px solid #334155;
                transition: background-color 0.2s ease;
            }
            
            .endpoint-item:hover {
                background: #334155;
            }
            
            .endpoint-item:last-child {
                border-bottom: none;
            }
            
            .endpoint-name {
                font-family: 'Consolas', 'Monaco', monospace;
                color: #60a5fa;
            }
            
            .endpoint-stats {
                display: flex;
                gap: 20px;
                align-items: center;
            }
            
            .endpoint-count {
                font-weight: 600;
                color: #f1f5f9;
            }
            
            .endpoint-percentage {
                color: #94a3b8;
                font-size: 0.875rem;
            }
            
            .footer {
                text-align: center;
                color: #64748b;
                font-size: 0.875rem;
                margin-top: 40px;
                padding-top: 20px;
                border-top: 1px solid #334155;
            }
            
            .refresh-indicator {
                display: inline-flex;
                align-items: center;
                gap: 8px;
                color: #60a5fa;
            }
            
            .refresh-dot {
                width: 8px;
                height: 8px;
                background: #60a5fa;
                border-radius: 50%;
                animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
            }
            
            @media (max-width: 768px) {
                .charts-row {
                    grid-template-columns: 1fr;
                }
                
                .chart-container {
                    height: 300px;
                }
                
                h1 {
                    font-size: 1.75rem;
                }
                
                .metric-value {
                    font-size: 1.5rem;
                }
            }
        </style>
    </head>
    <body>
        <div class="theme-toggle">
            <button class="theme-btn active" data-theme="dark">
                <i class="fas fa-moon"></i>
            </button>
            <button class="theme-btn" data-theme="light">
                <i class="fas fa-sun"></i>
            </button>
        </div>
        
        <div class="alert-panel" id="alertPanel">
        </div>
        
        <div class="container">
            <div class="header">
                <div>
                    <h1>🚀 Server Dashboard</h1>
                    <div style="display: flex; gap: 12px; margin-top: 8px;">
                        <div class="status-badge">LIVE</div>
                        <div class="status-badge" style="background: #3b82f6;" id="connectionStatus">CONNECTED</div>
                        <div class="status-badge" style="background: #8b5cf6;" id="lastUpdate">Updated now</div>
                        <button id="alertToggle" style="background: #ef4444; color: white; border: none; border-radius: 20px; padding: 4px 12px; font-size: 0.875rem; cursor: pointer;">
                            <i class="fas fa-bell"></i> <span id="alertCount">0</span>
                        </button>
                    </div>
                </div>
                <div style="display: flex; gap: 12px; align-items: center;">
                    <select id="timeRange" style="background: var(--secondary-bg); color: var(--text-primary); border: 1px solid var(--border-color); border-radius: 8px; padding: 8px 12px;">
                        <option value="1h">Last Hour</option>
                        <option value="6h">Last 6 Hours</option>
                        <option value="24h">Last 24 Hours</option>
                        <option value="7d">Last 7 Days</option>
                    </select>
                    <button id="refreshBtn" style="background: var(--accent-color); color: white; border: none; border-radius: 8px; padding: 8px 16px; cursor: pointer; transition: all 0.2s;">
                        <i class="fas fa-sync-alt"></i> Refresh
                    </button>
                    <button id="exportBtn" style="background: var(--success-color); color: white; border: none; border-radius: 8px; padding: 8px 16px; cursor: pointer; transition: all 0.2s;">
                        <i class="fas fa-download"></i> Export
                    </button>
                </div>
            </div>
            
            <div class="system-map" id="systemMap">
                <h3 style="margin-bottom: 20px; color: var(--text-primary);">
                    <i class="fas fa-network-wired"></i> System Architecture
                </h3>
                <div style="position: relative; height: 200px;">
                </div>
            </div>
            
            <div class="analytics-panel">
                <h3 style="margin-bottom: 20px; color: var(--text-primary);">
                    <i class="fas fa-brain"></i> AI Insights & Predictions
                </h3>
                <div id="insightsContainer">
                </div>
            </div>
            
            <div class="metrics-grid" id="metricsGrid">
            </div>
            
            <div style="background: var(--secondary-bg); border: 1px solid var(--border-color); border-radius: 16px; padding: 24px; margin-bottom: 30px;">
                <h3 style="margin-bottom: 20px; color: var(--text-primary);">
                    <i class="fas fa-fire"></i> 24-Hour Performance Heatmap
                </h3>
                <div class="performance-heatmap" id="performanceHeatmap">
                </div>
                <div style="display: flex; justify-content: space-between; margin-top: 12px; font-size: 0.75rem; color: var(--text-secondary);">
                    <span>00:00</span>
                    <span>06:00</span>
                    <span>12:00</span>
                    <span>18:00</span>
                    <span>24:00</span>
                </div>
            </div>
            
            <div class="live-requests">
                <h3 style="margin-bottom: 20px; color: var(--text-primary);">
                    <i class="fas fa-stream"></i> Live Request Flow
                </h3>
                <div id="requestFlow" style="position: relative; height: 200px;">
                </div>
            </div>
            
            <div class="charts-row">
                <div class="chart-container">
                    <div class="chart-title">Response Time Trend (Last 20 Requests)</div>
                    <div class="chart-wrapper">
                        <canvas id="responseTimeChart"></canvas>
                    </div>
                </div>
                
                <div class="chart-container">
                    <div class="chart-title">Request Methods Distribution</div>
                    <div class="chart-wrapper">
                        <canvas id="methodChart"></canvas>
                    </div>
                </div>
            </div>
            
            <div class="endpoint-container">
                <div class="chart-title">Top Endpoints</div>
                <div id="endpointsList"></div>
            </div>
            
            <div class="footer">
                <div class="refresh-indicator">
                    <div class="refresh-dot"></div>
                    <span>Auto-refreshing every 2 seconds</span>
                </div>
            </div>
        </div>

        <script>
        Chart.defaults.color = '#94a3b8';
        Chart.defaults.borderColor = '#334155';
        
        const responseTimeCtx = document.getElementById('responseTimeChart').getContext('2d');
        const responseTimeChart = new Chart(responseTimeCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Response Time (ms)',
                    data: [],
                    borderColor: '#60a5fa',
                    backgroundColor: 'rgba(96, 165, 250, 0.1)',
                    borderWidth: 2,
                    tension: 0.4,
                    pointRadius: 4,
                    pointHoverRadius: 6,
                    pointBackgroundColor: '#60a5fa',
                    pointBorderColor: '#1e293b',
                    pointBorderWidth: 2
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                interaction: {
                    mode: 'index',
                    intersect: false,
                },
                plugins: {
                    legend: {
                        display: false
                    },
                    tooltip: {
                        backgroundColor: '#1e293b',
                        titleColor: '#f1f5f9',
                        bodyColor: '#e2e8f0',
                        borderColor: '#334155',
                        borderWidth: 1,
                        padding: 12,
                        displayColors: false
                    }
                },
                scales: {
                    x: {
                        grid: {
                            color: '#1e293b',
                            drawBorder: false
                        },
                        ticks: {
                            maxRotation: 0,
                            autoSkip: true,
                            maxTicksLimit: 10
                        }
                    },
                    y: {
                        beginAtZero: true,
                        grid: {
                            color: '#1e293b',
                            drawBorder: false
                        },
                        title: {
                            display: true,
                            text: 'Response Time (ms)',
                            color: '#94a3b8'
                        }
                    }
                }
            }
        });

        const methodCtx = document.getElementById('methodChart').getContext('2d');
        const methodChart = new Chart(methodCtx, {
            type: 'doughnut',
            data: {
                labels: [],
                datasets: [{
                    data: [],
                    backgroundColor: [
                        '#60a5fa',
                        '#10b981',
                        '#f59e0b',
                        '#ef4444',
                        '#a78bfa',
                        '#ec4899',
                        '#14b8a6'
                    ],
                    borderColor: '#1e293b',
                    borderWidth: 3
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: 'right',
                        labels: {
                            padding: 15,
                            font: {
                                size: 12
                            }
                        }
                    },
                    tooltip: {
                        backgroundColor: '#1e293b',
                        titleColor: '#f1f5f9',
                        bodyColor: '#e2e8f0',
                        borderColor: '#334155',
                        borderWidth: 1,
                        padding: 12
                    }
                }
            }
        });

        let alerts = [];
        let currentTheme = 'dark';
        
        function initTheme() {
            const savedTheme = localStorage.getItem('dashboard-theme') || 'dark';
            setTheme(savedTheme);
        }
        
        function setTheme(theme) {
            currentTheme = theme;
            document.documentElement.setAttribute('data-theme', theme);
            localStorage.setItem('dashboard-theme', theme);
            
            document.querySelectorAll('.theme-btn').forEach(btn => {
                btn.classList.toggle('active', btn.dataset.theme === theme);
            });
        }
        
        function addAlert(type, title, message) {
            const alert = {
                id: Date.now(),
                type,
                title,
                message,
                timestamp: new Date()
            };
            
            alerts.unshift(alert);
            if (alerts.length > 10) alerts.pop();
            
            updateAlertPanel();
            updateAlertCount();
        }
        
        function updateAlertPanel() {
            const panel = document.getElementById('alertPanel');
            panel.innerHTML = alerts.map(alert => `
                <div class="alert-item ${alert.type}">
                    <div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 8px;">
                        <strong style="color: var(--text-primary);">${alert.title}</strong>
                        <small style="color: var(--text-secondary);">${formatTime(alert.timestamp)}</small>
                    </div>
                    <p style="margin: 0; color: var(--text-secondary); font-size: 0.875rem;">${alert.message}</p>
                </div>
            `).join('');
        }
        
        function updateAlertCount() {
            document.getElementById('alertCount').textContent = alerts.length;
        }
        
        function formatTime(date) {
            return date.toLocaleTimeString('en-US', { 
                hour: '2-digit', 
                minute: '2-digit' 
            });
        }
        
        function initSystemMap() {
            const mapContainer = document.querySelector('#systemMap > div');
            const nodes = [
                { id: 'api', label: '🌐', x: 50, y: 50, status: 'healthy' },
                { id: 'db', label: '🗄️', x: 200, y: 50, status: 'healthy' },
                { id: 'cache', label: '⚡', x: 350, y: 50, status: 'healthy' },
                { id: 'auth', label: '🔐', x: 125, y: 150, status: 'healthy' },
                { id: 'files', label: '📁', x: 275, y: 150, status: 'healthy' }
            ];
            
            mapContainer.innerHTML = nodes.map(node => `
                <div class="system-node" 
                     style="left: ${node.x}px; top: ${node.y}px; background: ${getNodeColor(node.status)};"
                     title="${node.id.toUpperCase()} - ${node.status}"
                     data-node="${node.id}">
                    ${node.label}
                </div>
            `).join('');
        }
        
        function getNodeColor(status) {
            switch(status) {
                case 'healthy': return 'linear-gradient(135deg, #10b981, #059669)';
                case 'warning': return 'linear-gradient(135deg, #f59e0b, #d97706)';
                case 'error': return 'linear-gradient(135deg, #ef4444, #dc2626)';
                default: return 'linear-gradient(135deg, #6b7280, #4b5563)';
            }
        }
        
        function initPerformanceHeatmap() {
            const heatmapContainer = document.getElementById('performanceHeatmap');
            const hours = 24;
            const cells = [];
            
            for (let i = 0; i < hours; i++) {
                const intensity = Math.random();
                const color = getHeatmapColor(intensity);
                cells.push(`
                    <div class="heatmap-cell" 
                         style="background: ${color};" 
                         title="Hour ${i}:00 - ${(intensity * 100).toFixed(1)}% load"
                         data-hour="${i}"
                         data-intensity="${intensity}">
                    </div>
                `);
            }
            
            heatmapContainer.innerHTML = cells.join('');
        }
        
        function getHeatmapColor(intensity) {
            const colors = [
                'rgba(16, 185, 129, 0.2)',
                'rgba(245, 158, 11, 0.4)',
                'rgba(239, 68, 68, 0.6)'
            ];
            
            if (intensity < 0.3) return colors[0];
            if (intensity < 0.7) return colors[1];
            return colors[2];
        }
        
        function generateInsights(metrics) {
            const insights = [];
            
            if (metrics.error_rate > 5) {
                insights.push({
                    type: 'warning',
                    title: 'High Error Rate Detected',
                    message: `Error rate is ${metrics.error_rate.toFixed(1)}%. Consider investigating recent deployments.`
                });
            }
            
            if (metrics.average_response_time_ms > 1000) {
                insights.push({
                    type: 'performance',
                    title: 'Response Time Alert',
                    message: `Average response time is ${metrics.average_response_time_ms.toFixed(0)}ms. Database optimization recommended.`
                });
            }
            
            if (metrics.requests_per_second > 10) {
                insights.push({
                    type: 'success',
                    title: 'High Traffic Volume',
                    message: `Handling ${metrics.requests_per_second.toFixed(1)} req/s. System performing well under load.`
                });
            }
            
            const container = document.getElementById('insightsContainer');
            container.innerHTML = insights.map(insight => `
                <div class="insight-card">
                    <h4 style="margin: 0 0 8px 0; color: var(--text-primary);">${insight.title}</h4>
                    <p style="margin: 0; color: var(--text-secondary); font-size: 0.875rem;">${insight.message}</p>
                </div>
            `).join('') || '<p style="color: var(--text-secondary); font-style: italic;">All systems operating normally. No insights to display.</p>';
        }
        
        function animateRequestFlow() {
            const container = document.getElementById('requestFlow');
            const request = document.createElement('div');
            request.className = 'request-flow';
            request.style.left = '0px';
            request.style.top = Math.random() * 180 + 'px';
            
            container.appendChild(request);
            
            setTimeout(() => {
                if (request.parentNode) {
                    request.parentNode.removeChild(request);
                }
            }, 3000);
        }
        
        function exportDashboard() {
            const data = {
                timestamp: new Date().toISOString(),
                metrics: window.currentMetrics || {},
                alerts: alerts,
                theme: currentTheme
            };
            
            const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `dashboard-export-${new Date().toISOString().split('T')[0]}.json`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
            
            addAlert('success', 'Export Complete', 'Dashboard data exported successfully');
        }

        async function updateDashboard() {
            try {
                updateConnectionStatus(false);
                const response = await fetch('/api/metrics');
                
                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
                }
                
                const result = await response.json();
                const metrics = result.data;
                
                updateConnectionStatus(true);
                
                const prevMetrics = window.prevMetrics || {};
                window.currentMetrics = metrics;
                window.prevMetrics = metrics;
                
                generateInsights(metrics);
                
                if (metrics.error_rate > 5 && (!prevMetrics.error_rate || prevMetrics.error_rate <= 5)) {
                    addAlert('error', 'High Error Rate', `Error rate spiked to ${metrics.error_rate.toFixed(1)}%`);
                }
                
                if (metrics.average_response_time_ms > 1000 && (!prevMetrics.average_response_time_ms || prevMetrics.average_response_time_ms <= 1000)) {
                    addAlert('warning', 'Slow Response Time', `Response time increased to ${metrics.average_response_time_ms.toFixed(0)}ms`);
                }
                
                document.getElementById('metricsGrid').innerHTML = `
                    <div class="metric-card">
                        <div class="metric-icon" style="background: linear-gradient(135deg, #60a5fa, #3b82f6);">📊</div>
                        <div class="metric-header">
                            <div class="metric-label">Total Requests</div>
                            <div class="metric-trend ${getTrendClass(metrics.total_requests, prevMetrics.total_requests)}">
                                ${getTrendIcon(metrics.total_requests, prevMetrics.total_requests)}
                                ${getTrendText(metrics.total_requests, prevMetrics.total_requests)}
                            </div>
                        </div>
                        <div class="metric-value">${metrics.total_requests.toLocaleString()}</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-icon" style="background: linear-gradient(135deg, #10b981, #059669);">✅</div>
                        <div class="metric-header">
                            <div class="metric-label">Success Rate</div>
                            <div class="metric-trend ${getTrendClass(100 - metrics.error_rate, 100 - (prevMetrics.error_rate || 0), true)}">
                                ${getTrendIcon(100 - metrics.error_rate, 100 - (prevMetrics.error_rate || 0), true)}
                                ${((100 - metrics.error_rate) - (100 - (prevMetrics.error_rate || 0))).toFixed(1)}%
                            </div>
                        </div>
                        <div class="metric-value success">${(100 - metrics.error_rate).toFixed(1)}%</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-icon" style="background: linear-gradient(135deg, ${metrics.error_rate > 5 ? '#ef4444, #dc2626' : metrics.error_rate > 0 ? '#f59e0b, #d97706' : '#10b981, #059669'});">${metrics.error_rate > 5 ? '❌' : metrics.error_rate > 0 ? '⚠️' : '✅'}</div>
                        <div class="metric-header">
                            <div class="metric-label">Error Rate</div>
                            <div class="metric-trend ${getTrendClass(metrics.error_rate, prevMetrics.error_rate || 0, false)}">
                                ${getTrendIcon(metrics.error_rate, prevMetrics.error_rate || 0, false)}
                                ${(metrics.error_rate - (prevMetrics.error_rate || 0)).toFixed(1)}%
                            </div>
                        </div>
                        <div class="metric-value ${metrics.error_rate > 5 ? 'error' : metrics.error_rate > 0 ? 'warning' : 'success'}">${metrics.error_rate.toFixed(1)}%</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-icon" style="background: linear-gradient(135deg, #60a5fa, #3b82f6);">⚡</div>
                        <div class="metric-header">
                            <div class="metric-label">Avg Response Time</div>
                            <div class="metric-trend ${getTrendClass(metrics.average_response_time_ms, prevMetrics.average_response_time_ms || 0, false)}">
                                ${getTrendIcon(metrics.average_response_time_ms, prevMetrics.average_response_time_ms || 0, false)}
                                ${(metrics.average_response_time_ms - (prevMetrics.average_response_time_ms || 0)).toFixed(0)}ms
                            </div>
                        </div>
                        <div class="metric-value info">${metrics.average_response_time_ms.toFixed(0)}ms</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-icon" style="background: linear-gradient(135deg, #a78bfa, #8b5cf6);">🚀</div>
                        <div class="metric-header">
                            <div class="metric-label">Requests/Second</div>
                            <div class="metric-trend ${getTrendClass(metrics.requests_per_second, prevMetrics.requests_per_second || 0)}">
                                ${getTrendIcon(metrics.requests_per_second, prevMetrics.requests_per_second || 0)}
                                ${(metrics.requests_per_second - (prevMetrics.requests_per_second || 0)).toFixed(2)}
                            </div>
                        </div>
                        <div class="metric-value">${metrics.requests_per_second.toFixed(2)}</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-icon" style="background: linear-gradient(135deg, #10b981, #059669);">⏱️</div>
                        <div class="metric-header">
                            <div class="metric-label">Uptime</div>
                            <div class="metric-trend trend-up">
                                ↗️ ${formatUptime(metrics.uptime_seconds - (prevMetrics.uptime_seconds || 0))}
                            </div>
                        </div>
                        <div class="metric-value">${formatUptime(metrics.uptime_seconds)}</div>
                    </div>
                `;
                
                const timeLabels = metrics.last_hour_response_times
                    .slice(-20)
                    .map(rt => new Date(rt.timestamp).toLocaleTimeString());
                const timeData = metrics.last_hour_response_times
                    .slice(-20)
                    .map(rt => rt.duration_ms);
                
                responseTimeChart.data.labels = timeLabels;
                responseTimeChart.data.datasets[0].data = timeData;
                responseTimeChart.update('none');
                
                const methods = Object.entries(metrics.requests_by_method);
                methodChart.data.labels = methods.map(([method]) => method);
                methodChart.data.datasets[0].data = methods.map(([, count]) => count);
                methodChart.update('none');
                
                const endpointsList = metrics.requests_by_endpoint
                    .slice(0, 10)
                    .map(ep => `
                        <div class="endpoint-item">
                            <span class="endpoint-name">${ep.endpoint}</span>
                            <div class="endpoint-stats">
                                <span class="endpoint-count">${ep.count.toLocaleString()}</span>
                                <span class="endpoint-percentage">${ep.percentage.toFixed(1)}%</span>
                            </div>
                        </div>
                    `).join('');
                
                document.getElementById('endpointsList').innerHTML = endpointsList || '<div class="endpoint-item">No endpoints accessed yet</div>';
                
            } catch (error) {
                console.error('Failed to update dashboard:', error);
                updateConnectionStatus(false);
                
                const errorNotification = document.createElement('div');
                errorNotification.style.cssText = `
                    position: fixed;
                    top: 20px;
                    right: 20px;
                    background: #ef4444;
                    color: white;
                    padding: 12px 20px;
                    border-radius: 8px;
                    z-index: 1000;
                    animation: slideIn 0.3s ease;
                `;
                errorNotification.textContent = `Connection error: ${error.message}`;
                document.body.appendChild(errorNotification);
                
                setTimeout(() => {
                    errorNotification.remove();
                }, 5000);
            }
        }

        function formatUptime(seconds) {
            const days = Math.floor(seconds / 86400);
            const hours = Math.floor((seconds % 86400) / 3600);
            const minutes = Math.floor((seconds % 3600) / 60);
            const secs = seconds % 60;
            
            if (days > 0) {
                return `${days}d ${hours}h`;
            } else if (hours > 0) {
                return `${hours}h ${minutes}m`;
            } else if (minutes > 0) {
                return `${minutes}m ${secs}s`;
            } else {
                return `${secs}s`;
            }
        }
        
        function getTrendClass(current, previous, higherIsBetter = true) {
            if (!previous || current === previous) return 'trend-neutral';
            const isIncreasing = current > previous;
            if (higherIsBetter) {
                return isIncreasing ? 'trend-up' : 'trend-down';
            } else {
                return isIncreasing ? 'trend-down' : 'trend-up';
            }
        }
        
        function getTrendIcon(current, previous, higherIsBetter = true) {
            if (!previous || current === previous) return '➡️';
            const isIncreasing = current > previous;
            if (higherIsBetter) {
                return isIncreasing ? '↗️' : '↘️';
            } else {
                return isIncreasing ? '↘️' : '↗️';
            }
        }
        
        function getTrendText(current, previous) {
            if (!previous) return 'N/A';
            const diff = current - previous;
            return diff >= 0 ? `+${diff}` : `${diff}`;
        }
        
        function updateConnectionStatus(isConnected) {
            const statusEl = document.getElementById('connectionStatus');
            const lastUpdateEl = document.getElementById('lastUpdate');
            
            if (isConnected) {
                statusEl.textContent = 'CONNECTED';
                statusEl.style.background = '#10b981';
                lastUpdateEl.textContent = 'Updated now';
                lastUpdateEl.style.background = '#8b5cf6';
            } else {
                statusEl.textContent = 'DISCONNECTED';
                statusEl.style.background = '#ef4444';
                lastUpdateEl.textContent = 'Connection lost';
                lastUpdateEl.style.background = '#6b7280';
            }
        }

        function cleanupChartData() {
            if (responseTimeChart.data.labels.length > 20) {
                responseTimeChart.data.labels = responseTimeChart.data.labels.slice(-20);
                responseTimeChart.data.datasets[0].data = responseTimeChart.data.datasets[0].data.slice(-20);
            }
        }

        const style = document.createElement('style');
        style.textContent = `
            @keyframes slideIn {
                from { transform: translateX(100%); opacity: 0; }
                to { transform: translateX(0); opacity: 1; }
            }
            @keyframes countUp {
                from { transform: scale(0.8); opacity: 0; }
                to { transform: scale(1); opacity: 1; }
            }
            .metric-value {
                animation: countUp 0.5s ease;
            }
        `;
        document.head.appendChild(style);
        
        document.getElementById('refreshBtn').addEventListener('click', () => {
            updateDashboard();
            document.getElementById('refreshBtn').style.transform = 'rotate(360deg)';
            setTimeout(() => {
                document.getElementById('refreshBtn').style.transform = 'rotate(0deg)';
            }, 500);
        });
        
        document.getElementById('exportBtn').addEventListener('click', exportDashboard);
        
        document.getElementById('alertToggle').addEventListener('click', () => {
            const panel = document.getElementById('alertPanel');
            panel.style.display = panel.style.display === 'none' ? 'block' : 'none';
        });
        
        document.querySelectorAll('.theme-btn').forEach(btn => {
            btn.addEventListener('click', () => setTheme(btn.dataset.theme));
        });
        
        document.getElementById('timeRange').addEventListener('change', (e) => {
            console.log('Time range changed to:', e.target.value);
            addAlert('info', 'Time Range Changed', `Switched to ${e.target.value} view`);
        });
        
        initTheme();
        initSystemMap();
        initPerformanceHeatmap();
        updateDashboard();
        
        setInterval(animateRequestFlow, 1000);
        
        let refreshInterval = 2000;
        let errorCount = 0;
        
        const autoRefresh = () => {
            updateDashboard().then(() => {
                errorCount = 0;
                refreshInterval = 2000;
            }).catch(() => {
                errorCount++;
                refreshInterval = Math.min(refreshInterval * 1.5, 30000);
            });
            
            cleanupChartData();
            setTimeout(autoRefresh, refreshInterval);
        };
        
        setTimeout(autoRefresh, 2000);

        const resizeObserver = new ResizeObserver(entries => {
            for (let entry of entries) {
                if (entry.target.querySelector('canvas')) {
                    Chart.getChart(entry.target.querySelector('canvas'))?.resize();
                }
            }
        });

        document.querySelectorAll('.chart-wrapper').forEach(wrapper => {
            resizeObserver.observe(wrapper);
        });
        </script>
    </body>
    </html>
    "#)
}

async fn handle_export_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Query(params): Query<ItemExportQuery>,
) -> Result<impl IntoResponse> {

    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = params.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Export query validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    let format = params.format.as_deref().unwrap_or("json");
    let items = state.item_service.get_items(None, None).await?;
    
    match format {
        "csv" => {
            let mut csv = String::from("id,name,description,tags,created_at,updated_at\n");
            for item in items {
                csv.push_str(&format!(
                    "{},{},\"{}\",\"{}\",{},{}\n",
                    item.id,
                    item.name,
                    item.description.unwrap_or_default().replace("\"", "\"\""),
                    item.tags.join(";"),
                    item.created_at.to_rfc3339(),
                    item.updated_at.to_rfc3339()
                ));
            }
            Ok((
                StatusCode::OK,
                [
                    ("Content-Type", "text/csv"),
                    ("Content-Disposition", "attachment; filename=\"items_export.csv\"")
                ],
                csv,
            ).into_response())
        },
        "yaml" => {
            let yaml = serde_yaml::to_string(&items)
                .map_err(|e| AppError::Other(anyhow::anyhow!("Failed to serialize to YAML: {}", e)))?;
            Ok((
                StatusCode::OK,
                [
                    ("Content-Type", "text/yaml"),
                    ("Content-Disposition", "attachment; filename=\"items_export.yaml\"")
                ],
                yaml,
            ).into_response())
        },
        _ => {
            let json = serde_json::to_string_pretty(&items)?;
            Ok((
                StatusCode::OK,
                [
                    ("Content-Type", "application/json"),
                    ("Content-Disposition", "attachment; filename=\"items_export.json\"")
                ],
                json,
            ).into_response())
        }
    }
}

fn create_auth_routes_with_middleware() -> Router<AppState> {
    use crate::handlers::auth::{
        register_user, login_user, refresh_token, logout_user, 
        get_current_user, get_user_by_id
    };
    use axum::routing::{get, post};
    use axum::middleware;

    let protected_routes = Router::new()
        .route("/me", get(get_current_user))
        .route("/users/:id", get(get_user_by_id))
        .layer(middleware::from_fn_with_state(
            crate::AppState::default(),
            crate::middleware::auth::jwt_auth_middleware,
        ));

    Router::new()
        .route("/register", post(register_user))
        .route("/login", post(login_user))
        .route("/refresh", post(refresh_token))
        .route("/logout", post(logout_user))
        .merge(protected_routes)
}

fn create_file_routes() -> Router<AppState> {
    use axum::routing::{delete, get, post};

    Router::new()
        .route("/upload", post(files::upload_file))
        .route("/:id/serve", get(files::serve_file))
        .route("/:id/info", get(files::get_file_info))
        .route("/:id/download", get(files::download_file))
        .route("/:id", delete(files::delete_file))
        .route("/:id/associate", post(files::associate_file_with_item))
        .route("/", get(files::list_files))
        .route("/item/:id", get(files::get_item_files))
}

fn create_job_routes() -> Router<AppState> {
    use crate::handlers::jobs;
    use axum::routing::{delete, get, post};

    Router::new()
        .route("/", post(jobs::submit_job).get(jobs::list_jobs))
        .route("/stats", get(jobs::get_queue_stats))
        .route("/cleanup", post(jobs::cleanup_jobs))
        .route("/bulk-import", post(jobs::submit_bulk_import))
        .route("/bulk-export", post(jobs::submit_bulk_export))
        .route("/:id", get(jobs::get_job))
        .route("/:id/status", get(jobs::get_job_status))
        .route("/:id/cancel", delete(jobs::cancel_job))
        .route("/:id/retry", post(jobs::retry_job))
}

fn create_cache_routes() -> Router<AppState> {
    use crate::handlers::cache;
    
    Router::new()
        .route("/stats", get(cache::get_cache_stats))
        .route("/health", get(cache::get_cache_health))
        .route("/clear", axum::routing::post(cache::clear_cache))
        .route("/invalidate", axum::routing::post(cache::invalidate_cache_pattern))
}

async fn handle_get_items_v2(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Query(mut params): Query<ItemListQuery>
) -> Result<impl IntoResponse> {
    info!("GET /api/v2/items - enhanced version");
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = params.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Query validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    let page_size = params.page_size.unwrap_or(50) as usize;
    let page = params.page.unwrap_or(1);
    let offset = ((page - 1) * page_size as u32) as usize;
    
    let items = state.item_service.get_items(Some(page_size), Some(offset)).await?;
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "items": items,
        "count": items.len(),
        "page_size": page_size,
        "page": page,
        "offset": offset,
        "api_version": "2.0",
        "enhanced_features": true,
        "source": if state.item_service.is_using_database() { "database" } else { "memory" },
        "include_files": params.include_files.unwrap_or(false)
    }))))
}

async fn handle_get_item_v2(
    State(state): State<AppState>,
    Path(id): Path<u64>
) -> Result<impl IntoResponse> {
    info!("GET /api/v2/items/{} - enhanced version", id);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }

    let item = state.item_service.get_item(id).await?;
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "item": item,
        "api_version": "2.0",
        "enhanced_features": true,
        "metadata": {
            "retrieved_at": chrono::Utc::now().to_rfc3339(),
            "source": if state.item_service.is_using_database() { "database" } else { "memory" }
        }
    }))))
}

async fn handle_post_item_v2(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Json(payload): Json<CreateItemRequest>
) -> Result<impl IntoResponse> {
    info!("POST /api/v2/items - enhanced version - name: {}", payload.name);
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = payload.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }

    let item = state.item_service.create_item(
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    ).await?;

    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }

    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemCreated(item.clone());
        ws_manager.broadcast(event).await;
    }

    Ok((StatusCode::CREATED, Json(ApiResponse::success(serde_json::json!({
        "item": item,
        "api_version": "2.0",
        "enhanced_features": true,
        "created_at": chrono::Utc::now().to_rfc3339()
    })))))
}

async fn handle_put_item_v2(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Json(payload): Json<CreateItemRequest>,
) -> Result<impl IntoResponse> {
    info!("PUT /api/v2/items/{} - enhanced version - name: {}", id, payload.name);
    
    if id == 0 {
        return Err(AppError::BadRequest("Invalid item ID".to_string()));
    }
    
    let addr = connect_info.map(|ci| ci.0).unwrap_or_else(|| {
        std::net::SocketAddr::from(([127, 0, 0, 1], 8080))
    });
    let context = extract_validation_context(&headers, &addr, None, None);
    
    let validation_result = payload.validate_with_context(&context);
    if !validation_result.is_valid {
        return Err(AppError::Validation(format!(
            "Validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }

    let item = state.item_service.update_item(
        id,
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    ).await?;

    if let Some(cache_manager) = &state.cache_manager {
        cache_manager.invalidate_item_cache(id);
        cache_manager.invalidate_items_cache();
        cache_manager.invalidate_search_cache();
    }

    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemUpdated(item.clone());
        ws_manager.broadcast(event).await;
    }

    Ok(Json(ApiResponse::success(serde_json::json!({
        "item": item,
        "api_version": "2.0",
        "enhanced_features": true,
        "updated_at": chrono::Utc::now().to_rfc3339()
    }))))
}
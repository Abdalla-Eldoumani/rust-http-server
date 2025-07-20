//! HTTP route handlers for all standard methods

use crate::{
    error::{AppError, Result},
    models::request::{ApiResponse, FormPayload},
    AppState,
};
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Html},
    routing::get,
    Json, Router,
};
use serde::{Deserialize};
use std::collections::HashMap;
use tracing::info;

pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handle_root))
        .route("/health", get(handle_health))
        .route("/dashboard", get(handle_dashboard))
        .route("/api/stats", get(handle_stats))
        .route("/api/metrics", get(handle_metrics))
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
        .route("/api/form", axum::routing::post(handle_form_submit))
        .route("/api/head", axum::routing::head(handle_head))
        .route("/api/options", axum::routing::options(handle_options))
        .route("/ws", axum::routing::get(crate::websocket::websocket_handler))
        .nest("/auth", create_auth_routes_with_middleware())
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

    Json(ApiResponse::success(serde_json::json!({
        "app": state.app_name,
        "version": state.version,
        "message": "Welcome to the Rust HTTP Server",
        "authentication_enabled": state.auth_service.is_some(),
        "websocket_enabled": state.websocket_manager.is_some(),
        "endpoints": endpoints
    })))
}

async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.item_service.get_stats().await.unwrap_or_default();
    
    let mut health_info = serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().timestamp(),
        "store_stats": stats,
        "using_database": state.item_service.is_using_database()
    });

    if let Some(db_manager) = &state.db_manager {
        match db_manager.health_check().await {
            Ok(_) => {
                health_info["database_status"] = serde_json::Value::String("healthy".to_string());
            }
            Err(e) => {
                health_info["database_status"] = serde_json::Value::String("unhealthy".to_string());
                health_info["database_error"] = serde_json::Value::String(e.to_string());
            }
        }
    }
    
    Json(ApiResponse::success(health_info))
}

async fn handle_stats(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let stats = state.item_service.get_stats().await?;
    Ok(Json(ApiResponse::success(stats)))
}

#[derive(Debug, Deserialize)]
struct ItemsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

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

async fn handle_search_items(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>
) -> Result<impl IntoResponse> {
    info!("GET /api/items/search - query: {:?}", params);
    
    let search_engine = state.search_engine.as_ref().ok_or_else(|| AppError::BadRequest("Search functionality not available".to_string()))?;
    
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
    
    let search_result = search_engine.search(&search_query).await?;
    
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
    Query(params): Query<ItemsQuery>
) -> Result<impl IntoResponse> {
    info!("GET /api/items - limit: {:?}, offset: {:?}", params.limit, params.offset);
    
    let items = state.item_service.get_items(params.limit, params.offset).await?;
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "items": items,
        "count": items.len(),
        "limit": params.limit,
        "offset": params.offset.unwrap_or(0),
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

    let item = state.item_service.create_item(
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    ).await?;

    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemCreated(item.clone());
        ws_manager.broadcast(event).await;
    }

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

    let item = state.item_service.update_item(
        id,
        payload.name,
        payload.description,
        payload.tags.unwrap_or_default(),
        payload.metadata
    ).await?;

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
    
    if let Some(ws_manager) = &state.websocket_manager {
        let event = crate::websocket::WebSocketEvent::ItemUpdated(item.clone());
        ws_manager.broadcast(event).await;
    }
    
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
    
    let item = state.item_service.create_item(
        item_name,
        Some(format!("Submitted by {} ({})", form.name, form.email)),
        vec!["form-submission".to_string()],
        Some(metadata)
    ).await?;

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

async fn handle_dashboard() -> impl IntoResponse {
    Html(r#"
    <!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Rust HTTP Server - Dashboard</title>
        <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
        <style>
            * {
                box-sizing: border-box;
                margin: 0;
                padding: 0;
            }
            
            body {
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
                background: #0f172a;
                color: #e2e8f0;
                min-height: 100vh;
                overflow-x: hidden;
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
                background: #1e293b;
                border: 1px solid #334155;
                border-radius: 12px;
                padding: 24px;
                transition: all 0.3s ease;
            }
            
            .metric-card:hover {
                border-color: #60a5fa;
                transform: translateY(-2px);
                box-shadow: 0 10px 20px rgba(0, 0, 0, 0.3);
            }
            
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
        <div class="container">
            <div class="header">
                <h1>🚀 Rust HTTP Server Dashboard</h1>
                <div class="status-badge">LIVE</div>
            </div>
            
            <div class="metrics-grid" id="metricsGrid">
                <!-- Metrics will be populated here -->
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

        async function updateDashboard() {
            try {
                const response = await fetch('/api/metrics');
                const result = await response.json();
                const metrics = result.data;
                
                document.getElementById('metricsGrid').innerHTML = `
                    <div class="metric-card">
                        <div class="metric-label">Total Requests</div>
                        <div class="metric-value">${metrics.total_requests.toLocaleString()}</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Success Rate</div>
                        <div class="metric-value success">${(100 - metrics.error_rate).toFixed(1)}%</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Error Rate</div>
                        <div class="metric-value ${metrics.error_rate > 5 ? 'error' : metrics.error_rate > 0 ? 'warning' : 'success'}">${metrics.error_rate.toFixed(1)}%</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Avg Response Time</div>
                        <div class="metric-value info">${metrics.average_response_time_ms.toFixed(0)}ms</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Requests/Second</div>
                        <div class="metric-value">${metrics.requests_per_second.toFixed(2)}</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Uptime</div>
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

        function cleanupChartData() {
            if (responseTimeChart.data.labels.length > 20) {
                responseTimeChart.data.labels = responseTimeChart.data.labels.slice(-20);
                responseTimeChart.data.datasets[0].data = responseTimeChart.data.datasets[0].data.slice(-20);
            }
        }

        updateDashboard();
        setInterval(() => {
            updateDashboard();
            cleanupChartData();
        }, 2000);

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

async fn handle_metrics(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let items = state.item_service.get_items(None, None).await?;
    let item_count = items.len();
    let snapshot = state.metrics.get_snapshot(item_count);
    
    Ok(Json(ApiResponse::success(snapshot)))
}

async fn handle_export_items(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let format = params.get("format").map(|s| s.as_str()).unwrap_or("json");
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

    Router::new()
        .route("/register", post(register_user))
        .route("/login", post(login_user))
        .route("/refresh", post(refresh_token))
        .route("/logout", post(logout_user))
        .route("/me", get(get_current_user))
        .route("/users/:id", get(get_user_by_id))
}
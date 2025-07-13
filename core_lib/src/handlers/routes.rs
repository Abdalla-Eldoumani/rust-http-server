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
    response::{IntoResponse, Html},
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
        .route("/dashboard", get(handle_dashboard))
        .route("/api/stats", get(handle_stats))
        .route("/api/metrics", get(handle_metrics))
        .route("/api/items", get(handle_get_items).post(handle_post_item))
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
            body {
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
                margin: 0;
                padding: 20px;
                background: #f5f5f5;
            }
            .container {
                max-width: 1200px;
                margin: 0 auto;
            }
            h1 {
                color: #333;
                margin-bottom: 30px;
            }
            .metrics-grid {
                display: grid;
                grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
                gap: 20px;
                margin-bottom: 30px;
            }
            .metric-card {
                background: white;
                border-radius: 8px;
                padding: 20px;
                box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            }
            .metric-label {
                font-size: 14px;
                color: #666;
                margin-bottom: 5px;
            }
            .metric-value {
                font-size: 32px;
                font-weight: bold;
                color: #333;
            }
            .metric-value.success { color: #10b981; }
            .metric-value.error { color: #ef4444; }
            .metric-value.warning { color: #f59e0b; }
            .chart-container {
                background: white;
                border-radius: 8px;
                padding: 20px;
                box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                margin-bottom: 20px;
            }
            .chart-title {
                font-size: 18px;
                font-weight: bold;
                margin-bottom: 15px;
                color: #333;
            }
            #responseTimeChart {
                max-height: 300px;
            }
            .endpoint-list {
                background: white;
                border-radius: 8px;
                padding: 20px;
                box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            }
            .endpoint-item {
                display: flex;
                justify-content: space-between;
                padding: 10px 0;
                border-bottom: 1px solid #eee;
            }
            .endpoint-item:last-child {
                border-bottom: none;
            }
            .refresh-info {
                text-align: center;
                color: #666;
                font-size: 14px;
                margin-top: 20px;
            }
        </style>
    </head>
    <body>
        <div class="container">
            <h1>🚀 Rust HTTP Server Dashboard</h1>
            
            <div class="metrics-grid" id="metricsGrid">
                <!-- Metrics will be populated here -->
            </div>
            
            <div class="chart-container">
                <div class="chart-title">Response Time Trend (Last Hour)</div>
                <canvas id="responseTimeChart"></canvas>
            </div>
            
            <div class="chart-container">
                <div class="chart-title">Request Methods Distribution</div>
                <canvas id="methodChart"></canvas>
            </div>
            
            <div class="endpoint-list">
                <div class="chart-title">Top Endpoints</div>
                <div id="endpointsList"></div>
            </div>
            
            <div class="refresh-info">Auto-refreshing every 2 seconds...</div>
        </div>

        <script>
        // Initialize charts
        const responseTimeCtx = document.getElementById('responseTimeChart').getContext('2d');
        const responseTimeChart = new Chart(responseTimeCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Response Time (ms)',
                    data: [],
                    borderColor: '#3b82f6',
                    backgroundColor: 'rgba(59, 130, 246, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Response Time (ms)'
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
                        '#3b82f6',
                        '#10b981',
                        '#f59e0b',
                        '#ef4444',
                        '#8b5cf6',
                        '#ec4899',
                        '#14b8a6'
                    ]
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        async function updateDashboard() {
            try {
                const response = await fetch('/api/metrics');
                const result = await response.json();
                const metrics = result.data;
                
                // Update metric cards
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
                        <div class="metric-value ${metrics.error_rate > 5 ? 'error' : 'warning'}">${metrics.error_rate.toFixed(1)}%</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Avg Response Time</div>
                        <div class="metric-value">${metrics.average_response_time_ms.toFixed(0)}ms</div>
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
                
                // Update response time chart
                const timeLabels = metrics.last_hour_response_times
                    .slice(-20)
                    .map(rt => new Date(rt.timestamp).toLocaleTimeString());
                const timeData = metrics.last_hour_response_times
                    .slice(-20)
                    .map(rt => rt.duration_ms);
                
                responseTimeChart.data.labels = timeLabels;
                responseTimeChart.data.datasets[0].data = timeData;
                responseTimeChart.update();
                
                // Update method chart
                const methods = Object.entries(metrics.requests_by_method);
                methodChart.data.labels = methods.map(([method]) => method);
                methodChart.data.datasets[0].data = methods.map(([, count]) => count);
                methodChart.update();
                
                // Update endpoints list
                const endpointsList = metrics.requests_by_endpoint
                    .slice(0, 10)
                    .map(ep => `
                        <div class="endpoint-item">
                            <span>${ep.endpoint}</span>
                            <span>${ep.count} (${ep.percentage.toFixed(1)}%)</span>
                        </div>
                    `).join('');
                document.getElementById('endpointsList').innerHTML = endpointsList;
                
            } catch (error) {
                console.error('Failed to update dashboard:', error);
            }
        }

        function formatUptime(seconds) {
            const hours = Math.floor(seconds / 3600);
            const minutes = Math.floor((seconds % 3600) / 60);
            const secs = seconds % 60;
            
            if (hours > 0) {
                return `${hours}h ${minutes}m`;
            } else if (minutes > 0) {
                return `${minutes}m ${secs}s`;
            } else {
                return `${secs}s`;
            }
        }

        // Update immediately and then every 2 seconds
        updateDashboard();
        setInterval(updateDashboard, 2000);
        </script>
    </body>
    </html>
    "#)
}

async fn handle_metrics(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let item_count = state.store.get_items(None, None)?.len();
    let snapshot = state.metrics.get_snapshot(item_count);
    
    Ok(Json(ApiResponse::success(snapshot)))
}

async fn handle_export_items(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let format = params.get("format").map(|s| s.as_str()).unwrap_or("json");
    let items = state.store.get_items(None, None)?;
    
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
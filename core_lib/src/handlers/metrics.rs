//! Enhanced metrics and monitoring handlers

use crate::{
    error::Result,
    models::request::ApiResponse,
    AppState,
};

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use tracing::info;

pub async fn handle_enhanced_metrics(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("GET /api/metrics - Enhanced metrics with system monitoring");
    
    let item_count = match state.item_service.get_stats().await {
        Ok(stats) => stats.get("total_items").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        Err(_) => 0,
    };
    
    let mut metrics_snapshot = state.metrics.get_snapshot(item_count);
    
    if let Some(system_monitor) = &state.system_monitor {
        let system_metrics = system_monitor.collect_metrics();
        let performance_metrics = system_monitor.get_performance_metrics(&metrics_snapshot);
        
        metrics_snapshot.system_metrics = Some(system_metrics);
        metrics_snapshot.performance_metrics = Some(performance_metrics);
    }
    
    Ok(Json(ApiResponse::success(metrics_snapshot)))
}

pub async fn handle_system_metrics(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("GET /api/system/metrics - System resource metrics");
    
    if let Some(system_monitor) = &state.system_monitor {
        let system_metrics = system_monitor.collect_metrics();
        Ok(Json(ApiResponse::success(system_metrics)))
    } else {
        Ok(Json(ApiResponse::error("System monitoring not enabled".to_string())))
    }
}

pub async fn handle_performance_metrics(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("GET /api/performance/metrics - Performance metrics");
    
    if let Some(system_monitor) = &state.system_monitor {
        let item_count = match state.item_service.get_stats().await {
            Ok(stats) => stats.get("total_items").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            Err(_) => 0,
        };
        
        let app_metrics = state.metrics.get_snapshot(item_count);
        let performance_metrics = system_monitor.get_performance_metrics(&app_metrics);
        
        Ok(Json(ApiResponse::success(performance_metrics)))
    } else {
        Ok(Json(ApiResponse::error("System monitoring not enabled".to_string())))
    }
}

pub async fn handle_resource_alerts(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("GET /api/system/alerts - Resource usage alerts");
    
    if let Some(system_monitor) = &state.system_monitor {
        let system_metrics = system_monitor.collect_metrics();
        let alerts = system_monitor.check_resource_alerts(&system_metrics);
        
        Ok(Json(ApiResponse::success(serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "alerts": alerts,
            "alert_count": alerts.len(),
            "has_critical_alerts": alerts.iter().any(|alert| alert.contains("Critical") || alert.contains("High")),
            "system_status": if alerts.is_empty() { "healthy" } else if alerts.iter().any(|alert| alert.contains("Critical")) { "critical" } else { "warning" }
        }))))
    } else {
        Ok(Json(ApiResponse::error("System monitoring not enabled".to_string())))
    }
}

pub async fn handle_health_history(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("GET /api/health/history - Health status change history");
    
    let item_count = match state.item_service.get_stats().await {
        Ok(stats) => stats.get("total_items").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        Err(_) => 0,
    };
    
    let metrics_snapshot = state.metrics.get_snapshot(item_count);
    
    Ok(Json(ApiResponse::success(serde_json::json!({
        "timestamp": chrono::Utc::now(),
        "health_status_changes": metrics_snapshot.health_status_changes,
        "change_count": metrics_snapshot.health_status_changes.len()
    }))))
}
//! Enhanced health check handlers

use crate::{
    error::{AppError, Result},
    models::request::ApiResponse,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::{info, warn};

pub async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    info!("GET /health - Running comprehensive health checks");
    
    if let Some(health_checker) = &state.health_checker {
        let system_health = health_checker.check_all().await;
        
        let status_code = match system_health.overall_status {
            crate::health::HealthStatus::Healthy => StatusCode::OK,
            crate::health::HealthStatus::Degraded => {
                warn!("System health is degraded");
                StatusCode::OK
            }
            crate::health::HealthStatus::Unhealthy => {
                warn!("System health is unhealthy");
                StatusCode::SERVICE_UNAVAILABLE
            }
        };
        
        (status_code, Json(ApiResponse::success(system_health))).into_response()
    } else {
        handle_basic_health(State(state)).await.into_response()
    }
}

pub async fn handle_basic_health(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.item_service.get_stats().await.unwrap_or_default();
    
    let mut health_info = serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().timestamp(),
        "store_stats": stats,
        "using_database": state.item_service.is_using_database(),
        "version": state.version,
        "uptime_seconds": 0
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

pub async fn handle_component_health(
    State(state): State<AppState>,
    Path(component): Path<String>,
) -> Result<impl IntoResponse> {
    info!("GET /health/{} - Checking specific component", component);
    
    if let Some(health_checker) = &state.health_checker {
        if let Some(component_health) = health_checker.check_component(&component).await {
            let status_code = match component_health.status {
                crate::health::HealthStatus::Healthy => StatusCode::OK,
                crate::health::HealthStatus::Degraded => StatusCode::OK,
                crate::health::HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
            };
            
            Ok((status_code, Json(ApiResponse::success(component_health))))
        } else {
            Err(AppError::NotFound(format!("Component '{}' not found", component)))
        }
    } else {
        Err(AppError::BadRequest("Health checker not configured".to_string()))
    }
}

pub async fn handle_readiness(State(state): State<AppState>) -> impl IntoResponse {
    info!("GET /ready - Readiness probe");
    
    let mut ready = true;
    let mut issues = Vec::new();
    
    if let Some(db_manager) = &state.db_manager {
        if db_manager.health_check().await.is_err() {
            ready = false;
            issues.push("database_unavailable");
        }
    }
    
    if state.item_service.get_stats().await.is_err() {
        ready = false;
        issues.push("item_service_unavailable");
    }
    
    if ready {
        (StatusCode::OK, Json(ApiResponse::success(serde_json::json!({
            "status": "ready",
            "timestamp": chrono::Utc::now().timestamp()
        }))))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(ApiResponse::error(
            format!("Service not ready: {}", issues.join(", "))
        )))
    }
}

pub async fn handle_liveness(State(_state): State<AppState>) -> impl IntoResponse {
    info!("GET /live - Liveness probe");
    
    (StatusCode::OK, Json(ApiResponse::success(serde_json::json!({
        "status": "alive",
        "timestamp": chrono::Utc::now().timestamp()
    }))))
}
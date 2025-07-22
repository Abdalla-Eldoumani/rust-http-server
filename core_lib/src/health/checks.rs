//! Comprehensive health check system for monitoring server components

use crate::{AppState, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tokio::fs;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub response_time_ms: u64,
    pub last_checked: chrono::DateTime<chrono::Utc>,
}

impl ComponentHealth {
    pub fn healthy(message: String, response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Healthy,
            message,
            details: None,
            response_time_ms,
            last_checked: chrono::Utc::now(),
        }
    }

    pub fn degraded(message: String, response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message,
            details: None,
            response_time_ms,
            last_checked: chrono::Utc::now(),
        }
    }

    pub fn unhealthy(message: String, response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message,
            details: None,
            response_time_ms,
            last_checked: chrono::Utc::now(),
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
    pub version: String,
}

impl SystemHealth {
    pub fn new(version: String, uptime_seconds: u64) -> Self {
        Self {
            overall_status: HealthStatus::Healthy,
            components: HashMap::new(),
            timestamp: chrono::Utc::now(),
            uptime_seconds,
            version,
        }
    }

    pub fn add_component(&mut self, name: String, health: ComponentHealth) {
        match health.status {
            HealthStatus::Unhealthy => {
                self.overall_status = HealthStatus::Unhealthy;
            }
            HealthStatus::Degraded => {
                if self.overall_status == HealthStatus::Healthy {
                    self.overall_status = HealthStatus::Degraded;
                }
            }
            HealthStatus::Healthy => {
            }
        }
        
        self.components.insert(name, health);
    }

    pub fn is_healthy(&self) -> bool {
        self.overall_status == HealthStatus::Healthy
    }
}

#[async_trait::async_trait]
pub trait HealthCheck {
    async fn check(&self) -> ComponentHealth;
    fn name(&self) -> &str;
}

pub struct DatabaseHealthCheck {
    pool: sqlx::SqlitePool,
}

impl DatabaseHealthCheck {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl HealthCheck for DatabaseHealthCheck {
    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();
        
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {
                let response_time = start.elapsed().as_millis() as u64;
                
                if response_time > 1000 {
                    ComponentHealth::degraded(
                        "Database responding slowly".to_string(),
                        response_time,
                    ).with_details(serde_json::json!({
                        "query_time_ms": response_time,
                        "threshold_ms": 1000
                    }))
                } else {
                    ComponentHealth::healthy(
                        "Database connection successful".to_string(),
                        response_time,
                    ).with_details(serde_json::json!({
                        "query_time_ms": response_time
                    }))
                }
            }
            Err(e) => {
                let response_time = start.elapsed().as_millis() as u64;
                ComponentHealth::unhealthy(
                    format!("Database connection failed: {}", e),
                    response_time,
                ).with_details(serde_json::json!({
                    "error": e.to_string(),
                    "error_type": "connection_failure"
                }))
            }
        }
    }

    fn name(&self) -> &str {
        "database"
    }
}

pub struct FilesystemHealthCheck {
    paths: Vec<String>,
}

impl FilesystemHealthCheck {
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }
}

#[async_trait::async_trait]
impl HealthCheck for FilesystemHealthCheck {
    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();
        let mut issues = Vec::new();
        let mut details = serde_json::Map::new();

        for path in &self.paths {
            let path_obj = Path::new(path);
            
            if !path_obj.exists() {
                issues.push(format!("Path does not exist: {}", path));
                details.insert(path.clone(), serde_json::json!({
                    "exists": false,
                    "readable": false,
                    "writable": false
                }));
                continue;
            }

            let mut path_details = serde_json::Map::new();
            path_details.insert("exists".to_string(), serde_json::Value::Bool(true));

            let readable = fs::metadata(path).await.is_ok();
            path_details.insert("readable".to_string(), serde_json::Value::Bool(readable));
            
            if !readable {
                issues.push(format!("Cannot read path: {}", path));
            }

            let writable = if path_obj.is_dir() {
                let temp_file = path_obj.join(".health_check_temp");
                match fs::write(&temp_file, "test").await {
                    Ok(_) => {
                        let _ = fs::remove_file(&temp_file).await;
                        true
                    }
                    Err(_) => false,
                }
            } else {
                if let Some(parent) = path_obj.parent() {
                    let temp_file = parent.join(".health_check_temp");
                    match fs::write(&temp_file, "test").await {
                        Ok(_) => {
                            let _ = fs::remove_file(&temp_file).await;
                            true
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                }
            };

            path_details.insert("writable".to_string(), serde_json::Value::Bool(writable));
            
            if !writable {
                issues.push(format!("Cannot write to path: {}", path));
            }

            details.insert(path.clone(), serde_json::Value::Object(path_details));
        }

        let response_time = start.elapsed().as_millis() as u64;

        if issues.is_empty() {
            ComponentHealth::healthy(
                "All filesystem paths accessible".to_string(),
                response_time,
            ).with_details(serde_json::Value::Object(details))
        } else if issues.len() < self.paths.len() {
            ComponentHealth::degraded(
                format!("Some filesystem issues: {}", issues.join(", ")),
                response_time,
            ).with_details(serde_json::json!({
                "paths": details,
                "issues": issues
            }))
        } else {
            ComponentHealth::unhealthy(
                format!("Filesystem access failed: {}", issues.join(", ")),
                response_time,
            ).with_details(serde_json::json!({
                "paths": details,
                "issues": issues
            }))
        }
    }

    fn name(&self) -> &str {
        "filesystem"
    }
}

pub struct DependencyHealthCheck {
    name: String,
    check_fn: Box<dyn Fn() -> Result<String> + Send + Sync>,
}

impl DependencyHealthCheck {
    pub fn new<F>(name: String, check_fn: F) -> Self
    where
        F: Fn() -> Result<String> + Send + Sync + 'static,
    {
        Self {
            name,
            check_fn: Box::new(check_fn),
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for DependencyHealthCheck {
    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();
        
        match (self.check_fn)() {
            Ok(message) => {
                let response_time = start.elapsed().as_millis() as u64;
                ComponentHealth::healthy(message, response_time)
            }
            Err(e) => {
                let response_time = start.elapsed().as_millis() as u64;
                ComponentHealth::unhealthy(
                    format!("Dependency check failed: {}", e),
                    response_time,
                ).with_details(serde_json::json!({
                    "error": e.to_string(),
                    "dependency": self.name
                }))
            }
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck + Send + Sync>>,
    start_time: Instant,
    version: String,
}

impl HealthChecker {
    pub fn new(version: String) -> Self {
        Self {
            checks: Vec::new(),
            start_time: Instant::now(),
            version,
        }
    }

    pub fn add_check<T: HealthCheck + Send + Sync + 'static>(mut self, check: T) -> Self {
        self.checks.push(Box::new(check));
        self
    }

    pub async fn check_all(&self) -> SystemHealth {
        let uptime_seconds = self.start_time.elapsed().as_secs();
        let mut system_health = SystemHealth::new(self.version.clone(), uptime_seconds);

        info!("Running comprehensive health checks for {} components", self.checks.len());

        for check in &self.checks {
            let component_name = check.name().to_string();
            let start = Instant::now();
            
            let health = check.check().await;
            let check_duration = start.elapsed();
            
            match health.status {
                HealthStatus::Healthy => {
                    info!("Health check '{}' passed in {:?}", component_name, check_duration);
                }
                HealthStatus::Degraded => {
                    warn!("Health check '{}' degraded in {:?}: {}", component_name, check_duration, health.message);
                }
                HealthStatus::Unhealthy => {
                    error!("Health check '{}' failed in {:?}: {}", component_name, check_duration, health.message);
                }
            }
            
            system_health.add_component(component_name, health);
        }

        info!("Health check completed - Overall status: {}", system_health.overall_status);
        system_health
    }

    pub async fn check_component(&self, component_name: &str) -> Option<ComponentHealth> {
        for check in &self.checks {
            if check.name() == component_name {
                return Some(check.check().await);
            }
        }
        None
    }
}

impl HealthChecker {
    pub fn from_app_state(state: &AppState) -> Self {
        let mut checker = HealthChecker::new(state.version.clone());

        if let Some(db_manager) = &state.db_manager {
            checker = checker.add_check(DatabaseHealthCheck::new(db_manager.pool().clone()));
        }

        let mut fs_paths = vec!["./".to_string()];
        
        if let Some(_file_manager) = &state.file_manager {
            fs_paths.push("./uploads".to_string());
            fs_paths.push("./temp".to_string());
        }
        
        checker = checker.add_check(FilesystemHealthCheck::new(fs_paths));

        checker = checker.add_check(DependencyHealthCheck::new(
            "memory_store".to_string(),
            {
                let store = state.store.clone();
                move || {
                    match store.get_stats() {
                        Ok(stats) => {
                            let item_count = stats.get("total_items")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            Ok(format!("Memory store operational with {} items", item_count))
                        }
                        Err(e) => Err(e)
                    }
                }
            }
        ));

        if state.auth_service.is_some() {
            checker = checker.add_check(DependencyHealthCheck::new(
                "auth_service".to_string(),
                || Ok("Authentication service is configured and ready".to_string())
            ));
        }

        if state.websocket_manager.is_some() {
            checker = checker.add_check(DependencyHealthCheck::new(
                "websocket_manager".to_string(),
                || Ok("WebSocket manager is configured and ready".to_string())
            ));
        }

        if state.job_queue.is_some() {
            checker = checker.add_check(DependencyHealthCheck::new(
                "job_queue".to_string(),
                || Ok("Job queue is configured and ready".to_string())
            ));
        }

        if state.cache_manager.is_some() {
            checker = checker.add_check(DependencyHealthCheck::new(
                "cache_manager".to_string(),
                || Ok("Cache manager is configured and ready".to_string())
            ));
        }

        checker
    }
}
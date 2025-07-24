//! Core library containing business logic and route handlers for the HTTP server.

pub mod auth;
pub mod cache;
pub mod config;
pub mod database;
pub mod error;
pub mod files;
pub mod handlers;
pub mod health;
pub mod jobs;
pub mod middleware;
pub mod models;
pub mod monitoring;
pub mod search;
pub mod services;
pub mod store;
pub mod metrics;
pub mod validation;
pub mod websocket;

pub use auth::{AuthService, JwtService, UserRepository, UserRepositoryTrait};
pub use cache::{CacheManager, CacheStats};
pub use config::AppConfig;
pub use database::{DatabaseManager, get_database_pool, run_migrations, ItemRepository, MigrationService};
pub use files::{FileManager, FileRepository, FileValidator, FileManagerConfig};
pub use health::{HealthChecker, HealthStatus, HealthCheck, ComponentHealth, SystemHealth};
pub use jobs::{JobQueue, JobRepository, JobRepositoryTrait, Job, JobRequest, JobResponse, JobStatus, JobType, JobPriority, JobListParams, JobListResponse};
pub use monitoring::{SystemMonitor, SystemMetrics, ResourceUsage, DiskUsage};
pub use search::{SearchEngine, SearchQuery, SearchResult, SearchFilters, SearchCache};
pub use services::ItemService;
pub use error::{AppError, Result};
pub use handlers::routes::create_routes;

pub use middleware::cors::{cors_layer, cors_layer_permissive, cors_layer_from_config};
pub use middleware::auth::{AuthUser, jwt_auth_middleware, optional_jwt_auth_middleware, require_admin, require_self_or_admin};
pub use middleware::cache::cache_middleware;
pub use store::DataStore;
pub use metrics::MetricsCollector;
pub use middleware::rate_limit::RateLimiter;
pub use validation::{ValidationResult, ValidationContext, Validatable, ContextValidatable, SecurityValidator};
pub use websocket::{WebSocketManager, websocket_handler};

use axum::{
    middleware as axum_middleware,
    Router,
    extract::State,
    response::Response,
    http::Request,
    middleware::Next,
    body::Body,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub app_name: String,
    pub version: String,
    pub store: DataStore,
    pub db_manager: Option<DatabaseManager>,
    pub item_service: ItemService,
    pub search_engine: Option<SearchEngine>,
    pub metrics: MetricsCollector,
    pub rate_limiter: RateLimiter,
    pub auth_service: Option<AuthService>,
    pub websocket_manager: Option<WebSocketManager>,
    pub file_manager: Option<FileManager>,
    pub job_queue: Option<JobQueue>,
    pub cache_manager: Option<CacheManager>,
    pub health_checker: Option<std::sync::Arc<HealthChecker>>,
    pub system_monitor: Option<std::sync::Arc<SystemMonitor>>,
}

impl Default for AppState {
    fn default() -> Self {
        let store = DataStore::new();
        let item_service = ItemService::with_memory_store(store.clone());
        
        Self {
            app_name: "Rust HTTP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            store,
            db_manager: None,
            item_service,
            search_engine: None,
            metrics: MetricsCollector::new(),
            rate_limiter: RateLimiter::new(crate::config::RateLimitConfig::default()),
            auth_service: None,
            websocket_manager: None,
            file_manager: None,
            job_queue: None,
            cache_manager: None,
            health_checker: None,
            system_monitor: None,
        }
    }
}

impl AppState {
    pub fn with_database(db_manager: DatabaseManager, item_repository: ItemRepository) -> Self {
        let store = DataStore::new();
        let item_service = ItemService::with_database(item_repository, store.clone());
        let search_cache = SearchCache::default();
        let search_engine = SearchEngine::new(db_manager.pool().clone()).with_cache(search_cache);
        
        Self {
            app_name: "Rust HTTP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            store,
            db_manager: Some(db_manager),
            item_service,
            search_engine: Some(search_engine),
            metrics: MetricsCollector::new(),
            rate_limiter: RateLimiter::new(crate::config::RateLimitConfig::default()),
            auth_service: None,
            websocket_manager: None,
            file_manager: None,
            job_queue: None,
            cache_manager: None,
            health_checker: None,
            system_monitor: None,
        }
    }

    pub fn with_rate_limiter(mut self, rate_limiter: RateLimiter) -> Self {
        self.rate_limiter = rate_limiter;
        self
    }

    pub fn with_auth(mut self, auth_service: AuthService) -> Self {
        self.auth_service = Some(auth_service);
        self
    }

    pub fn with_websocket(mut self, websocket_manager: WebSocketManager) -> Self {
        self.websocket_manager = Some(websocket_manager);
        self
    }

    pub fn with_file_manager(mut self, file_manager: FileManager) -> Self {
        self.file_manager = Some(file_manager);
        self
    }

    pub fn with_job_queue(mut self, job_queue: JobQueue) -> Self {
        self.job_queue = Some(job_queue);
        self
    }

    pub fn with_cache_manager(mut self, cache_manager: CacheManager) -> Self {
        self.cache_manager = Some(cache_manager);
        self
    }

    pub fn with_health_checker(mut self) -> Self {
        let health_checker = HealthChecker::from_app_state(&self);
        self.health_checker = Some(std::sync::Arc::new(health_checker));
        self
    }

    pub fn with_system_monitor(mut self) -> Self {
        let system_monitor = SystemMonitor::new();
        self.system_monitor = Some(std::sync::Arc::new(system_monitor));
        self
    }

    pub async fn create_job_queue_with_websocket(&self, job_repository: JobRepository) -> Result<JobQueue> {
        let websocket_manager = self.websocket_manager.as_ref().map(|ws| Arc::new(ws.clone()));
        let job_queue = JobQueue::new_with_websocket(job_repository, websocket_manager);
        Ok(job_queue)
    }

    pub async fn migrate_to_database_if_needed(&self) -> Result<()> {
        if let Some(repo) = self.item_service.repository() {
            let migration_service = MigrationService::new(repo.clone());
            
            if migration_service.is_migration_needed(&self.store).await? {
                info!("Starting automatic migration from in-memory store to database");
                let result = migration_service.migrate_from_memory_store(&self.store).await?;
                info!("Migration completed: {} items migrated successfully, {} failed", result.successful_migrations, result.failed_count);
            }
        }
        Ok(())
    }
}

pub fn create_app(state: AppState) -> Router {
    create_app_with_config(state, AppConfig::default())
}

pub fn create_app_with_config(state: AppState, config: AppConfig) -> Router {
    let mut router = Router::new().merge(create_routes());

    router = router.layer(middleware::cors::cors_layer_from_config(&config.cors));

    router = router.layer(axum_middleware::from_fn_with_state(
        state.clone(),
        middleware::auth::optional_jwt_auth_middleware,
    ));

    router = router.layer(axum_middleware::from_fn_with_state(
        state.clone(),
        middleware::cache::cache_middleware,
    ));

    if config.rate_limit.enable {
        router = router.layer(axum_middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            middleware::rate_limit::rate_limit_middleware,
        ));
    }

    router = router.layer(axum_middleware::from_fn_with_state(
        state.clone(),
        metrics_middleware,
    ));

    router = router.layer(axum_middleware::from_fn(validation::middleware::validation_middleware));

    router = router.layer(axum_middleware::from_fn(
        middleware::logging::log_request_with_config(config.logging)
    ));

    router.with_state(state)
}

async fn metrics_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> std::result::Result<Response, std::convert::Infallible> {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = std::time::Instant::now();
    
    state.metrics.record_request(&method, &path);
    
    let response = next.run(request).await;
    
    let duration = start.elapsed();
    let status = response.status().as_u16();
    state.metrics.record_response(&path, duration.as_millis(), status);
    
    Ok(response)
}

pub async fn run_server(app: Router, addr: SocketAddr) -> Result<()> {
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    let app = app.into_make_service_with_connect_info::<SocketAddr>();
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, starting graceful shutdown");
        },
        _ = terminate => {
            info!("Received SIGTERM, starting graceful shutdown");
        },
    }
}
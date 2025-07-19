//! Core library containing business logic and route handlers for the HTTP server.

pub mod config;
pub mod database;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod services;
pub mod store;
pub mod metrics;

pub use config::AppConfig;
pub use database::{DatabaseManager, get_database_pool, run_migrations, ItemRepository, MigrationService};
pub use services::ItemService;
pub use error::{AppError, Result};
pub use handlers::routes::create_routes;
pub use middleware::cors::{cors_layer, cors_layer_permissive};
pub use store::DataStore;
pub use metrics::MetricsCollector;
pub use middleware::rate_limit::RateLimiter;

use axum::{
    middleware as axum_middleware,
    Router,
    extract::State,
    response::Response,
    http::Request,
    middleware::Next,
    body::Body,
};
use std::net::SocketAddr;
use tokio::signal;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub app_name: String,
    pub version: String,
    pub store: DataStore,
    pub db_manager: Option<DatabaseManager>,
    pub item_service: ItemService,
    pub metrics: MetricsCollector,
    pub rate_limiter: RateLimiter,
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
            metrics: MetricsCollector::new(),
            rate_limiter: RateLimiter::new(100, 60),
        }
    }
}

impl AppState {
    pub fn with_database(db_manager: DatabaseManager, item_repository: ItemRepository) -> Self {
        let store = DataStore::new();
        let item_service = ItemService::with_database(item_repository, store.clone());
        
        Self {
            app_name: "Rust HTTP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            store,
            db_manager: Some(db_manager),
            item_service,
            metrics: MetricsCollector::new(),
            rate_limiter: RateLimiter::new(100, 60),
        }
    }

    pub async fn migrate_to_database_if_needed(&self) -> Result<()> {
        if let Some(repo) = self.item_service.repository() {
            let migration_service = MigrationService::new(repo.clone());
            
            if migration_service.is_migration_needed(&self.store).await? {
                info!("Starting automatic migration from in-memory store to database");
                let result = migration_service.migrate_from_memory_store(&self.store).await?;
                info!("Migration completed: {} items migrated successfully, {} failed", 
                      result.successful_migrations, result.failed_count);
            }
        }
        Ok(())
    }
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .merge(create_routes())
        .layer(cors_layer_permissive())
        .layer(axum_middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            middleware::rate_limit::rate_limit_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            metrics_middleware,
        ))
        .layer(axum_middleware::from_fn(middleware::logging::log_request))
        .with_state(state)
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
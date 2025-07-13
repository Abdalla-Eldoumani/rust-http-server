//! Core library containing business logic and route handlers for the HTTP server.

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod store;
pub mod metrics;

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
    pub metrics: MetricsCollector,
    pub rate_limiter: RateLimiter,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_name: "Rust HTTP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            store: DataStore::new(),
            metrics: MetricsCollector::new(),
            rate_limiter: RateLimiter::new(100, 60),
        }
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
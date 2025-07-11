//! Core library containing business logic and route handlers for the HTTP server.

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;

pub use error::{AppError, Result};
pub use handlers::routes::create_routes;
pub use middleware::{cors::cors_layer, logging::custom_logging_middleware};

use axum::Router;
use std::net::SocketAddr;
use tokio::signal;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub app_name: String,
    pub version: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_name: "Rust HTTP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .merge(create_routes())
        .layer(cors_layer())
        .layer(custom_logging_middleware())
        .with_state(state)
}

pub async fn run_server(app: Router, addr: SocketAddr) -> Result<()> {
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    
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
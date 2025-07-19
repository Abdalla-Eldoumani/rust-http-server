//! Main entry point for the HTTP server binary

use anyhow::Result;
use core_lib::{create_app, run_server, AppState, AppConfig};
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = AppConfig::load()
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    info!("Configuration loaded successfully");
    info!("Server will bind to: {}", config.bind_address());
    info!("Database URL: {}", config.database.url);

    config.create_directories()
        .map_err(|e| anyhow::anyhow!("Failed to create directories: {}", e))?;

    let addr: SocketAddr = config.bind_address().parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address: {}", e))?;

    info!("Initializing Rust HTTP Server");
    info!("Environment: {}", std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()));

    let state = AppState::default();
    info!("App: {} v{}", state.app_name, state.version);

    let app = create_app(state);

    run_server(app, addr).await?;

    info!("Server shutdown complete");
    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            let default_level = if cfg!(debug_assertions) {
                "debug"
            } else {
                "info"
            };
            
            format!(
                "{}={},tower_http=debug,axum=debug",
                env!("CARGO_CRATE_NAME").replace('-', "_"),
                default_level
            ).into()
        });

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    let is_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(false);

    if is_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer.json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer.pretty())
            .init();
    }
}
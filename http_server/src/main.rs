//! Main entry point for the HTTP server binary

use anyhow::Result;
use core_lib::{create_app, run_server, AppState, AppConfig, DatabaseManager, ItemRepository, get_database_pool, run_migrations, WebSocketManager, JwtService, FileManager, FileRepository, FileManagerConfig};
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

    let state = if config.database.url != "sqlite::memory:" && !config.database.url.is_empty() {
        info!("Initializing database connection: {}", config.database.url);
        
        match initialize_database(&config.database.url).await {
            Ok((db_manager, item_repository, file_manager)) => {
                info!("Database initialized successfully");
                let mut state = AppState::with_database(db_manager, item_repository);
                
                if let Err(e) = state.migrate_to_database_if_needed().await {
                    tracing::warn!("Failed to migrate data to database: {}", e);
                }
                
                state = state.with_file_manager(file_manager);
                info!("File manager initialized");
                
                let jwt_service = match JwtService::new() {
                    Ok(jwt) => {
                        info!("JWT service initialized for WebSocket authentication");
                        Some(jwt)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to initialize JWT service for WebSocket auth: {}", e);
                        None
                    }
                };
                
                let websocket_manager = WebSocketManager::new(jwt_service);
                state = state.with_websocket(websocket_manager);
                info!("WebSocket manager initialized");
                
                state
            }
            Err(e) => {
                tracing::warn!("Failed to initialize database, falling back to in-memory store: {}", e);
                let mut state = AppState::default();
                
                let websocket_manager = WebSocketManager::new(None);
                state = state.with_websocket(websocket_manager);
                info!("WebSocket manager initialized (no auth)");
                
                state
            }
        }
    } else {
        info!("Using in-memory data store");
        let mut state = AppState::default();
        
        let websocket_manager = WebSocketManager::new(None);
        state = state.with_websocket(websocket_manager);
        info!("WebSocket manager initialized (no auth)");
        
        state
    };

    info!("App: {} v{}", state.app_name, state.version);
    info!("Data storage: {}", if state.item_service.is_using_database() { "SQLite Database" } else { "In-Memory Store" });

    if let Some(ws_manager) = &state.websocket_manager {
        let ws_manager_clone = ws_manager.clone();
        let metrics_clone = state.metrics.clone();
        let item_service_clone = state.item_service.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                
                if ws_manager_clone.connection_count().await > 0 {
                    let item_count = match item_service_clone.get_stats().await {
                        Ok(stats) => stats.get("total_items").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        Err(_) => 0,
                    };
                    
                    let metrics_snapshot = metrics_clone.get_snapshot(item_count);
                    let event = core_lib::websocket::WebSocketEvent::MetricsUpdate(metrics_snapshot);
                    ws_manager_clone.broadcast(event).await;
                }
            }
        });
        
        info!("Started metrics broadcasting task (every 5 seconds)");
    }

    let app = create_app(state);

    run_server(app, addr).await?;

    info!("Server shutdown complete");
    Ok(())
}

async fn initialize_database(database_url: &str) -> Result<(DatabaseManager, ItemRepository, FileManager)> {
    let pool = get_database_pool(database_url).await
        .map_err(|e| anyhow::anyhow!("Failed to create database pool: {}", e))?;

    run_migrations(pool.clone()).await
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {}", e))?;

    let db_manager = DatabaseManager::new(pool.clone());
    let item_repository = ItemRepository::new(pool.clone());
    
    let file_repository = FileRepository::new(pool);
    let file_manager_config = FileManagerConfig::default();
    let file_manager = FileManager::new(file_manager_config, file_repository);
    
    file_manager.initialize().await
        .map_err(|e| anyhow::anyhow!("Failed to initialize file manager: {}", e))?;

    Ok((db_manager, item_repository, file_manager))
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
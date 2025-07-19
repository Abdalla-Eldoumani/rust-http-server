use axum::{
    extract::{
        ws::{WebSocketUpgrade, WebSocket},
        Query, State,
    },
    response::Response,
};
use tracing::{info, warn};

use crate::websocket::manager::WebSocketManager;
use crate::AppState;

#[derive(serde::Deserialize)]
pub struct WebSocketQuery {
    token: Option<String>,
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WebSocketQuery>,
    State(state): State<AppState>,
) -> Response {
    info!("WebSocket connection request received");

    let ws_manager = match &state.websocket_manager {
        Some(manager) => manager.clone(),
        None => {
            warn!("WebSocket manager not available");
            return ws.on_upgrade(|_| async {
            });
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, ws_manager, params.token))
}

async fn handle_socket(
    socket: WebSocket,
    ws_manager: WebSocketManager,
    token: Option<String>,
) {
    info!("WebSocket connection established");

    if let Err(e) = ws_manager.handle_connection(socket, token).await {
        warn!("WebSocket connection error: {}", e);
    }

    info!("WebSocket connection closed");
}
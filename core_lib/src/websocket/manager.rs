use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use axum::extract::ws::{WebSocket, Message};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

use crate::websocket::messages::{WebSocketMessage, WebSocketEvent};
use crate::auth::JwtService;
use crate::error::{AppError, Result};

#[derive(Debug)]
pub struct WebSocketConnection {
    pub id: Uuid,
    pub user_id: Option<u64>,
    pub connected_at: DateTime<Utc>,
    pub sender: mpsc::UnboundedSender<WebSocketMessage>,
}

impl WebSocketConnection {
    pub fn new(user_id: Option<u64>, sender: mpsc::UnboundedSender<WebSocketMessage>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            connected_at: Utc::now(),
            sender,
        }
    }

    pub fn send(&self, message: WebSocketMessage) -> Result<()> {
        self.sender.send(message)
            .map_err(|_| AppError::WebSocket("Failed to send message to connection".to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WebSocketManager {
    connections: Arc<RwLock<HashMap<Uuid, WebSocketConnection>>>,
    jwt_service: Option<JwtService>,
}

impl WebSocketManager {
    pub fn new(jwt_service: Option<JwtService>) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            jwt_service,
        }
    }

    pub async fn add_connection(&self, connection: WebSocketConnection) {
        let connection_id = connection.id;
        let mut connections = self.connections.write().await;
        connections.insert(connection_id, connection);
        info!("WebSocket connection added: {}", connection_id);
    }

    pub async fn remove_connection(&self, connection_id: &Uuid) {
        let mut connections = self.connections.write().await;
        if connections.remove(connection_id).is_some() {
            info!("WebSocket connection removed: {}", connection_id);
        }
    }

    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    pub async fn broadcast(&self, event: WebSocketEvent) {
        let message = WebSocketMessage::from(event);
        let connections = self.connections.read().await;
        let mut failed_connections = Vec::new();

        for (connection_id, connection) in connections.iter() {
            if let Err(_) = connection.send(message.clone()) {
                warn!("Failed to send message to connection: {}", connection_id);
                failed_connections.push(*connection_id);
            }
        }

        drop(connections);
        if !failed_connections.is_empty() {
            let mut connections = self.connections.write().await;
            for connection_id in failed_connections {
                connections.remove(&connection_id);
                info!("Removed failed connection: {}", connection_id);
            }
        }
    }

    pub async fn broadcast_to_user(&self, user_id: u64, event: WebSocketEvent) {
        let message = WebSocketMessage::from(event);
        let connections = self.connections.read().await;
        let mut failed_connections = Vec::new();

        for (connection_id, connection) in connections.iter() {
            if connection.user_id == Some(user_id) {
                if let Err(_) = connection.send(message.clone()) {
                    warn!("Failed to send message to user {} connection: {}", user_id, connection_id);
                    failed_connections.push(*connection_id);
                }
            }
        }

        drop(connections);
        if !failed_connections.is_empty() {
            let mut connections = self.connections.write().await;
            for connection_id in failed_connections {
                connections.remove(&connection_id);
                info!("Removed failed connection: {}", connection_id);
            }
        }
    }

    pub async fn handle_connection(
        &self,
        socket: WebSocket,
        token: Option<String>,
    ) -> Result<()> {
        let user_id = if let (Some(jwt_service), Some(token)) = (&self.jwt_service, token) {
            match jwt_service.validate_token(&token) {
                Ok(claims) => {
                    debug!("WebSocket connection authenticated for user: {}", claims.sub);
                    Some(claims.sub.parse::<u64>().unwrap_or(0))
                }
                Err(e) => {
                    warn!("WebSocket authentication failed: {}", e);
                    return Err(AppError::Authentication("Invalid JWT token".to_string()));
                }
            }
        } else {
            None
        };

        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<WebSocketMessage>();

        let connection = WebSocketConnection::new(user_id, tx);
        let connection_id = connection.id;

        let _ = connection.send(WebSocketMessage::Connected { connection_id });

        self.add_connection(connection).await;

        let _manager_clone = self.clone();
        let outgoing_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                let json = match message.to_json() {
                    Ok(json) => json,
                    Err(e) => {
                        error!("Failed to serialize WebSocket message: {}", e);
                        continue;
                    }
                };

                if sender.send(Message::Text(json)).await.is_err() {
                    debug!("WebSocket connection closed, stopping outgoing message handler");
                    break;
                }
            }
        });

        let manager_clone = self.clone();
        let incoming_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received WebSocket message: {}", text);
                        
                        if let Ok(message) = WebSocketMessage::from_json(&text) {
                            match message {
                                WebSocketMessage::Ping => {
                                    let connections = manager_clone.connections.read().await;
                                    if let Some(connection) = connections.get(&connection_id) {
                                        let _ = connection.send(WebSocketMessage::Pong);
                                    }
                                }
                                _ => {
                                    debug!("Received unhandled WebSocket message type");
                                }
                            }
                        }
                    }
                    Ok(Message::Binary(_)) => {
                        debug!("Received binary WebSocket message (not supported)");
                    }
                    Ok(Message::Close(_)) => {
                        debug!("WebSocket connection closed by client");
                        break;
                    }
                    Err(e) => {
                        warn!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        tokio::select! {
            _ = outgoing_task => {
                debug!("Outgoing message handler completed");
            }
            _ = incoming_task => {
                debug!("Incoming message handler completed");
            }
        }

        self.remove_connection(&connection_id).await;
        Ok(())
    }
}
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::store::Item;
use crate::metrics::MetricsSnapshot;
use crate::jobs::JobResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    ItemCreated(Item),
    ItemUpdated(Item),
    ItemDeleted { id: u64 },
    MetricsUpdate(MetricsSnapshot),
    JobStarted(JobResponse),
    JobCompleted(JobResponse),
    JobFailed(JobResponse),
    JobCancelled(JobResponse),
    JobRetrying(JobResponse),
    Connected { connection_id: Uuid },
    Ping,
    Pong,
    Error { message: String },
}

#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    ItemCreated(Item),
    ItemUpdated(Item),
    ItemDeleted(u64),
    MetricsUpdate(MetricsSnapshot),
    JobStarted(JobResponse),
    JobCompleted(JobResponse),
    JobFailed(JobResponse),
    JobCancelled(JobResponse),
    JobRetrying(JobResponse),
    Custom(serde_json::Value),
}

impl From<WebSocketEvent> for WebSocketMessage {
    fn from(event: WebSocketEvent) -> Self {
        match event {
            WebSocketEvent::ItemCreated(item) => WebSocketMessage::ItemCreated(item),
            WebSocketEvent::ItemUpdated(item) => WebSocketMessage::ItemUpdated(item),
            WebSocketEvent::ItemDeleted(id) => WebSocketMessage::ItemDeleted { id },
            WebSocketEvent::MetricsUpdate(metrics) => WebSocketMessage::MetricsUpdate(metrics),
            WebSocketEvent::JobStarted(job) => WebSocketMessage::JobStarted(job),
            WebSocketEvent::JobCompleted(job) => WebSocketMessage::JobCompleted(job),
            WebSocketEvent::JobFailed(job) => WebSocketMessage::JobFailed(job),
            WebSocketEvent::JobCancelled(job) => WebSocketMessage::JobCancelled(job),
            WebSocketEvent::JobRetrying(job) => WebSocketMessage::JobRetrying(job),
            WebSocketEvent::Custom(value) => {
                if let Ok(msg) = serde_json::from_value::<WebSocketMessage>(value.clone()) {
                    msg
                } else {
                    WebSocketMessage::Error { 
                        message: format!("Unknown custom event: {}", value) 
                    }
                }
            }
        }
    }
}

impl WebSocketMessage {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
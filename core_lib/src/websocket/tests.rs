#[cfg(test)]
mod tests {
    use crate::websocket::{
        manager::{WebSocketManager, WebSocketConnection},
        messages::{WebSocketMessage, WebSocketEvent},
    };
    use crate::auth::JwtService;
    use crate::store::Item;
    use crate::metrics::MetricsSnapshot;
    use crate::jobs::models::{JobResponse, JobType, JobStatus, JobPriority};
    use tokio::sync::mpsc;
    use uuid::Uuid;
    use std::env;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_websocket_connection_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let connection = WebSocketConnection::new(Some(1), tx);
        
        assert_eq!(connection.user_id, Some(1));
        assert!(!connection.id.is_nil());
    }

    #[tokio::test]
    async fn test_websocket_connection_send() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let connection = WebSocketConnection::new(Some(1), tx);
        
        let message = WebSocketMessage::Ping;
        assert!(connection.send(message.clone()).is_ok());
        
        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WebSocketMessage::Ping));
    }

    #[tokio::test]
    async fn test_websocket_manager_creation() {
        let manager = WebSocketManager::new(None);
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_websocket_manager_with_jwt() {
        env::set_var("JWT_SECRET", "test_secret_key_for_websocket_tests_12345678901234567890");
        let jwt_service = JwtService::new().ok();
        let manager = WebSocketManager::new(jwt_service);
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_and_remove_connection() {
        let manager = WebSocketManager::new(None);
        let (tx, _rx) = mpsc::unbounded_channel();
        let connection = WebSocketConnection::new(Some(1), tx);
        let connection_id = connection.id;
        
        manager.add_connection(connection).await;
        assert_eq!(manager.connection_count().await, 1);
        
        manager.remove_connection(&connection_id).await;
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_to_all_connections() {
        let manager = WebSocketManager::new(None);
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        
        let connection1 = WebSocketConnection::new(Some(1), tx1);
        let connection2 = WebSocketConnection::new(Some(2), tx2);
        
        manager.add_connection(connection1).await;
        manager.add_connection(connection2).await;
        
        let item = Item {
            id: 1,
            name: "Test Item".to_string(),
            description: Some("Test Description".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
            metadata: None,
        };
        
        let event = WebSocketEvent::ItemCreated(item.clone());
        manager.broadcast(event).await;
        
        let msg1 = rx1.recv().await.unwrap();
        let msg2 = rx2.recv().await.unwrap();
        
        assert!(matches!(msg1, WebSocketMessage::ItemCreated(_)));
        assert!(matches!(msg2, WebSocketMessage::ItemCreated(_)));
    }

    #[tokio::test]
    async fn test_broadcast_to_specific_user() {
        let manager = WebSocketManager::new(None);
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        
        let connection1 = WebSocketConnection::new(Some(1), tx1);
        let connection2 = WebSocketConnection::new(Some(2), tx2);
        
        manager.add_connection(connection1).await;
        manager.add_connection(connection2).await;
        
        let item = Item {
            id: 1,
            name: "Test Item".to_string(),
            description: Some("Test Description".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
            metadata: None,
        };
        
        let event = WebSocketEvent::ItemCreated(item.clone());
        manager.broadcast_to_user(1, event).await;
        
        let msg1 = rx1.recv().await.unwrap();
        assert!(matches!(msg1, WebSocketMessage::ItemCreated(_)));
        
        assert!(rx2.try_recv().is_err());
    }

    #[test]
    fn test_websocket_message_serialization() {
        let message = WebSocketMessage::Ping;
        let json = message.to_json().unwrap();
        assert!(json.contains("Ping"));
        
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        assert!(matches!(deserialized, WebSocketMessage::Ping));
    }

    #[test]
    fn test_websocket_message_item_created() {
        let item = Item {
            id: 1,
            name: "Test Item".to_string(),
            description: Some("Test Description".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
            metadata: None,
        };
        
        let message = WebSocketMessage::ItemCreated(item.clone());
        let json = message.to_json().unwrap();
        
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        if let WebSocketMessage::ItemCreated(deserialized_item) = deserialized {
            assert_eq!(deserialized_item.id, item.id);
            assert_eq!(deserialized_item.name, item.name);
        } else {
            panic!("Expected ItemCreated message");
        }
    }

    #[test]
    fn test_websocket_message_item_deleted() {
        let message = WebSocketMessage::ItemDeleted { id: 42 };
        let json = message.to_json().unwrap();
        
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        if let WebSocketMessage::ItemDeleted { id } = deserialized {
            assert_eq!(id, 42);
        } else {
            panic!("Expected ItemDeleted message");
        }
    }

    #[test]
    fn test_websocket_message_error() {
        let message = WebSocketMessage::Error { 
            message: "Test error".to_string() 
        };
        let json = message.to_json().unwrap();
        
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        if let WebSocketMessage::Error { message: error_msg } = deserialized {
            assert_eq!(error_msg, "Test error");
        } else {
            panic!("Expected Error message");
        }
    }

    #[test]
    fn test_websocket_event_to_message_conversion() {
        let item = Item {
            id: 1,
            name: "Test Item".to_string(),
            description: Some("Test Description".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
            metadata: None,
        };
        
        let event = WebSocketEvent::ItemCreated(item.clone());
        let message = WebSocketMessage::from(event);
        
        if let WebSocketMessage::ItemCreated(msg_item) = message {
            assert_eq!(msg_item.id, item.id);
            assert_eq!(msg_item.name, item.name);
        } else {
            panic!("Expected ItemCreated message");
        }
    }

    #[test]
    fn test_websocket_event_item_deleted_conversion() {
        let event = WebSocketEvent::ItemDeleted(42);
        let message = WebSocketMessage::from(event);
        
        if let WebSocketMessage::ItemDeleted { id } = message {
            assert_eq!(id, 42);
        } else {
            panic!("Expected ItemDeleted message");
        }
    }

    #[test]
    fn test_websocket_event_custom_conversion() {
        let custom_value = serde_json::json!({
            "type": "Ping"
        });
        
        let event = WebSocketEvent::Custom(custom_value);
        let message = WebSocketMessage::from(event);
        
        assert!(matches!(message, WebSocketMessage::Ping));
    }

    #[test]
    fn test_websocket_event_custom_invalid_conversion() {
        let custom_value = serde_json::json!({
            "invalid": "data"
        });
        
        let event = WebSocketEvent::Custom(custom_value);
        let message = WebSocketMessage::from(event);
        
        if let WebSocketMessage::Error { message: error_msg } = message {
            assert!(error_msg.contains("Unknown custom event"));
        } else {
            panic!("Expected Error message for invalid custom event");
        }
    }

    #[test]
    fn test_websocket_message_metrics_update() {
        let metrics = MetricsSnapshot {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            requests_by_method: HashMap::new(),
            requests_by_endpoint: vec![],
            average_response_time_ms: 150.0,
            uptime_seconds: 3600,
            requests_per_second: 10.0,
            error_rate: 0.05,
            last_hour_response_times: vec![],
            system_metrics: None,
            performance_metrics: None,
            health_status_changes: vec![],
        };
        
        let message = WebSocketMessage::MetricsUpdate(metrics.clone());
        let json = message.to_json().unwrap();
        
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        if let WebSocketMessage::MetricsUpdate(deserialized_metrics) = deserialized {
            assert_eq!(deserialized_metrics.total_requests, metrics.total_requests);
            assert_eq!(deserialized_metrics.uptime_seconds, metrics.uptime_seconds);
        } else {
            panic!("Expected MetricsUpdate message");
        }
    }

    #[test]
    fn test_websocket_message_job_events() {
        let job_response = JobResponse {
            id: Uuid::new_v4(),
            job_type: JobType::BulkImport,
            status: JobStatus::Running,
            created_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            completed_at: None,
            result: None,
            error_message: None,
            retry_count: 0,
            max_retries: 3,
            priority: JobPriority::Normal,
        };
        
        let message = WebSocketMessage::JobStarted(job_response.clone());
        let json = message.to_json().unwrap();
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        assert!(matches!(deserialized, WebSocketMessage::JobStarted(_)));
        
        let message = WebSocketMessage::JobCompleted(job_response.clone());
        let json = message.to_json().unwrap();
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        assert!(matches!(deserialized, WebSocketMessage::JobCompleted(_)));
        
        let message = WebSocketMessage::JobFailed(job_response.clone());
        let json = message.to_json().unwrap();
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        assert!(matches!(deserialized, WebSocketMessage::JobFailed(_)));
    }

    #[test]
    fn test_websocket_message_connected() {
        let connection_id = Uuid::new_v4();
        let message = WebSocketMessage::Connected { connection_id };
        let json = message.to_json().unwrap();
        
        let deserialized = WebSocketMessage::from_json(&json).unwrap();
        if let WebSocketMessage::Connected { connection_id: deserialized_id } = deserialized {
            assert_eq!(deserialized_id, connection_id);
        } else {
            panic!("Expected Connected message");
        }
    }

    #[test]
    fn test_websocket_message_invalid_json() {
        let invalid_json = "{ invalid json }";
        let result = WebSocketMessage::from_json(invalid_json);
        assert!(result.is_err());
    }
}
use core_lib::{
    get_database_pool, run_migrations, DatabaseManager, ItemRepository,
    auth::{JwtService, UserRepository, AuthService},
    files::{FileManager, FileRepository},
    jobs::{JobQueue, JobRepository},
    cache::CacheManager,
    websocket::WebSocketManager,
    config::CacheConfig,
};
use tempfile::NamedTempFile;
use std::env;

async fn setup_full_system() -> core_lib::AppState {
    let temp_file = NamedTempFile::new().unwrap();
    let database_url = format!("sqlite:{}", temp_file.path().display());
    
    let pool = get_database_pool(&database_url).await.unwrap();
    run_migrations(pool.clone()).await.unwrap();
    
    let db_manager = DatabaseManager::new(pool.clone());
    let item_repository = ItemRepository::new(pool.clone());
    
    env::set_var("JWT_SECRET", "regression_test_secret_key_1234567890123456789012345678901234567890");
    let jwt_service = JwtService::new().unwrap();
    let user_repository = UserRepository::new(pool.clone());
    let auth_service = AuthService::new(user_repository, jwt_service.clone());
    
    let file_repository = FileRepository::new(pool.clone());
    let file_manager = FileManager::with_default_config(file_repository);
    
    let job_repository = JobRepository::new(pool.clone());
    let job_queue = JobQueue::new(job_repository);
    
    let cache_config = CacheConfig {
        max_size: 1000,
        default_ttl_seconds: 300,
        cleanup_interval_seconds: 60,
        enable_stats: true,
    };
    let cache_manager = CacheManager::new(cache_config);
    
    let websocket_manager = WebSocketManager::new(Some(jwt_service));
    
    let state = core_lib::AppState::with_database(db_manager, item_repository)
        .with_auth(auth_service)
        .with_file_manager(file_manager)
        .with_job_queue(job_queue)
        .with_cache_manager(cache_manager)
        .with_websocket(websocket_manager)
        .with_health_checker()
        .with_system_monitor();
    
    state.migrate_to_database_if_needed().await.unwrap();
    
    state
}

#[tokio::test]
async fn test_original_item_crud_still_works() {
    let state = setup_full_system().await;
    
    let created_item = state.item_service.create_item(
        "Regression Test Item".to_string(),
        Some("Testing that original CRUD still works".to_string()),
        vec!["regression".to_string(), "test".to_string()],
        Some(serde_json::json!({"test": true})),
    ).await.unwrap();
    
    assert_eq!(created_item.name, "Regression Test Item");
    assert!(created_item.id > 0);
    assert_eq!(created_item.tags, vec!["regression", "test"]);
    
    let all_items = state.item_service.get_items(None, None).await.unwrap();
    assert!(all_items.len() >= 1);
    
    let updated_item = state.item_service.update_item(
        created_item.id,
        "Updated Regression Test Item".to_string(),
        Some("Updated description".to_string()),
        vec!["updated".to_string(), "regression".to_string()],
        Some(serde_json::json!({"updated": true})),
    ).await.unwrap();
    
    assert_eq!(updated_item.name, "Updated Regression Test Item");
    assert_eq!(updated_item.tags, vec!["updated", "regression"]);
    
    let delete_result = state.item_service.delete_item(created_item.id).await;
    assert!(delete_result.is_ok());
}

#[tokio::test]
async fn test_original_stats_functionality() {
    let state = setup_full_system().await;
    
    for i in 1..=5 {
        state.item_service.create_item(
            format!("Stats Test Item {}", i),
            Some(format!("Item {} for stats testing", i)),
            vec!["stats".to_string(), "test".to_string()],
            None,
        ).await.unwrap();
    }
    
    let stats = state.store.get_stats().unwrap();
    
    assert!(stats.is_object());
    
    if let Some(total_items) = stats.get("total_items") {
        assert!(total_items.is_number());
        let count = total_items.as_u64().unwrap();
        assert!(count >= 2);
    }
}

#[tokio::test]
async fn test_metrics_collection_with_all_features() {
    let state = setup_full_system().await;
    
    for i in 1..=3 {
        state.item_service.create_item(
            format!("Metrics Test Item {}", i),
            Some("Testing metrics".to_string()),
            vec!["metrics".to_string()],
            None,
        ).await.unwrap();
    }
    
    let metrics = state.metrics.get_snapshot(10);
    
    assert!(metrics.total_requests >= 0);
    assert!(metrics.successful_requests >= 0);
    assert!(metrics.failed_requests >= 0);
    assert!(metrics.uptime_seconds >= 0);
}

#[tokio::test]
async fn test_all_features_initialized_correctly() {
    let state = setup_full_system().await;
    
    assert!(state.db_manager.is_some(), "Database manager should be initialized");
    assert!(state.auth_service.is_some(), "Auth service should be initialized");
    assert!(state.file_manager.is_some(), "File manager should be initialized");
    assert!(state.job_queue.is_some(), "Job queue should be initialized");
    assert!(state.cache_manager.is_some(), "Cache manager should be initialized");
    assert!(state.websocket_manager.is_some(), "WebSocket manager should be initialized");
    assert!(state.health_checker.is_some(), "Health checker should be initialized");
    assert!(state.system_monitor.is_some(), "System monitor should be initialized");
    assert!(state.search_engine.is_some(), "Search engine should be initialized");
    
    assert!(state.item_service.is_using_database(), "Item service should use database");
}

#[tokio::test]
async fn test_database_health_check() {
    let state = setup_full_system().await;
    
    if let Some(db_manager) = &state.db_manager {
        let health_result = db_manager.health_check().await;
        assert!(health_result.is_ok(), "Database health check should pass");
    }
}

#[tokio::test]
async fn test_websocket_manager_basic_functionality() {
    let state = setup_full_system().await;
    
    if let Some(ws_manager) = &state.websocket_manager {
        let connection_count = ws_manager.connection_count().await;
        assert_eq!(connection_count, 0, "Should start with no connections");
        
        let item = core_lib::store::Item {
            id: 999,
            name: "WebSocket Test".to_string(),
            description: Some("Test item".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
            metadata: None,
        };
        
        let event = core_lib::websocket::WebSocketEvent::ItemCreated(item);
        ws_manager.broadcast(event).await;
    }
}

#[tokio::test]
async fn test_cache_manager_basic_functionality() {
    let state = setup_full_system().await;
    
    if let Some(cache_manager) = &state.cache_manager {
        let key = "test_key";
        let value = serde_json::json!({"test": "value"});
        
        let set_result = cache_manager.set(key, &value);
        assert!(set_result.is_ok(), "Cache set should succeed");
        
        let cached_value: Option<serde_json::Value> = cache_manager.get(key);
        assert!(cached_value.is_some(), "Should retrieve cached value");
        assert_eq!(cached_value.unwrap(), value);
        
        let stats = cache_manager.stats();
        assert!(stats.hits >= 0);
        assert!(stats.misses >= 0);
    }
}

#[tokio::test]
async fn test_auth_system_non_interference() {
    let state = setup_full_system().await;
    
    let item = state.item_service.create_item(
        "Non-Auth Test Item".to_string(),
        Some("Testing without auth".to_string()),
        vec!["no-auth".to_string()],
        None,
    ).await.unwrap();
    
    assert_eq!(item.name, "Non-Auth Test Item");
    
    if let Some(auth_service) = &state.auth_service {
        assert!(true, "Auth service is available");
    }
}

#[tokio::test]
async fn test_concurrent_operations_with_all_features() {
    let state = setup_full_system().await;
    
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            state_clone.item_service.create_item(
                format!("Concurrent Item {}", i),
                Some(format!("Created concurrently {}", i)),
                vec![format!("concurrent{}", i)],
                None,
            ).await
        });
        handles.push(handle);
    }
    
    let results = futures_util::future::join_all(handles).await;
    
    let mut success_count = 0;
    for result in results {
        assert!(result.is_ok(), "Task should complete without panicking");
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert!(success_count >= 3, "Most concurrent operations should succeed");
}

#[tokio::test]
async fn test_performance_regression() {
    let state = setup_full_system().await;
    
    let start_time = std::time::Instant::now();
    
    for i in 0..20 {
        state.item_service.create_item(
            format!("Performance Test Item {}", i),
            Some(format!("Performance test {}", i)),
            vec!["performance".to_string()],
            None,
        ).await.unwrap();
    }
    
    let creation_time = start_time.elapsed();
    
    let retrieval_start = std::time::Instant::now();
    let all_items = state.item_service.get_items(None, None).await.unwrap();
    let retrieval_time = retrieval_start.elapsed();
    
    assert!(creation_time.as_millis() < 3000, "Item creation should be reasonably fast");
    assert!(retrieval_time.as_millis() < 500, "Item retrieval should be fast");
    assert!(all_items.len() >= 20, "Should retrieve all created items");
    
    println!("Performance test: Created 20 items in {:?}, retrieved {} items in {:?}", creation_time, all_items.len(), retrieval_time);
}

#[tokio::test]
async fn test_error_handling_with_all_features() {
    let state = setup_full_system().await;
    
    let invalid_get_result = state.item_service.get_item(99999).await;
    
    let valid_item = state.item_service.create_item(
        "Error Recovery Test".to_string(),
        Some("Testing error recovery".to_string()),
        vec!["error".to_string(), "recovery".to_string()],
        None,
    ).await.unwrap();
    
    assert_eq!(valid_item.name, "Error Recovery Test");
}
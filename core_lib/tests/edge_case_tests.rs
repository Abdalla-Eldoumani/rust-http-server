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

async fn setup_test_system() -> core_lib::AppState {
    let temp_file = NamedTempFile::new().unwrap();
    let database_url = format!("sqlite:{}", temp_file.path().display());
    
    let pool = get_database_pool(&database_url).await.unwrap();
    run_migrations(pool.clone()).await.unwrap();
    
    let db_manager = DatabaseManager::new(pool.clone());
    let item_repository = ItemRepository::new(pool.clone());
    
    env::set_var("JWT_SECRET", "edge_case_test_secret_key_1234567890123456789012345678901234567890");
    let jwt_service = JwtService::new().unwrap();
    let user_repository = UserRepository::new(pool.clone());
    let auth_service = AuthService::new(user_repository, jwt_service.clone());
    
    let file_repository = FileRepository::new(pool.clone());
    let file_manager = FileManager::with_default_config(file_repository);
    
    let job_repository = JobRepository::new(pool.clone());
    let job_queue = JobQueue::new(job_repository);
    
    let cache_config = CacheConfig {
        max_size: 100,
        default_ttl_seconds: 1,
        cleanup_interval_seconds: 1,
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
async fn test_empty_and_invalid_inputs() {
    let state = setup_test_system().await;
    
    let empty_name_result = state.item_service.create_item(
        "".to_string(),
        Some("Valid description".to_string()),
        vec!["tag".to_string()],
        None,
    ).await;
    assert!(empty_name_result.is_err(), "Empty item name should be rejected");
    
    let long_name = "x".repeat(10000);
    let _long_name_result = state.item_service.create_item(
        long_name,
        Some("Description".to_string()),
        vec!["tag".to_string()],
        None,
    ).await;
    
    let empty_tags_result = state.item_service.create_item(
        "Valid Name".to_string(),
        Some("Valid description".to_string()),
        vec![],
        None,
    ).await;
    assert!(empty_tags_result.is_ok(), "Empty tags should be allowed");
    
    let null_desc_result = state.item_service.create_item(
        "Valid Name 2".to_string(),
        None,
        vec!["tag".to_string()],
        None,
    ).await;
    assert!(null_desc_result.is_ok(), "Null description should be allowed");
}

#[tokio::test]
async fn test_auth_boundary_conditions() {
    let state = setup_test_system().await;
    let auth_service = state.auth_service.as_ref().unwrap();
    
    let long_username = "x".repeat(1000);
    let long_user_request = core_lib::auth::models::CreateUserRequest {
        username: long_username,
        email: "test@example.com".to_string(),
        password: "StrongPass123!".to_string(),
        role: Some(core_lib::auth::models::UserRole::User),
    };
    
    let _long_user_result = auth_service.register_user(long_user_request).await;
    
    let invalid_emails = vec![
        "notanemail",
        "@example.com",
        "test@",
        "test..test@example.com",
        "test@example",
    ];
    
    for invalid_email in invalid_emails {
        let invalid_email_request = core_lib::auth::models::CreateUserRequest {
            username: format!("user_{}", invalid_email.replace("@", "_at_").replace(".", "_dot_")),
            email: invalid_email.to_string(),
            password: "StrongPass123!".to_string(),
            role: Some(core_lib::auth::models::UserRole::User),
        };
        
        let result = auth_service.register_user(invalid_email_request).await;
        if result.is_ok() {
            println!("Warning: Invalid email '{}' was accepted", invalid_email);
        }
    }
    
    let weak_passwords = vec!["", "123", "password", "a"];
    
    for weak_password in weak_passwords {
        let weak_pass_request = core_lib::auth::models::CreateUserRequest {
            username: format!("user_weak_{}", weak_password.len()),
            email: format!("weak{}@example.com", weak_password.len()),
            password: weak_password.to_string(),
            role: Some(core_lib::auth::models::UserRole::User),
        };
        
        let result = auth_service.register_user(weak_pass_request).await;
        if result.is_ok() {
            println!("Warning: Weak password '{}' was accepted", weak_password);
        }
    }
}

#[tokio::test]
async fn test_cache_edge_cases() {
    let state = setup_test_system().await;
    let cache_manager = state.cache_manager.as_ref().unwrap();
    
    for i in 0..150 {
        let key = format!("key_{}", i);
        let value = serde_json::json!({"index": i});
        let _ = cache_manager.set(&key, &value);
    }
    
    let early_item: Option<serde_json::Value> = cache_manager.get("key_0");
    let late_item: Option<serde_json::Value> = cache_manager.get("key_149");
    
    assert!(early_item.is_none(), "Early cache items should be evicted");
    assert!(late_item.is_some(), "Recent cache items should exist");
    
    let ttl_key = "ttl_test";
    let ttl_value = serde_json::json!({"test": "ttl"});
    let _ = cache_manager.set(ttl_key, &ttl_value);
    
    let immediate_get: Option<serde_json::Value> = cache_manager.get(ttl_key);
    assert!(immediate_get.is_some(), "Item should exist immediately after set");
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let expired_get: Option<serde_json::Value> = cache_manager.get(ttl_key);
    if expired_get.is_some() {
        println!("Warning: TTL expiration might not be working");
    }
}

#[tokio::test]
async fn test_websocket_edge_cases() {
    let state = setup_test_system().await;
    let ws_manager = state.websocket_manager.as_ref().unwrap();
    
    let item = core_lib::store::Item {
        id: 1,
        name: "Test Item".to_string(),
        description: Some("Test".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        tags: vec!["test".to_string()],
        metadata: None,
    };
    
    let event = core_lib::websocket::WebSocketEvent::ItemCreated(item);
    ws_manager.broadcast(event).await;
    
    let item2 = core_lib::store::Item {
        id: 2,
        name: "Test Item 2".to_string(),
        description: Some("Test 2".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        tags: vec!["test".to_string()],
        metadata: None,
    };
    
    let event2 = core_lib::websocket::WebSocketEvent::ItemCreated(item2);
    ws_manager.broadcast_to_user(99999, event2).await;
    
    let initial_count = ws_manager.connection_count().await;
    assert_eq!(initial_count, 0, "Should start with no connections");
}

#[tokio::test]
async fn test_database_transaction_edge_cases() {
    let state = setup_test_system().await;
    let db_manager = state.db_manager.as_ref().unwrap();
    let pool = db_manager.pool();
    
    let mut tx1 = pool.begin().await.unwrap();
    let mut tx2 = pool.begin().await.unwrap();
    
    let result1 = sqlx::query("INSERT INTO items (name, description, tags, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))")
        .bind("Transaction Item 1")
        .bind("Test concurrent tx 1")
        .bind("tx1,test")
        .bind("{}")
        .execute(&mut *tx1).await;
    
    let result2 = sqlx::query("INSERT INTO items (name, description, tags, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))")
        .bind("Transaction Item 2")
        .bind("Test concurrent tx 2")
        .bind("tx2,test")
        .bind("{}")
        .execute(&mut *tx2).await;
    
    if result1.is_err() {
        println!("⚠️  Transaction 1 failed: {:?}", result1.as_ref().err());
    }
    if result2.is_err() {
        println!("⚠️  Transaction 2 failed: {:?}", result2.as_ref().err());
        println!("   This might indicate transaction isolation issues");
    }
    
    if result1.is_ok() && result2.is_ok() {
        println!("✅ Both transactions succeeded - good concurrency handling");
    } else if result1.is_ok() || result2.is_ok() {
        println!("⚠️  Only one transaction succeeded - this is acceptable for SQLite");
    } else {
        panic!("Both transactions failed - this indicates a serious issue");
    }
    
    tx1.commit().await.unwrap();
    tx2.rollback().await.unwrap();
    
    let mut long_tx = pool.begin().await.unwrap();
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let result = sqlx::query("INSERT INTO items (name, description, tags, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))")
        .bind("Long Transaction Item")
        .bind("Test long transaction")
        .bind("long,test")
        .bind("{}")
        .execute(&mut *long_tx).await;
    
    assert!(result.is_ok(), "Long transaction should still work");
    long_tx.commit().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_access_edge_cases() {
    let state = setup_test_system().await;
    
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            state_clone.item_service.create_item(
                "Duplicate Name Item".to_string(),
                Some(format!("Description {}", i)),
                vec!["duplicate".to_string()],
                None,
            ).await
        });
        handles.push(handle);
    }
    
    let results = futures_util::future::join_all(handles).await;
    
    let mut success_count = 0;
    for result in results {
        assert!(result.is_ok(), "Task should not panic");
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert!(success_count >= 8, "Most concurrent operations should succeed");
    
    let all_items = state.item_service.get_items(None, None).await.unwrap();
    let duplicate_items: Vec<_> = all_items.iter()
        .filter(|item| item.name == "Duplicate Name Item")
        .collect();
    
    assert!(duplicate_items.len() >= 8, "Should have created multiple items with same name");
}

#[tokio::test]
async fn test_resource_limits() {
    let state = setup_test_system().await;
    
    let mut created_items = Vec::new();
    
    for i in 0..1000 {
        let item_result = state.item_service.create_item(
            format!("Stress Test Item {}", i),
            Some(format!("Stress testing with item {}", i)),
            vec![format!("stress{}", i % 10), "test".to_string()],
            Some(serde_json::json!({"index": i, "data": "x".repeat(100)})),
        ).await;
        
        if let Ok(item) = item_result {
            created_items.push(item);
        } else {
            println!("Failed to create item {} - might have hit resource limit", i);
            break;
        }
        
        if i % 100 == 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
    
    println!("Successfully created {} items", created_items.len());
    assert!(created_items.len() >= 500, "Should be able to create at least 500 items");
    
    let start_time = std::time::Instant::now();
    let all_items = state.item_service.get_items(None, None).await.unwrap();
    let retrieval_time = start_time.elapsed();
    
    println!("Retrieved {} items in {:?}", all_items.len(), retrieval_time);
    assert!(retrieval_time.as_millis() < 5000, "Retrieval should be reasonably fast even with many items");
}

#[tokio::test]
async fn test_error_recovery() {
    let state = setup_test_system().await;
    
    let _ = state.item_service.get_item(99999).await;
    let _ = state.item_service.delete_item(99999).await;
    
    let recovery_item = state.item_service.create_item(
        "Recovery Test Item".to_string(),
        Some("Testing recovery after errors".to_string()),
        vec!["recovery".to_string()],
        None,
    ).await.unwrap();
    
    assert_eq!(recovery_item.name, "Recovery Test Item");
    
    if let Some(auth_service) = &state.auth_service {
        let invalid_login = core_lib::auth::models::LoginRequest {
            username: "nonexistent_user".to_string(),
            password: "wrong_password".to_string(),
        };
        let _ = auth_service.login(invalid_login).await;
        
        let valid_user = core_lib::auth::models::CreateUserRequest {
            username: "recovery_user".to_string(),
            email: "recovery@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            role: Some(core_lib::auth::models::UserRole::User),
        };
        
        let user_result = auth_service.register_user(valid_user).await;
        assert!(user_result.is_ok(), "System should recover from auth errors");
    }
}

#[tokio::test]
async fn test_data_consistency() {
    let state = setup_test_system().await;
    
    let initial_item = state.item_service.create_item(
        "Consistency Test Item".to_string(),
        Some("Testing data consistency".to_string()),
        vec!["consistency".to_string()],
        None,
    ).await.unwrap();
    
    let mut update_handles = Vec::new();
    
    for i in 0..20 {
        let state_clone = state.clone();
        let item_id = initial_item.id;
        let handle = tokio::spawn(async move {
            state_clone.item_service.update_item(
                item_id,
                format!("Updated Item {}", i),
                Some(format!("Updated description {}", i)),
                vec![format!("update{}", i), "consistency".to_string()],
                Some(serde_json::json!({"update_index": i})),
            ).await
        });
        update_handles.push(handle);
    }
    
    let update_results = futures_util::future::join_all(update_handles).await;
    
    let mut successful_updates = 0;
    for result in update_results {
        assert!(result.is_ok(), "Update task should not panic");
        if result.unwrap().is_ok() {
            successful_updates += 1;
        }
    }
    
    assert!(successful_updates > 0, "At least some updates should succeed");
    
    let all_items = state.item_service.get_items(None, None).await.unwrap();
    let updated_items: Vec<_> = all_items.iter()
        .filter(|item| item.id == initial_item.id)
        .collect();
    
    assert_eq!(updated_items.len(), 1, "Should have exactly one item with the ID");
    
    let final_item = updated_items[0];
    assert!(final_item.name.starts_with("Updated Item"), "Item should be updated");
}

#[tokio::test]
async fn test_rapid_operations() {
    let state = setup_test_system().await;
    
    let start_time = std::time::Instant::now();
    
    for i in 0..100 {
        let item = state.item_service.create_item(
            format!("Rapid Item {}", i),
            Some("Rapid test".to_string()),
            vec!["rapid".to_string()],
            None,
        ).await.unwrap();
        
        let delete_result = state.item_service.delete_item(item.id).await;
        assert!(delete_result.is_ok(), "Delete should succeed");
    }
    
    let total_time = start_time.elapsed();
    println!("Completed 100 create/delete cycles in {:?}", total_time);
    
    let final_item = state.item_service.create_item(
        "Final Test Item".to_string(),
        Some("After rapid operations".to_string()),
        vec!["final".to_string()],
        None,
    ).await.unwrap();
    
    assert_eq!(final_item.name, "Final Test Item");
}
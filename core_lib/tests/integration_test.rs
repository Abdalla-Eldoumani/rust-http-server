use core_lib::{get_database_pool, run_migrations, DatabaseManager, ItemRepository, AppState};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_database_integration() {
    let temp_file = NamedTempFile::new().unwrap();
    let database_url = format!("sqlite:{}", temp_file.path().display());
    
    println!("Testing database integration with: {}", database_url);
    
    let pool = get_database_pool(&database_url).await.unwrap();
    run_migrations(pool.clone()).await.unwrap();
    
    let db_manager = DatabaseManager::new(pool.clone());
    let item_repository = ItemRepository::new(pool);
    
    let state = AppState::with_database(db_manager, item_repository);
    
    println!("Database initialized successfully");
    assert!(state.item_service.is_using_database());
    
    let items_before = state.item_service.get_items(None, None).await.unwrap();
    println!("Items before migration: {}", items_before.len());
    
    state.migrate_to_database_if_needed().await.unwrap();
    println!("Migration completed");
    
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    let items_after = state.item_service.get_items(None, None).await.unwrap();
    println!("Items after migration: {}", items_after.len());
    assert_eq!(items_after.len(), 2);
    
    let new_item = state.item_service.create_item(
        "Test Item".to_string(),
        Some("Created via database integration test".to_string()),
        vec!["test".to_string(), "integration".to_string()],
        None,
    ).await.unwrap();
    
    println!("Created new item: {} (ID: {})", new_item.name, new_item.id);
    assert_eq!(new_item.name, "Test Item");
    
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    let final_items = state.item_service.get_items(None, None).await.unwrap();
    println!("Final item count: {}", final_items.len());
        
    assert_eq!(final_items.len(), 3);
    
    let health_result = state.db_manager.as_ref().unwrap().health_check().await;
    assert!(health_result.is_ok());
    println!(" Database health check passed");
    
    println!("\nDatabase integration test completed successfully!");
}
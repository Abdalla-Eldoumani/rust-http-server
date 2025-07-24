use core_lib::{
    get_database_pool, run_migrations, DatabaseManager, ItemRepository,
    auth::{JwtService, UserRepository, AuthService, UserRepositoryTrait},
    database::{
        repository::{Repository, ListParams},
        CreateItemInput, UpdateItemInput,
    },
};
use tempfile::NamedTempFile;
use sqlx::Row;
use std::env;

async fn setup_test_database() -> sqlx::SqlitePool {
    let temp_file = NamedTempFile::new().unwrap();
    let database_url = format!("sqlite:{}", temp_file.path().display());
    
    let pool = get_database_pool(&database_url).await.unwrap();
    run_migrations(pool.clone()).await.unwrap();
    
    pool
}

#[tokio::test]
async fn test_basic_database_operations() {
    let pool = setup_test_database().await;
    
    let result = sqlx::query("SELECT 1").fetch_one(&pool).await;
    assert!(result.is_ok());
    
    let tables: Vec<String> = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get::<String, _>("name"))
        .collect();
    
    assert!(tables.contains(&"items".to_string()));
    assert!(tables.contains(&"users".to_string()));
}

#[tokio::test]
async fn test_item_repository_operations() {
    let pool = setup_test_database().await;
    let item_repository = ItemRepository::new(pool.clone());
    
    let create_input = CreateItemInput {
        name: "Test Item".to_string(),
        description: Some("Test Description".to_string()),
        tags: vec!["test".to_string()],
        metadata: Some(serde_json::json!({"test": true})),
        created_by: None,
    };
    
    let created_item = item_repository.create(create_input).await.unwrap();
    assert_eq!(created_item.name, "Test Item");
    assert!(created_item.id > 0);
    
    let _count = item_repository.count().await.unwrap();
    
    let retrieved_item = item_repository.get_by_id(created_item.id as i64).await.unwrap();
    assert!(retrieved_item.is_some(), "Failed to retrieve item with id: {}", created_item.id);
    let retrieved_item = retrieved_item.unwrap();
    assert_eq!(retrieved_item.name, "Test Item");
    
    let update_input = UpdateItemInput {
        name: "Updated Test Item".to_string(),
        description: Some("Updated Description".to_string()),
        tags: vec!["updated".to_string()],
        metadata: Some(serde_json::json!({"updated": true})),
    };
    
    let updated_item = item_repository.update(created_item.id as i64, update_input).await.unwrap();
    assert_eq!(updated_item.name, "Updated Test Item");
    
    let params = ListParams::default();
    let items = item_repository.list(params).await.unwrap();
    assert!(items.len() >= 1);
    
    let count = item_repository.count().await.unwrap();
    assert!(count >= 1);
    
    let search_params = ListParams::default();
    let search_results = item_repository.search("Test", search_params).await.unwrap();
    assert!(search_results.len() >= 1);
    
    item_repository.delete(created_item.id as i64).await.unwrap();
    
    let deleted_item = item_repository.get_by_id(created_item.id as i64).await.unwrap();
    assert!(deleted_item.is_none());
}

#[tokio::test]
async fn test_user_repository_operations() {
    let pool = setup_test_database().await;
    let user_repository = UserRepository::new(pool.clone());
    
    user_repository.ensure_tables_exist().await.unwrap();
    
    let create_request = core_lib::auth::models::CreateUserRequest {
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        password: "StrongPass123!".to_string(),
        role: Some(core_lib::auth::models::UserRole::User),
    };
    
    let created_user = user_repository.create_user(&create_request, "hashed_password").await.unwrap();
    assert_eq!(created_user.username, "testuser");
    assert_eq!(created_user.email, "test@example.com");
    assert!(created_user.id > 0);
    
    let retrieved_user = user_repository.get_user_by_id(created_user.id).await.unwrap();
    assert!(retrieved_user.is_some());
    let retrieved_user = retrieved_user.unwrap();
    assert_eq!(retrieved_user.username, "testuser");
    
    let user_by_username = user_repository.get_user_by_username("testuser").await.unwrap();
    assert!(user_by_username.is_some());
    let user_by_username = user_by_username.unwrap();
    assert_eq!(user_by_username.id, created_user.id);
    
    let user_by_email = user_repository.get_user_by_email("test@example.com").await.unwrap();
    assert!(user_by_email.is_some());
    let user_by_email = user_by_email.unwrap();
    assert_eq!(user_by_email.id, created_user.id);
    
    user_repository.update_last_login(created_user.id).await.unwrap();
    
    let updated_user = user_repository.get_user_by_id(created_user.id).await.unwrap().unwrap();
    assert!(updated_user.last_login.is_some());
    
    user_repository.update_user_status(created_user.id, false).await.unwrap();
    let inactive_user = user_repository.get_user_by_id(created_user.id).await.unwrap().unwrap();
    assert!(!inactive_user.is_active);
    
    user_repository.update_user_status(created_user.id, true).await.unwrap();
    let active_user = user_repository.get_user_by_id(created_user.id).await.unwrap().unwrap();
    assert!(active_user.is_active);
    
    let users = user_repository.list_users(Some(10), Some(0)).await.unwrap();
    assert!(users.len() >= 1);
}

#[tokio::test]
async fn test_auth_service_integration() {
    let pool = setup_test_database().await;
    
    env::set_var("JWT_SECRET", "test_secret_key_for_auth_integration_1234567890123456789012345678901234567890");
    let jwt_service = JwtService::new().unwrap();
    let user_repository = UserRepository::new(pool.clone());
    let auth_service = AuthService::new(user_repository, jwt_service);
    
    let create_request = core_lib::auth::models::CreateUserRequest {
        username: "authtest".to_string(),
        email: "authtest@example.com".to_string(),
        password: "StrongPass123!".to_string(),
        role: Some(core_lib::auth::models::UserRole::User),
    };
    
    let user_response = auth_service.register_user(create_request).await.unwrap();
    assert_eq!(user_response.username, "authtest");
    
    let login_request = core_lib::auth::models::LoginRequest {
        username: "authtest".to_string(),
        password: "StrongPass123!".to_string(),
    };
    
    let login_response = auth_service.login(login_request).await.unwrap();
    assert_eq!(login_response.user.username, "authtest");
    assert!(!login_response.access_token.is_empty());
    assert!(!login_response.refresh_token.is_empty());
    
    let token_validation = auth_service.validate_token(&login_response.access_token).await;
    assert!(token_validation.is_ok());
    
    let claims = token_validation.unwrap();
    assert_eq!(claims.username, "authtest");
    
    let refresh_response = auth_service.refresh_token(&login_response.refresh_token).await.unwrap();
    assert!(!refresh_response.access_token.is_empty());
    
    let invalid_login_request = core_lib::auth::models::LoginRequest {
        username: "authtest".to_string(),
        password: "wrongpassword".to_string(),
    };
    
    let invalid_result = auth_service.login(invalid_login_request).await;
    assert!(invalid_result.is_err());
}

#[tokio::test]
async fn test_database_transactions() {
    let pool = setup_test_database().await;
    let item_repository = ItemRepository::new(pool.clone());
    
    let mut tx = pool.begin().await.unwrap();
    
    let result = sqlx::query(
        "INSERT INTO items (name, description, tags, metadata, created_at, updated_at) 
         VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))"
    )
    .bind("Transaction Test Item")
    .bind("This should be rolled back")
    .bind("transaction,test")
    .bind("{\"test\": true}")
    .execute(&mut *tx).await;
    
    assert!(result.is_ok());
    
    tx.rollback().await.unwrap();
    
    let search_params = ListParams::default();
    let items = item_repository.search("Transaction Test", search_params).await.unwrap();
    assert_eq!(items.len(), 0);
    
    let mut tx = pool.begin().await.unwrap();
    
    let result = sqlx::query(
        "INSERT INTO items (name, description, tags, metadata, created_at, updated_at) 
         VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))"
    )
    .bind("Committed Transaction Item")
    .bind("This should be committed")
    .bind("transaction,committed")
    .bind("{\"committed\": true}")
    .execute(&mut *tx).await;
    
    assert!(result.is_ok());
    
    tx.commit().await.unwrap();
    
    let search_params = ListParams::default();
    let items = item_repository.search("Committed Transaction", search_params).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Committed Transaction Item");
}

#[tokio::test]
async fn test_concurrent_database_operations() {
    let pool = setup_test_database().await;
    let item_repository = ItemRepository::new(pool.clone());
    
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let repo = item_repository.clone();
        let handle = tokio::spawn(async move {
            let create_input = CreateItemInput {
                name: format!("Concurrent Item {}", i),
                description: Some(format!("Created concurrently {}", i)),
                tags: vec![format!("concurrent{}", i), "test".to_string()],
                metadata: Some(serde_json::json!({"index": i})),
                created_by: None,
            };
            repo.create(create_input).await
        });
        handles.push(handle);
    }
    
    let results = futures_util::future::join_all(handles).await;
    
    let mut successful_count = 0;
    for result in results {
        assert!(result.is_ok());
        if result.unwrap().is_ok() {
            successful_count += 1;
        }
    }
    
    let params = ListParams::default();
    let all_items = item_repository.list(params).await.unwrap();
    let concurrent_items: Vec<_> = all_items.iter()
        .filter(|item| item.name.starts_with("Concurrent Item"))
        .collect();
    
    assert!(concurrent_items.len() >= 3, "Expected at least 3 concurrent items, got {}", concurrent_items.len());
    assert!(concurrent_items.len() <= 5, "Expected at most 5 concurrent items, got {}", concurrent_items.len());
}

#[tokio::test]
async fn test_database_manager_health_check() {
    let pool = setup_test_database().await;
    let db_manager = DatabaseManager::new(pool);
    
    let health_result = db_manager.health_check().await;
    assert!(health_result.is_ok());
    
    let pool_ref = db_manager.pool();
    let test_query = sqlx::query("SELECT 1").fetch_one(pool_ref).await;
    assert!(test_query.is_ok());
}
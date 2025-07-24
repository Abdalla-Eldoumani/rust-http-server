use core_lib::{
    get_database_pool, run_migrations,
    auth::{JwtService, UserRepository, AuthService, models::CreateUserRequest, models::UserRole},
};
use tempfile::NamedTempFile;
use std::env;

async fn setup_auth_service() -> AuthService {
    let temp_file = NamedTempFile::new().unwrap();
    let database_url = format!("sqlite:{}", temp_file.path().display());
    
    let pool = get_database_pool(&database_url).await.unwrap();
    run_migrations(pool.clone()).await.unwrap();
    
    env::set_var("JWT_SECRET", "test_secret_key_1234567890123456789012345678901234567890");
    let jwt_service = JwtService::new().unwrap();
    let user_repository = UserRepository::new(pool.clone());
    
    AuthService::new(user_repository, jwt_service)
}

#[tokio::test]
async fn test_password_validation_fixes() {
    let auth_service = setup_auth_service().await;
    
    println!("=== TESTING PASSWORD VALIDATION FIXES ===");
    
    let weak_passwords = vec![
        ("", "empty password"),
        ("123", "too short"),
        ("password", "common weak password"),
        ("12345678", "numeric only"),
        ("abcdefgh", "letters only"),
        ("PASSWORD", "uppercase only"),
        ("Password", "missing digit and special char"),
        ("Password1", "missing special char"),
    ];
    
    for (password, description) in weak_passwords {
        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: password.to_string(),
            role: Some(UserRole::User),
        };
        
        let result = auth_service.register_user(request).await;
        match result {
            Ok(_) => println!("❌ FAILED: {} was accepted", description),
            Err(_) => println!("✅ PASSED: {} correctly rejected", description),
        }
    }
    
    let strong_request = CreateUserRequest {
        username: "stronguser".to_string(),
        email: "strong@example.com".to_string(),
        password: "StrongPass123!".to_string(),
        role: Some(UserRole::User),
    };
    
    let result = auth_service.register_user(strong_request).await;
    match result {
        Ok(_) => println!("✅ PASSED: Strong password accepted"),
        Err(e) => println!("❌ FAILED: Strong password rejected: {:?}", e),
    }
}

#[tokio::test]
async fn test_email_validation_fixes() {
    let auth_service = setup_auth_service().await;
    
    println!("=== TESTING EMAIL VALIDATION FIXES ===");
    
    let invalid_emails = vec![
        ("", "empty email"),
        ("notanemail", "no @ symbol"),
        ("@example.com", "no local part"),
        ("test@", "no domain"),
        ("test..test@example.com", "consecutive dots"),
        ("test@example", "no TLD"),
        ("test@.com", "domain starts with dot"),
        (".test@example.com", "local starts with dot"),
        ("test.@example.com", "local ends with dot"),
    ];
    
    for (email, description) in invalid_emails {
        let request = CreateUserRequest {
            username: format!("user_{}", description.replace(" ", "_")),
            email: email.to_string(),
            password: "StrongPass123!".to_string(),
            role: Some(UserRole::User),
        };
        
        let result = auth_service.register_user(request).await;
        match result {
            Ok(_) => println!("❌ FAILED: {} was accepted", description),
            Err(_) => println!("✅ PASSED: {} correctly rejected", description),
        }
    }
    
    let valid_request = CreateUserRequest {
        username: "validuser".to_string(),
        email: "valid@example.com".to_string(),
        password: "StrongPass123!".to_string(),
        role: Some(UserRole::User),
    };
    
    let result = auth_service.register_user(valid_request).await;
    match result {
        Ok(_) => println!("✅ PASSED: Valid email accepted"),
        Err(e) => println!("❌ FAILED: Valid email rejected: {:?}", e),
    }
}
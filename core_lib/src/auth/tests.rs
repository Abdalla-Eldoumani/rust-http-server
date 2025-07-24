#[cfg(test)]
mod tests {
    use crate::auth::{
        jwt::JwtService,
        models::{CreateUserRequest, LoginRequest, User, UserRole},
        repository::{UserRepository, UserRepositoryTrait},
        service::AuthService,
    };
    use chrono::Utc;
    use sqlx::SqlitePool;
    use std::env;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let user_repo = UserRepository::new(pool.clone());
        user_repo.ensure_tables_exist().await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_jwt_service_creation() {
        env::set_var("JWT_SECRET", "cccb02dcfa1318ea49f54c76211210b822f8542d");
        let jwt_service = JwtService::new();
        assert!(jwt_service.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_token_generation_and_validation() {
        env::set_var("JWT_SECRET", "d85735aadb9a3e089ae7a06f417ed32080376f24");
        let jwt_service = JwtService::new().unwrap();

        let user = User {
            id: 1,
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            role: "user".to_string(),
            created_at: Utc::now(),
            last_login: None,
            is_active: true,
        };

        let access_token = jwt_service.generate_access_token(&user).unwrap();
        let claims = jwt_service.validate_access_token(&access_token).unwrap();
        assert_eq!(claims.sub, "1");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.token_type, "access");

        let refresh_token = jwt_service.generate_refresh_token(&user).unwrap();
        let claims = jwt_service.validate_refresh_token(&refresh_token).unwrap();
        assert_eq!(claims.sub, "1");
        assert_eq!(claims.token_type, "refresh");
    }

    #[tokio::test]
    async fn test_user_repository_create_and_get() {
        let pool = setup_test_db().await;
        let user_repo = UserRepository::new(pool);

        let create_request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "StrongTest123!".to_string(),
            role: Some(UserRole::User),
        };

        let user = user_repo.create_user(&create_request, "hashed_password").await.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.role, "user");

        let retrieved_user = user_repo.get_user_by_id(user.id).await.unwrap().unwrap();
        assert_eq!(retrieved_user.username, "testuser");

        let retrieved_by_username = user_repo.get_user_by_username("testuser").await.unwrap().unwrap();
        assert_eq!(retrieved_by_username.id, user.id);
    }

    #[tokio::test]
    async fn test_auth_service_registration() {
        env::set_var("JWT_SECRET", "fcb2e0cf59920daee6d502b120f27c5a7cb86385fcb2e0cf59920daee6d502b120f27c5a7cb86385");
        let pool = setup_test_db().await;
        let user_repo = UserRepository::new(pool);
        let jwt_service = JwtService::new().unwrap();
        let auth_service = AuthService::new(user_repo, jwt_service);

        let create_request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "StrongTest123!".to_string(),
            role: Some(UserRole::User),
        };

        let user_response = auth_service.register_user(create_request).await.unwrap();
        assert_eq!(user_response.username, "testuser");
        assert_eq!(user_response.email, "test@example.com");
        assert_eq!(user_response.role, UserRole::User);
    }

    #[tokio::test]
    async fn test_auth_service_login() {
        env::set_var("JWT_SECRET", "fcb2e0cf59920daee6d502b120f27c5a7cb86385");
        let pool = setup_test_db().await;
        let user_repo = UserRepository::new(pool);
        let jwt_service = JwtService::new().unwrap();
        let auth_service = AuthService::new(user_repo, jwt_service);

        let create_request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "StrongTest123!".to_string(),
            role: Some(UserRole::User),
        };
        auth_service.register_user(create_request).await.unwrap();

        let login_request = LoginRequest {
            username: "testuser".to_string(),
            password: "StrongTest123!".to_string(),
        };

        let login_response = auth_service.login(login_request).await.unwrap();
        assert_eq!(login_response.user.username, "testuser");
        assert_eq!(login_response.token_type, "Bearer");
        assert!(!login_response.access_token.is_empty());
        assert!(!login_response.refresh_token.is_empty());
    }

    #[tokio::test]
    async fn test_auth_service_invalid_login() {
        env::set_var("JWT_SECRET", "1a9e1a1d8f3e9613a555adea1881bbd11a9e1a1d8f3e9613a555adea1881bbd1");
        let pool = setup_test_db().await;
        let user_repo = UserRepository::new(pool);
        let jwt_service = JwtService::new().unwrap();
        let auth_service = AuthService::new(user_repo, jwt_service);

        let login_request = LoginRequest {
            username: "nonexistent".to_string(),
            password: "password123".to_string(),
        };

        let result = auth_service.login(login_request).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), crate::error::AppError::Authentication(_)));
    }

    #[tokio::test]
    async fn test_user_role_conversion() {
        assert_eq!(UserRole::Admin.to_string(), "admin");
        assert_eq!(UserRole::User.to_string(), "user");
        assert_eq!(UserRole::ReadOnly.to_string(), "readonly");

        assert_eq!("admin".parse::<UserRole>().unwrap(), UserRole::Admin);
        assert_eq!("user".parse::<UserRole>().unwrap(), UserRole::User);
        assert_eq!("readonly".parse::<UserRole>().unwrap(), UserRole::ReadOnly);

        assert!("invalid".parse::<UserRole>().is_err());
    }
}
use crate::auth::{
    models::{CreateUserRequest, LoginRequest, LoginResponse, RefreshTokenResponse, UserResponse}
};
use crate::error::AppError;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

pub async fn register_user(
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let auth_service = state
        .auth_service
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;
    
    let user_response = auth_service.register_user(request).await?;
    Ok((StatusCode::CREATED, Json(user_response)))
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let auth_service = state
        .auth_service
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;
    
    let login_response = auth_service.login(request).await?;
    Ok(Json(login_response))
}

pub async fn refresh_token(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<RefreshTokenResponse>, AppError> {
    let auth_service = state
        .auth_service
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;
    
    let refresh_response = auth_service
        .refresh_token(&request.refresh_token)
        .await?;
    Ok(Json(refresh_response))
}

pub async fn get_current_user(
    State(state): State<AppState>,
    axum::extract::Extension(auth_user): axum::extract::Extension<crate::middleware::auth::AuthUser>,
) -> Result<Json<UserResponse>, AppError> {
    let auth_service = state
        .auth_service
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;
    
    let user = auth_service
        .get_user_by_id(auth_user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;
    
    Ok(Json(user))
}

pub async fn logout_user() -> Result<Json<MessageResponse>, AppError> {
    Ok(Json(MessageResponse {
        message: "Successfully logged out".to_string(),
    }))
}

pub async fn get_user_by_id(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
) -> Result<Json<UserResponse>, AppError> {
    let auth_service = state
        .auth_service
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;
    
    let user = auth_service
        .get_user_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;
    
    Ok(Json(user))
}

pub fn create_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register_user))
        .route("/login", post(login_user))
        .route("/refresh", post(refresh_token))
        .route("/logout", post(logout_user))
        .route("/me", get(get_current_user))
        .route("/users/:id", get(get_user_by_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{
        jwt::JwtService,
        repository::UserRepository,
        service::AuthService,
    };
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use serde_json::json;
    use sqlx::SqlitePool;
    use std::env;
    use tower::ServiceExt;

    async fn setup_test_app_state() -> AppState {
        env::set_var("JWT_SECRET", "1a9e1a1d8f3e9613a555adea1881bbd1");
        
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let user_repo = UserRepository::new(pool);
        user_repo.ensure_tables_exist().await.unwrap();
        
        let jwt_service = JwtService::new().unwrap();
        let auth_service = AuthService::new(user_repo, jwt_service);
        
        AppState::default().with_auth(auth_service)
    }

    #[tokio::test]
    async fn test_register_endpoint() {
        let app_state = setup_test_app_state().await;
        let app = create_auth_routes().with_state(app_state);

        let request_body = json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123",
            "role": "user"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/register")
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_login_endpoint() {
        let app_state = setup_test_app_state().await;
        let app = create_auth_routes().with_state(app_state.clone());

        let register_body = json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123",
            "role": "user"
        });

        let register_request = Request::builder()
            .method(Method::POST)
            .uri("/register")
            .header("content-type", "application/json")
            .body(Body::from(register_body.to_string()))
            .unwrap();

        let app_clone = create_auth_routes().with_state(app_state);
        let _register_response = app_clone.oneshot(register_request).await.unwrap();

        let login_body = json!({
            "username": "testuser",
            "password": "password123"
        });

        let login_request = Request::builder()
            .method(Method::POST)
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::from(login_body.to_string()))
            .unwrap();

        let response = app.oneshot(login_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_login() {
        let app_state = setup_test_app_state().await;
        let app = create_auth_routes().with_state(app_state);

        let login_body = json!({
            "username": "nonexistent",
            "password": "wrongpassword"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::from(login_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_register_validation() {
        let app_state = setup_test_app_state().await;
        let app = create_auth_routes().with_state(app_state);

        let request_body = json!({
            "username": "testuser",
            "email": "invalid-email",
            "password": "password123",
            "role": "user"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/register")
            .header("content-type", "application/json")
            .body(Body::from(request_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
use crate::auth::models::{UserRole};
use crate::error::AppError;
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap},
    middleware::Next,
    response::Response,
};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub username: String,
    pub role: UserRole,
}

impl AuthUser {
    pub fn new(user_id: i64, username: String, role: UserRole) -> Self {
        Self {
            user_id,
            username,
            role,
        }
    }

    pub fn has_role(&self, required_role: &UserRole) -> bool {
        match (&self.role, required_role) {
            (UserRole::Admin, _) => true,
            (UserRole::User, UserRole::User) => true,
            (UserRole::User, UserRole::ReadOnly) => true,
            (UserRole::ReadOnly, UserRole::ReadOnly) => true,
            _ => false,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role, UserRole::Admin)
    }
}

pub async fn jwt_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_service = state
        .auth_service
        .as_ref()
        .ok_or_else(|| AppError::InternalServerError)?;

    let token = extract_token_from_header(request.headers())?;

    let claims = auth_service.jwt_service().validate_access_token(&token)?;

    let role: UserRole = claims.role.parse()
        .map_err(|_| AppError::Authentication("Invalid role in token".to_string()))?;

    let user_id: i64 = claims.sub.parse()
        .map_err(|_| AppError::Authentication("Invalid user ID in token".to_string()))?;

    let auth_user = AuthUser::new(user_id, claims.username, role);
    request.extensions_mut().insert(auth_user);

    Ok(next.run(request).await)
}

pub async fn optional_jwt_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_service = match state.auth_service.as_ref() {
        Some(service) => service,
        None => return Ok(next.run(request).await),
    };

    if let Ok(token) = extract_token_from_header(request.headers()) {
        if let Ok(claims) = auth_service.jwt_service().validate_access_token(&token) {
            if let (Ok(role), Ok(user_id)) = (
                claims.role.parse::<UserRole>(),
                claims.sub.parse::<i64>()
            ) {
                let auth_user = AuthUser::new(user_id, claims.username, role);
                request.extensions_mut().insert(auth_user);
            }
        }
    }

    Ok(next.run(request).await)
}

pub fn require_role(required_role: UserRole) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, AppError>> + Send>> + Clone {
    move |request: Request, next: Next| {
        let required_role = required_role.clone();
        Box::pin(async move {
            let auth_user = request
                .extensions()
                .get::<AuthUser>()
                .ok_or_else(|| AppError::Authentication("Authentication required".to_string()))?;

            if !auth_user.has_role(&required_role) {
                return Err(AppError::Authorization(format!(
                    "Insufficient permissions. Required: {:?}, User has: {:?}",
                    required_role, auth_user.role
                )));
            }

            Ok(next.run(request).await)
        })
    }
}

pub async fn require_admin(request: Request, next: Next) -> Result<Response, AppError> {
    let auth_user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or_else(|| AppError::Authentication("Authentication required".to_string()))?;

    if !auth_user.is_admin() {
        return Err(AppError::Authorization(
            "Admin access required".to_string(),
        ));
    }

    Ok(next.run(request).await)
}

pub async fn require_self_or_admin(
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or_else(|| AppError::Authentication("Authentication required".to_string()))?;

    let path = request.uri().path();
    let path_segments: Vec<&str> = path.split('/').collect();
    
    let requested_user_id = path_segments
        .iter()
        .position(|&segment| segment == "users")
        .and_then(|pos| path_segments.get(pos + 1))
        .and_then(|id_str| id_str.parse::<i64>().ok());

    if let Some(requested_id) = requested_user_id {
        if !auth_user.is_admin() && auth_user.user_id != requested_id {
            return Err(AppError::Authorization(
                "You can only access your own resources".to_string(),
            ));
        }
    }

    Ok(next.run(request).await)
}

fn extract_token_from_header(headers: &HeaderMap) -> Result<String, AppError> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .ok_or_else(|| AppError::Authentication("Missing Authorization header".to_string()))?
        .to_str()
        .map_err(|_| AppError::Authentication("Invalid Authorization header format".to_string()))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(AppError::Authentication(
            "Authorization header must start with 'Bearer '".to_string(),
        ));
    }

    let token = auth_header.strip_prefix("Bearer ").unwrap();
    
    if token.is_empty() {
        return Err(AppError::Authentication("Empty token".to_string()));
    }

    Ok(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{
        jwt::JwtService,
        models::{User, UserRole},
        repository::UserRepository,
        service::AuthService,
    };
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use chrono::Utc;
    use sqlx::SqlitePool;
    use std::env;
    use tower::ServiceExt;

    async fn setup_test_state() -> AppState {
        env::set_var("JWT_SECRET", "1a9e1a1d8f3e9613a555adea1881bbd1");
        
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let user_repo = UserRepository::new(pool);
        user_repo.ensure_tables_exist().await.unwrap();
        
        let jwt_service = JwtService::new().unwrap();
        let auth_service = AuthService::new(user_repo, jwt_service);
        
        AppState::default().with_auth(auth_service)
    }

    async fn create_test_token(state: &AppState, role: UserRole) -> String {
        let auth_service = state.auth_service.as_ref().unwrap();
        let jwt_service = auth_service.jwt_service();
        
        let user = User {
            id: 1,
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            role: role.to_string(),
            created_at: Utc::now(),
            last_login: None,
            is_active: true,
        };

        jwt_service.generate_access_token(&user).unwrap()
    }

    async fn test_handler() -> &'static str {
        "success"
    }

    #[tokio::test]
    async fn test_jwt_auth_middleware_success() {
        let state = setup_test_state().await;
        let token = create_test_token(&state, UserRole::User).await;

        let app = Router::new()
            .route("/protected", get(test_handler))
            .layer(middleware::from_fn_with_state(state.clone(), jwt_auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/protected")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_jwt_auth_middleware_missing_token() {
        let state = setup_test_state().await;

        let app = Router::new()
            .route("/protected", get(test_handler))
            .layer(middleware::from_fn_with_state(state.clone(), jwt_auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_jwt_auth_middleware_invalid_token() {
        let state = setup_test_state().await;

        let app = Router::new()
            .route("/protected", get(test_handler))
            .layer(middleware::from_fn_with_state(state.clone(), jwt_auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/protected")
            .header("Authorization", "Bearer invalid-token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_optional_auth_middleware_with_token() {
        let state = setup_test_state().await;
        let token = create_test_token(&state, UserRole::User).await;

        let app = Router::new()
            .route("/optional", get(test_handler))
            .layer(middleware::from_fn_with_state(state.clone(), optional_jwt_auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/optional")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_optional_auth_middleware_without_token() {
        let state = setup_test_state().await;

        let app = Router::new()
            .route("/optional", get(test_handler))
            .layer(middleware::from_fn_with_state(state.clone(), optional_jwt_auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/optional")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_auth_user_role_permissions() {
        let admin = AuthUser::new(1, "admin".to_string(), UserRole::Admin);
        let user = AuthUser::new(2, "user".to_string(), UserRole::User);
        let readonly = AuthUser::new(3, "readonly".to_string(), UserRole::ReadOnly);

        assert!(admin.has_role(&UserRole::Admin));
        assert!(admin.has_role(&UserRole::User));
        assert!(admin.has_role(&UserRole::ReadOnly));
        assert!(admin.is_admin());

        assert!(!user.has_role(&UserRole::Admin));
        assert!(user.has_role(&UserRole::User));
        assert!(user.has_role(&UserRole::ReadOnly));
        assert!(!user.is_admin());

        assert!(!readonly.has_role(&UserRole::Admin));
        assert!(!readonly.has_role(&UserRole::User));
        assert!(readonly.has_role(&UserRole::ReadOnly));
        assert!(!readonly.is_admin());
    }

    #[test]
    fn test_extract_token_from_header() {
        let mut headers = HeaderMap::new();
        
        assert!(extract_token_from_header(&headers).is_err());
        
        headers.insert(AUTHORIZATION, "Bearer valid-token-123".parse().unwrap());
        let token = extract_token_from_header(&headers).unwrap();
        assert_eq!(token, "valid-token-123");
        
        headers.insert(AUTHORIZATION, "Basic invalid".parse().unwrap());
        assert!(extract_token_from_header(&headers).is_err());
        
        headers.insert(AUTHORIZATION, "Bearer ".parse().unwrap());
        assert!(extract_token_from_header(&headers).is_err());
    }
}
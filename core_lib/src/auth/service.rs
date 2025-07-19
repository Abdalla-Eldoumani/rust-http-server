use crate::auth::jwt::JwtService;
use crate::auth::models::{
    CreateUserRequest, JwtClaims, LoginRequest, LoginResponse, RefreshTokenResponse,
    UserResponse,
};
use crate::auth::repository::{UserRepository, UserRepositoryTrait};
use crate::error::AppError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthService {
    user_repository: Arc<dyn UserRepositoryTrait + Send + Sync>,
    jwt_service: Arc<JwtService>,
    argon2: Argon2<'static>,
}

impl AuthService {
    pub fn new(user_repository: UserRepository, jwt_service: JwtService) -> Self {
        Self {
            user_repository: Arc::new(user_repository),
            jwt_service: Arc::new(jwt_service),
            argon2: Argon2::default(),
        }
    }

    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    pub async fn register_user(&self, request: CreateUserRequest) -> Result<UserResponse, AppError> {
        self.validate_registration_request(&request)?;

        if let Some(_) = self.user_repository.get_user_by_username(&request.username).await? {
            return Err(AppError::BadRequest("Username already exists".to_string()));
        }

        if let Some(_) = self.user_repository.get_user_by_email(&request.email).await? {
            return Err(AppError::BadRequest("Email already exists".to_string()));
        }

        let password_hash = self.hash_password(&request.password)?;

        let user = self.user_repository.create_user(&request, &password_hash).await?;

        Ok(UserResponse::from(user))
    }

    pub async fn login(&self, request: LoginRequest) -> Result<LoginResponse, AppError> {
        self.validate_login_request(&request)?;

        let user = self
            .user_repository
            .get_user_by_username(&request.username)
            .await?
            .ok_or_else(|| AppError::Authentication("Invalid credentials".to_string()))?;

        if !user.is_active {
            return Err(AppError::Authentication("Account is disabled".to_string()));
        }

        if !self.verify_password(&request.password, &user.password_hash)? {
            return Err(AppError::Authentication("Invalid credentials".to_string()));
        }

        self.user_repository.update_last_login(user.id).await?;

        let access_token = self.jwt_service.generate_access_token(&user)?;
        let refresh_token = self.jwt_service.generate_refresh_token(&user)?;

        Ok(LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_service.get_access_token_expiry_seconds(),
            user: UserResponse::from(user),
        })
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> Result<RefreshTokenResponse, AppError> {
        let claims = self.jwt_service.validate_refresh_token(refresh_token)?;

        let user_id: i64 = claims.sub.parse()
            .map_err(|_| AppError::Authentication("Invalid user ID in token".to_string()))?;

        let user = self
            .user_repository
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::Authentication("User not found".to_string()))?;

        if !user.is_active {
            return Err(AppError::Authentication("Account is disabled".to_string()));
        }

        let access_token = self.jwt_service.generate_access_token(&user)?;

        Ok(RefreshTokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_service.get_access_token_expiry_seconds(),
        })
    }

    pub async fn validate_token(&self, token: &str) -> Result<JwtClaims, AppError> {
        let claims = self.jwt_service.validate_access_token(token)?;

        let user_id: i64 = claims.sub.parse()
            .map_err(|_| AppError::Authentication("Invalid user ID in token".to_string()))?;

        let user = self
            .user_repository
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::Authentication("User not found".to_string()))?;

        if !user.is_active {
            return Err(AppError::Authentication("Account is disabled".to_string()));
        }

        Ok(claims)
    }

    pub async fn get_user_by_id(&self, user_id: i64) -> Result<Option<UserResponse>, AppError> {
        let user = self.user_repository.get_user_by_id(user_id).await?;
        Ok(user.map(UserResponse::from))
    }

    fn hash_password(&self, password: &str) -> Result<String, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Authentication(format!("Failed to hash password: {}", e)))?;

        Ok(password_hash.to_string())
    }

    fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::Authentication(format!("Invalid password hash: {}", e)))?;

        Ok(self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    fn validate_registration_request(&self, request: &CreateUserRequest) -> Result<(), AppError> {
        if request.username.trim().is_empty() {
            return Err(AppError::BadRequest("Username cannot be empty".to_string()));
        }

        if request.username.len() < 3 {
            return Err(AppError::BadRequest(
                "Username must be at least 3 characters long".to_string(),
            ));
        }

        if request.username.len() > 50 {
            return Err(AppError::BadRequest(
                "Username cannot be longer than 50 characters".to_string(),
            ));
        }

        if !request.username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err(AppError::BadRequest(
                "Username can only contain alphanumeric characters, underscores, and hyphens".to_string(),
            ));
        }

        if request.email.trim().is_empty() {
            return Err(AppError::BadRequest("Email cannot be empty".to_string()));
        }

        if !self.is_valid_email(&request.email) {
            return Err(AppError::BadRequest("Invalid email format".to_string()));
        }

        if request.password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        if request.password.len() > 128 {
            return Err(AppError::BadRequest(
                "Password cannot be longer than 128 characters".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_login_request(&self, request: &LoginRequest) -> Result<(), AppError> {
        if request.username.trim().is_empty() {
            return Err(AppError::BadRequest("Username cannot be empty".to_string()));
        }

        if request.password.trim().is_empty() {
            return Err(AppError::BadRequest("Password cannot be empty".to_string()));
        }

        Ok(())
    }

    fn is_valid_email(&self, email: &str) -> bool {
        let email_regex = regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
        email_regex.is_match(email)
    }
}
use crate::auth::models::{JwtClaims, User, UserRole};
use crate::error::AppError;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::env;

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_expiry: Duration,
    refresh_token_expiry: Duration,
}

impl JwtService {
    pub fn new() -> Result<Self, AppError> {
        let secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "1a9e1a1d8f3e9613a555adea1881bbd1".to_string()); // I'm keeping my jwt-secret here. You would want to keep this in a .env file when dealing with serious environments for security reasons.
        
        if secret.len() < 32 {
            return Err(AppError::Authentication(
                "JWT secret must be at least 32 characters long".to_string(),
            ));
        }

        let encoding_key = EncodingKey::from_secret(secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(secret.as_bytes());

        Ok(Self {
            encoding_key,
            decoding_key,
            access_token_expiry: Duration::hours(1),
            refresh_token_expiry: Duration::days(7),
        })
    }

    pub fn generate_access_token(&self, user: &User) -> Result<String, AppError> {
        let now = Utc::now();
        let exp = (now + self.access_token_expiry).timestamp() as usize;
        let iat = now.timestamp() as usize;

        let claims = JwtClaims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            role: user.role.clone(),
            exp,
            iat,
            token_type: "access".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::Authentication(format!("Failed to generate access token: {}", e)))
    }

    pub fn generate_refresh_token(&self, user: &User) -> Result<String, AppError> {
        let now = Utc::now();
        let exp = (now + self.refresh_token_expiry).timestamp() as usize;
        let iat = now.timestamp() as usize;

        let claims = JwtClaims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            role: user.role.clone(),
            exp,
            iat,
            token_type: "refresh".to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::Authentication(format!("Failed to generate refresh token: {}", e)))
    }

    pub fn validate_token(&self, token: &str) -> Result<JwtClaims, AppError> {
        let validation = Validation::new(Algorithm::HS256);
        
        decode::<JwtClaims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    AppError::Authentication("Token has expired".to_string())
                }
                jsonwebtoken::errors::ErrorKind::InvalidToken => {
                    AppError::Authentication("Invalid token".to_string())
                }
                _ => AppError::Authentication(format!("Token validation failed: {}", e)),
            })
    }

    pub fn validate_access_token(&self, token: &str) -> Result<JwtClaims, AppError> {
        let claims = self.validate_token(token)?;
        
        if claims.token_type != "access" {
            return Err(AppError::Authentication(
                "Invalid token type for access token".to_string(),
            ));
        }

        Ok(claims)
    }

    pub fn validate_refresh_token(&self, token: &str) -> Result<JwtClaims, AppError> {
        let claims = self.validate_token(token)?;
        
        if claims.token_type != "refresh" {
            return Err(AppError::Authentication(
                "Invalid token type for refresh token".to_string(),
            ));
        }

        Ok(claims)
    }

    pub fn get_access_token_expiry_seconds(&self) -> i64 {
        self.access_token_expiry.num_seconds()
    }

    pub fn extract_user_role(&self, token: &str) -> Result<UserRole, AppError> {
        let claims = self.validate_access_token(token)?;
        claims.role.parse()
            .map_err(|e| AppError::Authentication(format!("Invalid role in token: {}", e)))
    }

    pub fn extract_user_id(&self, token: &str) -> Result<i64, AppError> {
        let claims = self.validate_access_token(token)?;
        claims.sub.parse()
            .map_err(|_| AppError::Authentication("Invalid user ID in token".to_string()))
    }
}

impl Default for JwtService {
    fn default() -> Self {
        Self::new().expect("Failed to create JWT service")
    }
}
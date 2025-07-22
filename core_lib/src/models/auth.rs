//! Authentication-related models with validation

use crate::validation::{ValidationResult, ValidationContext, ContextValidatable, Validatable};
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 3, max = 30, message = "Username must be between 3 and 30 characters"))]
    pub username: String,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[validate(length(min = 8, max = 128, message = "Password must be between 8 and 128 characters"))]
    pub password: String,

    #[validate(must_match(other = "password", message = "Passwords do not match"))]
    pub password_confirmation: String,

    #[validate(length(max = 100, message = "First name is too long"))]
    pub first_name: Option<String>,

    #[validate(length(max = 100, message = "Last name is too long"))]
    pub last_name: Option<String>,
}

impl ContextValidatable for RegisterRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Err(err) = crate::validation::rules::validate_username(&self.username) {
            result.add_error("username", &err.to_string());
        }

        if let Err(err) = crate::validation::rules::validate_email(&self.email) {
            result.add_error("email", &err.to_string());
        }

        if let Err(err) = crate::validation::rules::validate_password(&self.password) {
            result.add_error("password", &err.to_string());
        }

        if let Some(first_name) = &self.first_name {
            if let Err(_) = crate::validation::rules::validate_no_xss(first_name) {
                result.add_error("first_name", "First name contains invalid characters");
            }
        }

        if let Some(last_name) = &self.last_name {
            if let Err(_) = crate::validation::rules::validate_no_xss(last_name) {
                result.add_error("last_name", "Last name contains invalid characters");
            }
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "Username or email is required"))]
    pub username_or_email: String,

    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

impl ContextValidatable for LoginRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Err(_) = crate::validation::rules::validate_no_sql_injection(&self.username_or_email) {
            result.add_error("username_or_email", "Username/email contains invalid characters");
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, message = "Current password is required"))]
    pub current_password: String,

    #[validate(length(min = 8, max = 128, message = "New password must be between 8 and 128 characters"))]
    pub new_password: String,

    #[validate(must_match(other = "new_password", message = "Passwords do not match"))]
    pub new_password_confirmation: String,
}

impl ContextValidatable for ChangePasswordRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Err(err) = crate::validation::rules::validate_password(&self.new_password) {
            result.add_error("new_password", &err.to_string());
        }

        if self.current_password == self.new_password {
            result.add_error("new_password", "New password must be different from current password");
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateProfileRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    #[validate(length(max = 100, message = "First name is too long"))]
    pub first_name: Option<String>,

    #[validate(length(max = 100, message = "Last name is too long"))]
    pub last_name: Option<String>,
}

impl ContextValidatable for UpdateProfileRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(email) = &self.email {
            if let Err(err) = crate::validation::rules::validate_email(email) {
                result.add_error("email", &err.to_string());
            }
        }

        if let Some(first_name) = &self.first_name {
            if let Err(_) = crate::validation::rules::validate_no_xss(first_name) {
                result.add_error("first_name", "First name contains invalid characters");
            }
        }

        if let Some(last_name) = &self.last_name {
            if let Err(_) = crate::validation::rules::validate_no_xss(last_name) {
                result.add_error("last_name", "Last name contains invalid characters");
            }
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RefreshTokenRequest {
    #[validate(length(min = 1, message = "Refresh token is required"))]
    pub refresh_token: String,
}

impl ContextValidatable for RefreshTokenRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        let parts: Vec<&str> = self.refresh_token.split('.').collect();
        if parts.len() != 3 {
            result.add_error("refresh_token", "Invalid token format");
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PasswordResetRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

impl ContextValidatable for PasswordResetRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Err(err) = crate::validation::rules::validate_email(&self.email) {
            result.add_error("email", &err.to_string());
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PasswordResetConfirmRequest {
    #[validate(length(min = 1, message = "Reset token is required"))]
    pub token: String,

    #[validate(length(min = 8, max = 128, message = "Password must be between 8 and 128 characters"))]
    pub new_password: String,

    #[validate(must_match(other = "new_password", message = "Passwords do not match"))]
    pub new_password_confirmation: String,
}

impl ContextValidatable for PasswordResetConfirmRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Err(err) = crate::validation::rules::validate_password(&self.new_password) {
            result.add_error("new_password", &err.to_string());
        }
        
        result
    }
}
//! File-related models with validation

use crate::validation::{ValidationResult, ValidationContext, ContextValidatable, Validatable};
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FileUploadRequest {
    #[validate(length(min = 1, max = 255, message = "Filename must be between 1 and 255 characters"))]
    pub filename: String,

    #[validate(length(min = 1, max = 100, message = "Content type is required and must not exceed 100 characters"))]
    pub content_type: String,

    #[validate(range(min = 1, max = 104857600, message = "File size must be between 1 byte and 100MB"))]
    pub size: u64,

    #[validate(length(max = 500, message = "Description is too long"))]
    pub description: Option<String>,

    #[validate(length(max = 20, message = "Too many tags"))]
    pub tags: Option<Vec<String>>,
}

impl ContextValidatable for FileUploadRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Err(err) = crate::validation::rules::validate_file_extension(&self.filename) {
            result.add_error("filename", &err.to_string());
        }

        if let Err(err) = crate::validation::SecurityValidator::validate_path_traversal(&self.filename) {
            result.add_error("filename", &err.to_string());
        }

        if let Err(_) = crate::validation::rules::validate_no_xss(&self.filename) {
            result.add_error("filename", "Filename contains invalid characters");
        }

        if self.filename.contains('\0') {
            result.add_error("filename", "Filename contains null bytes");
        }

        if !self.is_allowed_content_type() {
            result.add_error("content_type", "Content type not allowed");
        }

        if let Some(desc) = &self.description {
            if let Err(_) = crate::validation::rules::validate_no_xss(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
        }

        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }
                if tag.len() > 50 {
                    result.add_error(&format!("tags[{}]", i), "Tag is too long");
                }
                if let Err(_) = crate::validation::rules::validate_no_xss(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
                if let Err(_) = crate::validation::rules::validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }
        
        result
    }
}

impl FileUploadRequest {
    fn is_allowed_content_type(&self) -> bool {
        let allowed_types = [
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/webp",
            "text/plain",
            "text/csv",
            "application/pdf",
            "application/json",
            "application/xml",
            "application/zip",
            "application/msword",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.ms-excel",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ];
        
        allowed_types.iter().any(|&allowed| self.content_type.starts_with(allowed))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponse {
    pub id: Uuid,
    pub filename: String,
    pub original_filename: String,
    pub content_type: String,
    pub size: u64,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: Option<u64>,
    pub download_url: String,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FileListQuery {
    #[validate(range(min = 1, max = 1000, message = "Page size must be between 1 and 1000"))]
    pub page_size: Option<u32>,

    #[validate(range(min = 1, message = "Page number must be at least 1"))]
    pub page: Option<u32>,

    #[validate(length(max = 100, message = "Content type filter is too long"))]
    pub content_type: Option<String>,

    #[validate(length(max = 20, message = "Too many tags in filter"))]
    pub tags: Option<Vec<String>>,

    #[validate(length(max = 500, message = "Search query is too long"))]
    pub search: Option<String>,
}

impl ContextValidatable for FileListQuery {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }
                if let Err(_) = crate::validation::rules::validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }

        if let Some(search) = &self.search {
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(search) {
                result.add_error("search", "Search query contains invalid characters");
            }
        }

        if let Some(ct) = &self.content_type {
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(ct) {
                result.add_error("content_type", "Content type filter contains invalid characters");
            }
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FileUpdateRequest {
    #[validate(length(max = 500, message = "Description is too long"))]
    pub description: Option<String>,

    #[validate(length(max = 20, message = "Too many tags"))]
    pub tags: Option<Vec<String>>,

    pub is_public: Option<bool>,
}

impl ContextValidatable for FileUpdateRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(desc) = &self.description {
            if let Err(_) = crate::validation::rules::validate_no_xss(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
        }

        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }
                if tag.len() > 50 {
                    result.add_error(&format!("tags[{}]", i), "Tag is too long");
                }
                if let Err(_) = crate::validation::rules::validate_no_xss(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
                if let Err(_) = crate::validation::rules::validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct FileShareRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,

    #[validate(length(min = 3, max = 30, message = "Username must be between 3 and 30 characters"))]
    pub username: Option<String>,

    #[validate(length(max = 20, message = "Permission string is too long"))]
    pub permission: String,

    pub expires_at: Option<DateTime<Utc>>,
}

impl ContextValidatable for FileShareRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if self.email.is_none() && self.username.is_none() {
            result.add_error("recipient", "Either email or username must be provided");
        }

        let allowed_permissions = ["read", "write", "admin"];
        if !allowed_permissions.contains(&self.permission.as_str()) {
            result.add_error("permission", "Permission must be one of: read, write, admin");
        }

        if let Some(expires_at) = self.expires_at {
            if expires_at <= Utc::now() {
                result.add_error("expires_at", "Expiration date must be in the future");
            }
        }

        if let Some(email) = &self.email {
            if let Err(err) = crate::validation::rules::validate_email(email) {
                result.add_error("email", &err.to_string());
            }
        }

        if let Some(username) = &self.username {
            if let Err(err) = crate::validation::rules::validate_username(username) {
                result.add_error("username", &err.to_string());
            }
        }
        
        result
    }
}
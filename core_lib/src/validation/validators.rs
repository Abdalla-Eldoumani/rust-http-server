//! Specific validators for data models

use super::{ValidationResult, ValidationContext, ContextValidatable, Validatable, rules::*};
use crate::models::request::{JsonPayload, FormPayload};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ItemValidator {
    #[validate(length(min = 1, max = 255, message = "Name must be between 1 and 255 characters"))]
    #[validate(custom(function = "validate_no_sql_injection", message = "Name contains invalid characters"))]
    #[validate(custom(function = "validate_no_xss", message = "Name contains invalid characters"))]
    pub name: String,

    #[validate(length(max = 2000, message = "Description must not exceed 2000 characters"))]
    #[validate(custom(function = "validate_no_sql_injection", message = "Description contains invalid characters"))]
    #[validate(custom(function = "validate_no_xss", message = "Description contains invalid characters"))]
    pub description: Option<String>,

    #[validate(length(max = 50, message = "Too many tags"))]
    pub tags: Option<Vec<String>>,

    pub metadata: Option<serde_json::Value>,
}

impl ItemValidator {
    pub fn new(name: String, description: Option<String>, tags: Option<Vec<String>>, metadata: Option<serde_json::Value>) -> Self {
        Self { name, description, tags, metadata }
    }

    pub fn validate_tags(&self) -> ValidationResult {
        let mut result = ValidationResult::success();

        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }
                if tag.len() > 50 {
                    result.add_error(&format!("tags[{}]", i), "Tag must not exceed 50 characters");
                }
                if let Err(_) = validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
                if let Err(_) = validate_no_xss(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }

        result
    }

    pub fn validate_metadata(&self) -> ValidationResult {
        let mut result = ValidationResult::success();

        if let Some(metadata) = &self.metadata {
            match serde_json::to_string(metadata) {
                Ok(serialized) => {
                    if serialized.len() > 10000 {
                        result.add_error("metadata", "Metadata is too large (max 10KB)");
                    }
                }
                Err(_) => {
                    result.add_error("metadata", "Invalid metadata format");
                }
            }

            let metadata_str = metadata.to_string();
            if let Err(_) = validate_no_sql_injection(&metadata_str) {
                result.add_error("metadata", "Metadata contains potentially dangerous content");
            }
            if let Err(_) = validate_no_xss(&metadata_str) {
                result.add_error("metadata", "Metadata contains potentially dangerous content");
            }
        }

        result
    }
}

impl ContextValidatable for ItemValidator {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        let tags_result = self.validate_tags();
        result.merge(tags_result);
        
        let metadata_result = self.validate_metadata();
        result.merge(metadata_result);
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UserRegistrationValidator {
    #[validate(custom(function = "validate_username", message = "Invalid username format"))]
    pub username: String,

    #[validate(custom(function = "validate_email", message = "Invalid email format"))]
    pub email: String,

    #[validate(custom(function = "validate_password", message = "Password does not meet security requirements"))]
    pub password: String,

    #[validate(must_match(other = "password", message = "Passwords do not match"))]
    pub password_confirmation: String,

    #[validate(length(max = 100, message = "First name is too long"))]
    #[validate(custom(function = "validate_no_xss", message = "First name contains invalid characters"))]
    pub first_name: Option<String>,

    #[validate(length(max = 100, message = "Last name is too long"))]
    #[validate(custom(function = "validate_no_xss", message = "Last name contains invalid characters"))]
    pub last_name: Option<String>,
}

impl UserRegistrationValidator {
    pub fn new(
        username: String,
        email: String,
        password: String,
        password_confirmation: String,
        first_name: Option<String>,
        last_name: Option<String>,
    ) -> Self {
        Self {
            username,
            email,
            password,
            password_confirmation,
            first_name,
            last_name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UserLoginValidator {
    #[validate(length(min = 1, message = "Username or email is required"))]
    pub username_or_email: String,

    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

impl UserLoginValidator {
    pub fn new(username_or_email: String, password: String) -> Self {
        Self { username_or_email, password }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadValidator {
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub content: Vec<u8>,
}

impl FileUploadValidator {
    pub fn new(filename: String, content_type: String, size: u64, content: Vec<u8>) -> Self {
        Self { filename, content_type, size, content }
    }

    pub fn validate_comprehensive(&self) -> ValidationResult {
        let mut result = ValidationResult::success();

        if self.filename.is_empty() {
            result.add_error("filename", "Filename cannot be empty");
        } else {
            if let Err(err) = validate_file_extension(&self.filename) {
                result.add_error("filename", &err.to_string());
            }
            if let Err(_) = validate_no_xss(&self.filename) {
                result.add_error("filename", "Filename contains invalid characters");
            }
        }

        if let Err(err) = validate_file_size(self.size, 10 * 1024 * 1024) {
            result.add_error("size", &err.to_string());
        }

        if self.content_type.is_empty() {
            result.add_error("content_type", "Content type is required");
        } else if !self.is_allowed_content_type() {
            result.add_error("content_type", "Content type not allowed");
        }

        if let Err(err) = self.validate_file_content() {
            result.add_error("content", &err.to_string());
        }

        result
    }

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
        
        allowed_types.contains(&self.content_type.as_str())
    }

    fn validate_file_content(&self) -> Result<(), ValidationError> {
        if self.content.contains(&0) && !self.is_binary_allowed() {
            return Err(ValidationError::new("File contains null bytes"));
        }

        if self.content_type.starts_with("image/") {
            self.validate_image_content()?;
        }

        Ok(())
    }

    fn is_binary_allowed(&self) -> bool {
        self.content_type.starts_with("image/") ||
        self.content_type == "application/pdf" ||
        self.content_type == "application/zip"
    }

    fn validate_image_content(&self) -> Result<(), ValidationError> {
        if self.content.len() < 4 {
            return Err(ValidationError::new("Invalid image file"));
        }

        let header = &self.content[0..4];
        
        let is_valid_image = match self.content_type.as_str() {
            "image/jpeg" => header.starts_with(&[0xFF, 0xD8, 0xFF]),
            "image/png" => header.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
            "image/gif" => header.starts_with(b"GIF8"),
            _ => true,
        };

        if !is_valid_image {
            return Err(ValidationError::new("File content does not match declared image type"));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SearchQueryValidator {
    #[validate(length(max = 500, message = "Search query is too long"))]
    #[validate(custom(function = "validate_no_sql_injection", message = "Search query contains invalid characters"))]
    pub query: Option<String>,

    #[validate(length(max = 20, message = "Too many tags in search"))]
    pub tags: Option<Vec<String>>,

    #[validate(range(min = 1, max = 1000, message = "Page size must be between 1 and 1000"))]
    pub page_size: Option<u32>,

    #[validate(range(min = 1, message = "Page number must be at least 1"))]
    pub page: Option<u32>,
}

impl SearchQueryValidator {
    pub fn validate_tags(&self) -> ValidationResult {
        let mut result = ValidationResult::success();

        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }
                if tag.len() > 50 {
                    result.add_error(&format!("tags[{}]", i), "Tag is too long");
                }
                if let Err(_) = validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }

        result
    }
}

impl ContextValidatable for SearchQueryValidator {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        let tags_result = self.validate_tags();
        result.merge(tags_result);
        
        result
    }
}

impl ContextValidatable for JsonPayload {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = ValidationResult::success();

        if self.message.is_empty() {
            result.add_error("message", "Message cannot be empty");
        } else if self.message.len() > 1000 {
            result.add_error("message", "Message is too long");
        } else {
            if let Err(_) = validate_no_sql_injection(&self.message) {
                result.add_error("message", "Message contains invalid characters");
            }
            if let Err(_) = validate_no_xss(&self.message) {
                result.add_error("message", "Message contains invalid characters");
            }
        }

        if let Some(timestamp) = self.timestamp {
            let now = chrono::Utc::now().timestamp();
            if timestamp < 0 || timestamp > now + 3600 {
                result.add_error("timestamp", "Invalid timestamp");
            }
        }

        if let Some(data) = &self.data {
            let data_str = data.to_string();
            if data_str.len() > 5000 {
                result.add_error("data", "Data payload is too large");
            }
            if let Err(_) = validate_no_sql_injection(&data_str) {
                result.add_error("data", "Data contains invalid characters");
            }
            if let Err(_) = validate_no_xss(&data_str) {
                result.add_error("data", "Data contains invalid characters");
            }
        }

        result
    }
}

impl ContextValidatable for FormPayload {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = ValidationResult::success();

        if self.name.is_empty() {
            result.add_error("name", "Name cannot be empty");
        } else if self.name.len() > 100 {
            result.add_error("name", "Name is too long");
        } else {
            if let Err(_) = validate_no_xss(&self.name) {
                result.add_error("name", "Name contains invalid characters");
            }
        }

        if let Err(err) = validate_email(&self.email) {
            result.add_error("email", &err.to_string());
        }

        if let Some(message) = &self.message {
            if message.len() > 2000 {
                result.add_error("message", "Message is too long");
            } else {
                if let Err(_) = validate_no_sql_injection(message) {
                    result.add_error("message", "Message contains invalid characters");
                }
                if let Err(_) = validate_no_xss(message) {
                    result.add_error("message", "Message contains invalid characters");
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_validator() {
        let validator = ItemValidator::new(
            "Test Item".to_string(),
            Some("A test description".to_string()),
            Some(vec!["tag1".to_string(), "tag2".to_string()]),
            None,
        );
        
        let result = validator.validate_with_context(&ValidationContext::default());
        assert!(result.is_valid);
    }

    #[test]
    fn test_item_validator_with_xss() {
        let validator = ItemValidator::new(
            "<script>alert('xss')</script>".to_string(),
            None,
            None,
            None,
        );
        
        let result = validator.validate_with_context(&ValidationContext::default());
        assert!(!result.is_valid);
        assert!(result.errors.contains_key("name"));
    }

    #[test]
    fn test_user_registration_validator() {
        let validator = UserRegistrationValidator::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "StrongPass123!".to_string(),
            "StrongPass123!".to_string(),
            Some("John".to_string()),
            Some("Doe".to_string()),
        );
        
        let result = validator.validate_comprehensive();
        assert!(result.is_valid);
    }

    #[test]
    fn test_file_upload_validator() {
        let validator = FileUploadValidator::new(
            "test.jpg".to_string(),
            "image/jpeg".to_string(),
            1024,
            vec![0xFF, 0xD8, 0xFF, 0xE0],
        );
        
        let result = validator.validate_comprehensive();
        assert!(result.is_valid);
    }

    #[test]
    fn test_file_upload_validator_dangerous_extension() {
        let validator = FileUploadValidator::new(
            "malware.exe".to_string(),
            "application/octet-stream".to_string(),
            1024,
            vec![0x4D, 0x5A],
        );
        
        let result = validator.validate_comprehensive();
        assert!(!result.is_valid);
        assert!(result.errors.contains_key("filename"));
    }
}
//! Item-related models with validation

use crate::validation::{ValidationResult, ValidationContext, ContextValidatable, Validatable};
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateItemRequest {
    #[validate(length(min = 1, max = 255, message = "Name must be between 1 and 255 characters"))]
    pub name: String,

    #[validate(length(max = 2000, message = "Description must not exceed 2000 characters"))]
    pub description: Option<String>,

    #[validate(length(max = 50, message = "Too many tags"))]
    pub tags: Option<Vec<String>>,

    pub metadata: Option<serde_json::Value>,
}

impl ContextValidatable for CreateItemRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }

                if tag.len() > 50 {
                    result.add_error(&format!("tags[{}]", i), "Tag must not exceed 50 characters");
                }

                if let Err(_) = crate::validation::rules::validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }

                if let Err(_) = crate::validation::rules::validate_no_xss(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }

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
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(&metadata_str) {
                result.add_error("metadata", "Metadata contains potentially dangerous content");
            }
            if let Err(_) = crate::validation::rules::validate_no_xss(&metadata_str) {
                result.add_error("metadata", "Metadata contains potentially dangerous content");
            }
        }

        if let Err(_) = crate::validation::rules::validate_no_sql_injection(&self.name) {
            result.add_error("name", "Name contains invalid characters");
        }
        if let Err(_) = crate::validation::rules::validate_no_xss(&self.name) {
            result.add_error("name", "Name contains invalid characters");
        }

        if let Some(desc) = &self.description {
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
            if let Err(_) = crate::validation::rules::validate_no_xss(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateItemRequest {
    #[validate(length(min = 1, max = 255, message = "Name must be between 1 and 255 characters"))]
    pub name: Option<String>,

    #[validate(length(max = 2000, message = "Description must not exceed 2000 characters"))]
    pub description: Option<String>,

    #[validate(length(max = 50, message = "Too many tags"))]
    pub tags: Option<Vec<String>>,

    pub metadata: Option<serde_json::Value>,
}

impl ContextValidatable for UpdateItemRequest {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(name) = &self.name {
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(name) {
                result.add_error("name", "Name contains invalid characters");
            }
            if let Err(_) = crate::validation::rules::validate_no_xss(name) {
                result.add_error("name", "Name contains invalid characters");
            }
        }

        if let Some(desc) = &self.description {
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
            if let Err(_) = crate::validation::rules::validate_no_xss(desc) {
                result.add_error("description", "Description contains invalid characters");
            }
        }

        if let Some(tags) = &self.tags {
            for (i, tag) in tags.iter().enumerate() {
                if tag.is_empty() {
                    result.add_error(&format!("tags[{}]", i), "Tag cannot be empty");
                }
                if tag.len() > 50 {
                    result.add_error(&format!("tags[{}]", i), "Tag must not exceed 50 characters");
                }
                if let Err(_) = crate::validation::rules::validate_no_sql_injection(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
                if let Err(_) = crate::validation::rules::validate_no_xss(tag) {
                    result.add_error(&format!("tags[{}]", i), "Tag contains invalid characters");
                }
            }
        }

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
            if let Err(_) = crate::validation::rules::validate_no_sql_injection(&metadata_str) {
                result.add_error("metadata", "Metadata contains potentially dangerous content");
            }
            if let Err(_) = crate::validation::rules::validate_no_xss(&metadata_str) {
                result.add_error("metadata", "Metadata contains potentially dangerous content");
            }
        }
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemResponse {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<u64>,
    pub file_attachments: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ItemListQuery {
    #[validate(range(min = 1, max = 1000, message = "Page size must be between 1 and 1000"))]
    pub page_size: Option<u32>,

    #[validate(range(min = 1, message = "Page number must be at least 1"))]
    pub page: Option<u32>,

    #[validate(length(max = 100, message = "Sort field name is too long"))]
    pub sort_by: Option<String>,

    pub sort_order: Option<String>,

    #[validate(length(max = 20, message = "Too many tags in filter"))]
    pub tags: Option<Vec<String>>,

    #[validate(length(max = 500, message = "Search query is too long"))]
    pub search: Option<String>,
}

impl ContextValidatable for ItemListQuery {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(order) = &self.sort_order {
            if !["asc", "desc"].contains(&order.to_lowercase().as_str()) {
                result.add_error("sort_order", "Sort order must be 'asc' or 'desc'");
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
        
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ItemExportQuery {
    #[validate(length(max = 10, message = "Format string is too long"))]
    pub format: Option<String>,

    #[validate(length(max = 20, message = "Too many tags in filter"))]
    pub tags: Option<Vec<String>>,

    #[validate(length(max = 500, message = "Search query is too long"))]
    pub search: Option<String>,
}

impl ContextValidatable for ItemExportQuery {
    fn validate_with_context(&self, _context: &ValidationContext) -> ValidationResult {
        let mut result = self.validate_comprehensive();
        
        if let Some(format) = &self.format {
            let allowed_formats = ["json", "csv", "yaml"];
            if !allowed_formats.contains(&format.to_lowercase().as_str()) {
                result.add_error("format", "Format must be one of: json, csv, yaml");
            }
        }

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
        
        result
    }
}
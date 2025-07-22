//! Comprehensive validation framework for input validation and security

pub mod rules;
pub mod validators;
pub mod middleware;
pub mod macros;
pub mod security;

pub use rules::*;
pub use validators::*;
pub use middleware::*;
pub use security::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::{Validate, ValidationError, ValidationErrors};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: HashMap<String, Vec<String>>,
    pub field_errors: HashMap<String, FieldValidationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValidationError {
    pub field: String,
    pub value: Option<String>,
    pub errors: Vec<String>,
    pub error_codes: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: HashMap::new(),
            field_errors: HashMap::new(),
        }
    }

    pub fn from_validation_errors(errors: ValidationErrors) -> Self {
        let mut result = Self {
            is_valid: false,
            errors: HashMap::new(),
            field_errors: HashMap::new(),
        };

        for (field, field_errors) in errors.field_errors() {
            let mut error_messages = Vec::new();
            let mut error_codes = Vec::new();

            for error in field_errors {
                if let Some(message) = &error.message {
                    error_messages.push(message.to_string());
                } else {
                    error_messages.push(format!("Validation failed for field '{}'", field));
                }
                error_codes.push(error.code.to_string());
            }

            result.errors.insert(field.to_string(), error_messages.clone());
            result.field_errors.insert(field.to_string(), FieldValidationError {
                field: field.to_string(),
                value: None,
                errors: error_messages,
                error_codes,
            });
        }

        result
    }

    pub fn add_error(&mut self, field: &str, message: &str) {
        self.is_valid = false;
        self.errors.entry(field.to_string())
            .or_insert_with(Vec::new)
            .push(message.to_string());
        
        self.field_errors.insert(field.to_string(), FieldValidationError {
            field: field.to_string(),
            value: None,
            errors: vec![message.to_string()],
            error_codes: vec!["custom".to_string()],
        });
    }

    pub fn merge(&mut self, other: ValidationResult) {
        if !other.is_valid {
            self.is_valid = false;
        }

        for (field, errors) in other.errors {
            self.errors.entry(field.clone())
                .or_insert_with(Vec::new)
                .extend(errors);
        }

        for (field, field_error) in other.field_errors {
            self.field_errors.insert(field, field_error);
        }
    }
}

pub trait Validatable {
    fn validate_comprehensive(&self) -> ValidationResult;
}

impl<T> Validatable for T
where T: Validate, {
    fn validate_comprehensive(&self) -> ValidationResult {
        match self.validate() {
            Ok(_) => ValidationResult::success(),
            Err(errors) => ValidationResult::from_validation_errors(errors),
        }
    }
}

pub type CustomValidator<T> = fn(&T) -> Result<(), ValidationError>;

#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub user_id: Option<u64>,
    pub user_role: Option<String>,
    pub request_ip: Option<String>,
    pub additional_data: HashMap<String, serde_json::Value>,
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self {
            user_id: None,
            user_role: None,
            request_ip: None,
            additional_data: HashMap::new(),
        }
    }
}

pub trait ContextValidatable {
    fn validate_with_context(&self, context: &ValidationContext) -> ValidationResult;
}
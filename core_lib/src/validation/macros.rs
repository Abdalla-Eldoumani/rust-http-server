//! Validation macros and utilities for easier validation

#[macro_export]
macro_rules! validate_fields {
    ($($field:expr => $validation:expr),* $(,)?) => {{
        let mut result = $crate::validation::ValidationResult::success();
        $(
            if let Err(err) = $validation {
                result.add_error($field, &err.to_string());
            }
        )*
        result
    }};
}

#[macro_export]
macro_rules! validate_field {
    ($field_name:expr, $field_value:expr, $($validator:expr),* $(,)?) => {{
        let mut field_result = $crate::validation::ValidationResult::success();
        $(
            if let Err(err) = $validator($field_value) {
                field_result.add_error($field_name, &err.to_string());
            }
        )*
        field_result
    }};
}

#[macro_export]
macro_rules! custom_validator {
    ($name:ident, $param:ident: $param_type:ty, $validation:expr) => {
        pub fn $name($param: $param_type) -> Result<(), validator::ValidationError> {
            if $validation {
                Ok(())
            } else {
                Err(validator::ValidationError::new(stringify!($name)))
            }
        }
    };
    ($name:ident, $param:ident: $param_type:ty, $validation:expr, $message:expr) => {
        pub fn $name($param: $param_type) -> Result<(), validator::ValidationError> {
            if $validation {
                Ok(())
            } else {
                Err(validator::ValidationError::new($message))
            }
        }
    };
}

#[macro_export]
macro_rules! validate_or_return {
    ($validation:expr) => {
        match $validation {
            result if result.is_valid => {},
            result => return Ok(result.into()),
        }
    };
}

#[macro_export]
macro_rules! combine_validations {
    ($($validation:expr),* $(,)?) => {{
        let mut combined = $crate::validation::ValidationResult::success();
        $(
            combined.merge($validation);
        )*
        combined
    }};
}

#[macro_export]
macro_rules! validation_chain {
    ($value:expr => $($validator:expr),* $(,)?) => {{
        let mut result = $crate::validation::ValidationResult::success();
        $(
            if let Err(err) = $validator($value) {
                result.add_error("value", &err.to_string());
                break;
            }
        )*
        result
    }};
}

#[macro_export]
macro_rules! validate_optional {
    ($field_name:expr, $option:expr, $validator:expr) => {
        if let Some(value) = $option {
            if let Err(err) = $validator(value) {
                return Err(err);
            }
        }
    };
}

#[macro_export]
macro_rules! validate_if {
    ($condition:expr, $field_name:expr, $value:expr, $validator:expr) => {
        if $condition {
            if let Err(err) = $validator($value) {
                return Err(validator::ValidationError::new(&format!("{}: {}", $field_name, err)));
            }
        }
    };
}

#[macro_export]
macro_rules! validate_collection {
    ($collection:expr, $validator:expr) => {{
        let mut result = $crate::validation::ValidationResult::success();
        for (index, item) in $collection.iter().enumerate() {
            if let Err(err) = $validator(item) {
                result.add_error(&format!("[{}]", index), &err.to_string());
            }
        }
        result
    }};
    ($collection:expr, $field_prefix:expr, $validator:expr) => {{
        let mut result = $crate::validation::ValidationResult::success();
        for (index, item) in $collection.iter().enumerate() {
            if let Err(err) = $validator(item) {
                result.add_error(&format!("{}[{}]", $field_prefix, index), &err.to_string());
            }
        }
        result
    }};
}

#[macro_export]
macro_rules! validation_builder {
    ($struct_name:ident {
        $($field:ident: $field_type:ty $(= $default:expr)?),* $(,)?
    }) => {
        #[derive(Debug, Clone)]
        pub struct $struct_name {
            $(pub $field: $field_type,)*
        }

        impl $struct_name {
            pub fn new() -> Self {
                Self {
                    $($field: validation_builder!(@default $field_type $(, $default)?),)*
                }
            }

            $(
                pub fn $field(mut self, value: $field_type) -> Self {
                    self.$field = value;
                    self
                }
            )*

            pub fn validate(&self) -> $crate::validation::ValidationResult {
                let result = $crate::validation::ValidationResult::success();
                result
            }
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }
    };

    (@default $field_type:ty) => {
        Default::default()
    };
    (@default $field_type:ty, $default:expr) => {
        $default
    };
}

#[macro_export]
macro_rules! validate_enum {
    ($enum_name:ident, $value:expr, [$($variant:ident),* $(,)?]) => {{
        let valid_variants = vec![$(stringify!($variant)),*];
        if valid_variants.contains(&$value) {
            Ok(())
        } else {
            Err(validator::ValidationError::new(&format!(
                "Invalid {}: must be one of {:?}",
                stringify!($enum_name),
                valid_variants
            )))
        }
    }};
}

#[macro_export]
macro_rules! validation_context {
    ($($field:ident: $value:expr),* $(,)?) => {{
        let mut context = $crate::validation::ValidationContext::default();
        $(
            context.$field = Some($value.to_string());
        )*
        context
    }};
}

#[macro_export]
macro_rules! validate_with_context_or_error {
    ($validatable:expr, $context:expr) => {{
        let validation_result = $validatable.validate_with_context($context);
        if !validation_result.is_valid {
            return Err($crate::error::AppError::BadRequest(
                serde_json::to_string(&validation_result.errors).unwrap_or_else(|_| "Validation failed".to_string())
            ));
        }
    }};
}

#[macro_export]
macro_rules! impl_comprehensive_validator {
    ($struct_name:ident {
        $($field:ident: $validator:expr),* $(,)?
    }) => {
        impl $crate::validation::ContextValidatable for $struct_name {
            fn validate_with_context(&self, _context: &$crate::validation::ValidationContext) -> $crate::validation::ValidationResult {
                let mut result = $crate::validation::ValidationResult::success();
                
                $(
                    if let Err(err) = $validator(&self.$field) {
                        result.add_error(stringify!($field), &err.to_string());
                    }
                )*
                
                result
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::validation::{ValidationResult};
    use validator::ValidationError;

    custom_validator!(
        validate_positive,
        value: &i32,
        *value > 0,
        "Value must be positive"
    );

    #[test]
    fn test_custom_validator_macro() {
        assert!(validate_positive(&5).is_ok());
        assert!(validate_positive(&-1).is_err());
    }

    #[test]
    fn test_validate_fields_macro() {
        let result = validate_fields!(
            "field1" => Ok::<(), ValidationError>(()),
            "field2" => Err::<(), ValidationError>(ValidationError::new("error message"))
        );
        
        assert!(!result.is_valid);
        assert!(result.errors.contains_key("field2"));
    }

    #[test]
    fn test_validate_collection_macro() {
        let numbers = vec![1, -2, 3, -4];
        let result = validate_collection!(numbers, "numbers", validate_positive);
        
        assert!(!result.is_valid);
        assert!(result.errors.contains_key("numbers[1]"));
        assert!(result.errors.contains_key("numbers[3]"));
    }

    validation_builder!(TestValidator {
        min_length: usize = 0,
        max_length: usize = 100,
        required: bool = false,
    });

    #[test]
    fn test_validation_builder_macro() {
        let validator = TestValidator::new()
            .min_length(5)
            .max_length(50)
            .required(true);
        
        assert_eq!(validator.min_length, 5);
        assert_eq!(validator.max_length, 50);
        assert_eq!(validator.required, true);
    }

    #[test]
    fn test_combine_validations_macro() {
        let mut result1 = ValidationResult::success();
        result1.add_error("field1", "error1");
        
        let mut result2 = ValidationResult::success();
        result2.add_error("field2", "error2");
        
        let combined = combine_validations!(result1, result2);
        
        assert!(!combined.is_valid);
        assert!(combined.errors.contains_key("field1"));
        assert!(combined.errors.contains_key("field2"));
    }
}
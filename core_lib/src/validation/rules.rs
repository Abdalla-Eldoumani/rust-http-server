//! Validation rules and custom validators

use lazy_static::lazy_static;
use regex::Regex;
use validator::ValidationError;
use std::collections::HashSet;

lazy_static! {
    static ref EMAIL_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();
    
    static ref USERNAME_REGEX: Regex = Regex::new(
        r"^[a-zA-Z0-9_-]{3,30}$"
    ).unwrap();
    
    static ref PASSWORD_REGEX: Regex = Regex::new(
        r"^[A-Za-z\d@$!%*?&]{8,}$"
    ).unwrap();
    
    static ref SLUG_REGEX: Regex = Regex::new(
        r"^[a-z0-9]+(?:-[a-z0-9]+)*$"
    ).unwrap();
    
    static ref PHONE_REGEX: Regex = Regex::new(
        r"^\+?[1-9]\d{1,14}$"
    ).unwrap();
    
    static ref URL_REGEX: Regex = Regex::new(
        r"^https?://[^\s/$.?#].[^\s]*$"
    ).unwrap();

    static ref DANGEROUS_EXTENSIONS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("exe");
        set.insert("bat");
        set.insert("cmd");
        set.insert("com");
        set.insert("pif");
        set.insert("scr");
        set.insert("vbs");
        set.insert("js");
        set.insert("jar");
        set.insert("sh");
        set.insert("ps1");
        set.insert("php");
        set.insert("asp");
        set.insert("aspx");
        set.insert("jsp");
        set
    };

    static ref SQL_INJECTION_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)(union\s+select)").unwrap(),
        Regex::new(r"(?i)(drop\s+table)").unwrap(),
        Regex::new(r"(?i)(delete\s+from)").unwrap(),
        Regex::new(r"(?i)(insert\s+into)").unwrap(),
        Regex::new(r"(?i)(update\s+\w+\s+set)").unwrap(),
        Regex::new(r"(?i)(exec\s*\()").unwrap(),
        Regex::new(r"(?i)(script\s*>)").unwrap(),
        Regex::new(r"(?i)(<\s*script)").unwrap(),
        Regex::new(r"(?i)(javascript\s*:)").unwrap(),
        Regex::new(r"(?i)(on\w+\s*=)").unwrap(),
    ];

    static ref XSS_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)<script[^>]*>").unwrap(),
        Regex::new(r"(?i)</script>").unwrap(),
        Regex::new(r"(?i)javascript:").unwrap(),
        Regex::new(r"(?i)on\w+\s*=").unwrap(),
        Regex::new(r"(?i)<iframe[^>]*>").unwrap(),
        Regex::new(r"(?i)<object[^>]*>").unwrap(),
        Regex::new(r"(?i)<embed[^>]*>").unwrap(),
        Regex::new(r"(?i)<link[^>]*>").unwrap(),
        Regex::new(r"(?i)<meta[^>]*>").unwrap(),
    ];
}

pub fn validate_email(email: &str) -> Result<(), ValidationError> {
    if email.is_empty() {
        return Err(ValidationError::new("Email cannot be empty"));
    }
    
    if email.len() > 254 {
        return Err(ValidationError::new("Email is too long"));
    }
    
    if !EMAIL_REGEX.is_match(email) {
        return Err(ValidationError::new("Invalid email format"));
    }
    
    Ok(())
}

pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    if username.is_empty() {
        return Err(ValidationError::new("Username cannot be empty"));
    }
    
    if !USERNAME_REGEX.is_match(username) {
        return Err(ValidationError::new(
            "Username must be 3-30 characters long and contain only letters, numbers, hyphens, and underscores"
        ));
    }
    
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.is_empty() {
        return Err(ValidationError::new("Password cannot be empty"));
    }
    
    if password.len() < 8 {
        return Err(ValidationError::new("Password must be at least 8 characters long"));
    }
    
    if password.len() > 128 {
        return Err(ValidationError::new("Password is too long"));
    }
    
    if !PASSWORD_REGEX.is_match(password) {
        return Err(ValidationError::new("Password contains invalid characters"));
    }
    
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| "@$!%*?&".contains(c));
    
    if !has_lowercase {
        return Err(ValidationError::new("Password must contain at least one lowercase letter"));
    }
    
    if !has_uppercase {
        return Err(ValidationError::new("Password must contain at least one uppercase letter"));
    }
    
    if !has_digit {
        return Err(ValidationError::new("Password must contain at least one digit"));
    }
    
    if !has_special {
        return Err(ValidationError::new("Password must contain at least one special character (@$!%*?&)"));
    }
    
    Ok(())
}

pub fn validate_slug(slug: &str) -> Result<(), ValidationError> {
    if slug.is_empty() {
        return Err(ValidationError::new("Slug cannot be empty"));
    }
    
    if slug.len() > 100 {
        return Err(ValidationError::new("Slug is too long"));
    }
    
    if !SLUG_REGEX.is_match(slug) {
        return Err(ValidationError::new(
            "Slug must contain only lowercase letters, numbers, and hyphens"
        ));
    }
    
    Ok(())
}

pub fn validate_phone(phone: &str) -> Result<(), ValidationError> {
    if phone.is_empty() {
        return Err(ValidationError::new("Phone number cannot be empty"));
    }
    
    if !PHONE_REGEX.is_match(phone) {
        return Err(ValidationError::new("Invalid phone number format"));
    }
    
    Ok(())
}

pub fn validate_url(url: &str) -> Result<(), ValidationError> {
    if url.is_empty() {
        return Err(ValidationError::new("URL cannot be empty"));
    }
    
    if url.len() > 2048 {
        return Err(ValidationError::new("URL is too long"));
    }
    
    if !URL_REGEX.is_match(url) {
        return Err(ValidationError::new("Invalid URL format"));
    }
    
    Ok(())
}

pub fn validate_file_extension(filename: &str) -> Result<(), ValidationError> {
    if filename.is_empty() {
        return Err(ValidationError::new("Filename cannot be empty"));
    }
    
    let extension = filename
        .split('.')
        .last()
        .unwrap_or("")
        .to_lowercase();
    
    if DANGEROUS_EXTENSIONS.contains(extension.as_str()) {
        return Err(ValidationError::new("File type not allowed for security reasons"));
    }
    
    Ok(())
}

pub fn validate_file_size(size: u64, max_size: u64) -> Result<(), ValidationError> {
    if size == 0 {
        return Err(ValidationError::new("File cannot be empty"));
    }
    
    if size > max_size {
        return Err(ValidationError::new("File size exceeds maximum allowed size"));
    }
    
    Ok(())
}

pub fn validate_no_sql_injection(input: &str) -> Result<(), ValidationError> {
    for pattern in SQL_INJECTION_PATTERNS.iter() {
        if pattern.is_match(input) {
            return Err(ValidationError::new("Input contains potentially dangerous SQL patterns"));
        }
    }
    Ok(())
}

pub fn validate_no_xss(input: &str) -> Result<(), ValidationError> {
    for pattern in XSS_PATTERNS.iter() {
        if pattern.is_match(input) {
            return Err(ValidationError::new("Input contains potentially dangerous script patterns"));
        }
    }
    Ok(())
}

pub fn validate_text_length(text: &str, min: usize, max: usize) -> Result<(), ValidationError> {
    let len = text.chars().count();
    
    if len < min {
        return Err(ValidationError::new("Text is too short"));
    }
    
    if len > max {
        return Err(ValidationError::new("Text is too long"));
    }
    
    Ok(())
}

pub fn validate_allowed_chars(text: &str, allowed_pattern: &str) -> Result<(), ValidationError> {
    let regex = Regex::new(allowed_pattern)
        .map_err(|_| ValidationError::new("Invalid validation pattern"))?;
    
    if !regex.is_match(text) {
        return Err(ValidationError::new("Text contains invalid characters"));
    }
    
    Ok(())
}

pub fn validate_numeric_range<T>(value: T, min: T, max: T) -> Result<(), ValidationError>
where
    T: PartialOrd + std::fmt::Display,
{
    if value < min {
        return Err(ValidationError::new("Value is below minimum"));
    }
    
    if value > max {
        return Err(ValidationError::new("Value exceeds maximum"));
    }
    
    Ok(())
}

pub fn validate_not_in_list(value: &str, forbidden: &[&str]) -> Result<(), ValidationError> {
    if forbidden.contains(&value) {
        return Err(ValidationError::new("Value is not allowed"));
    }
    Ok(())
}

pub fn validate_in_list(value: &str, allowed: &[&str]) -> Result<(), ValidationError> {
    if !allowed.contains(&value) {
        return Err(ValidationError::new("Value is not in the allowed list"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("invalid-email").is_err());
        assert!(validate_email("").is_err());
    }

    #[test]
    fn test_username_validation() {
        assert!(validate_username("valid_user123").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("invalid user").is_err());
    }

    #[test]
    fn test_password_validation() {
        assert!(validate_password("StrongPass123!").is_ok());
        assert!(validate_password("weak").is_err());
        assert!(validate_password("").is_err());
    }

    #[test]
    fn test_sql_injection_detection() {
        assert!(validate_no_sql_injection("normal text").is_ok());
        assert!(validate_no_sql_injection("'; DROP TABLE users; --").is_err());
        assert!(validate_no_sql_injection("UNION SELECT * FROM passwords").is_err());
    }

    #[test]
    fn test_xss_detection() {
        assert!(validate_no_xss("normal text").is_ok());
        assert!(validate_no_xss("<script>alert('xss')</script>").is_err());
        assert!(validate_no_xss("javascript:alert('xss')").is_err());
    }

    #[test]
    fn test_file_extension_validation() {
        assert!(validate_file_extension("document.pdf").is_ok());
        assert!(validate_file_extension("image.jpg").is_ok());
        assert!(validate_file_extension("malware.exe").is_err());
        assert!(validate_file_extension("script.js").is_err());
    }
}
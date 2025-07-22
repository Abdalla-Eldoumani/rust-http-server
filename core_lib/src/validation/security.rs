//! Security-focused validation utilities

use super::{ValidationResult};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashSet;
use validator::ValidationError;

lazy_static! {
    static ref LDAP_INJECTION_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)(\*\)|&\||!\|)").unwrap(),
        Regex::new(r"(?i)(\(\||\)\||&\()").unwrap(),
    ];

    static ref COMMAND_INJECTION_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)(;\s*rm\s)").unwrap(),
        Regex::new(r"(?i)(;\s*cat\s)").unwrap(),
        Regex::new(r"(?i)(;\s*ls\s)").unwrap(),
        Regex::new(r"(?i)(;\s*wget\s)").unwrap(),
        Regex::new(r"(?i)(;\s*curl\s)").unwrap(),
        Regex::new(r"(?i)(\|\s*nc\s)").unwrap(),
        Regex::new(r"(?i)(\$\(|\`|&&|\|\|)").unwrap(),
    ];

    static ref PATH_TRAVERSAL_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"\.\.[\\/]").unwrap(),
        Regex::new(r"[\\/]\.\.").unwrap(),
        Regex::new(r"%2e%2e[\\/]").unwrap(),
        Regex::new(r"[\\/]%2e%2e").unwrap(),
    ];

    static ref SUSPICIOUS_USER_AGENTS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("sqlmap");
        set.insert("nikto");
        set.insert("nmap");
        set.insert("masscan");
        set.insert("zap");
        set.insert("burp");
        set.insert("w3af");
        set.insert("acunetix");
        set.insert("nessus");
        set
    };

    static ref RATE_LIMIT_KEYS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("/auth/login");
        set.insert("/auth/register");
        set.insert("/api/files/upload");
        set.insert("/api/items");
        set
    };
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub request_path: String,
    pub request_method: String,
    pub headers: std::collections::HashMap<String, String>,
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self {
            ip_address: None,
            user_agent: None,
            referer: None,
            request_path: String::new(),
            request_method: String::new(),
            headers: std::collections::HashMap::new(),
        }
    }
}

pub struct SecurityValidator;

impl SecurityValidator {
    pub fn validate_sql_injection(input: &str) -> Result<(), ValidationError> {
        super::rules::validate_no_sql_injection(input)
    }

    pub fn validate_xss(input: &str) -> Result<(), ValidationError> {
        super::rules::validate_no_xss(input)
    }

    pub fn validate_ldap_injection(input: &str) -> Result<(), ValidationError> {
        for pattern in LDAP_INJECTION_PATTERNS.iter() {
            if pattern.is_match(input) {
                return Err(ValidationError::new("Input contains potentially dangerous LDAP patterns"));
            }
        }
        Ok(())
    }

    pub fn validate_command_injection(input: &str) -> Result<(), ValidationError> {
        for pattern in COMMAND_INJECTION_PATTERNS.iter() {
            if pattern.is_match(input) {
                return Err(ValidationError::new("Input contains potentially dangerous command patterns"));
            }
        }
        Ok(())
    }

    pub fn validate_path_traversal(input: &str) -> Result<(), ValidationError> {
        for pattern in PATH_TRAVERSAL_PATTERNS.iter() {
            if pattern.is_match(input) {
                return Err(ValidationError::new("Input contains path traversal patterns"));
            }
        }
        Ok(())
    }

    pub fn validate_user_agent(user_agent: &str) -> Result<(), ValidationError> {
        let ua_lower = user_agent.to_lowercase();
        
        for suspicious_ua in SUSPICIOUS_USER_AGENTS.iter() {
            if ua_lower.contains(suspicious_ua) {
                return Err(ValidationError::new("Suspicious user agent detected"));
            }
        }
        
        Ok(())
    }

    pub fn validate_headers(headers: &std::collections::HashMap<String, String>) -> ValidationResult {
        let mut result = ValidationResult::success();

        for (name, value) in headers {
            let name_lower = name.to_lowercase();
            
            if name_lower == "user-agent" {
                if let Err(err) = Self::validate_user_agent(value) {
                    result.add_error("user-agent", &err.to_string());
                }
            }
            
            if let Err(_) = Self::validate_sql_injection(value) {
                result.add_error(&name_lower, "Header contains potentially dangerous SQL patterns");
            }
            
            if let Err(_) = Self::validate_xss(value) {
                result.add_error(&name_lower, "Header contains potentially dangerous script patterns");
            }
            
            if let Err(_) = Self::validate_command_injection(value) {
                result.add_error(&name_lower, "Header contains potentially dangerous command patterns");
            }
        }

        result
    }

    pub fn validate_file_upload_security(
        filename: &str,
        content_type: &str,
        content: &[u8],
    ) -> ValidationResult {
        let mut result = ValidationResult::success();

        if let Err(err) = Self::validate_path_traversal(filename) {
            result.add_error("filename", &err.to_string());
        }

        if filename.contains('\0') {
            result.add_error("filename", "Filename contains null bytes");
        }

        if content_type.is_empty() {
            result.add_error("content_type", "Content type is required");
        } else if content_type.contains('\0') {
            result.add_error("content_type", "Content type contains null bytes");
        }

        if let Ok(content_str) = std::str::from_utf8(content) {
            if let Err(_) = Self::validate_xss(content_str) {
                result.add_error("content", "File content contains potentially dangerous scripts");
            }
        }

        if let Err(err) = Self::validate_file_signature(filename, content) {
            result.add_error("content", &err.to_string());
        }

        result
    }

    fn validate_file_signature(filename: &str, content: &[u8]) -> Result<(), ValidationError> {
        if content.len() < 4 {
            return Ok(());
        }

        let extension = filename
            .split('.')
            .last()
            .unwrap_or("")
            .to_lowercase();

        let header = &content[0..std::cmp::min(content.len(), 16)];

        match extension.as_str() {
            "jpg" | "jpeg" => {
                if !header.starts_with(&[0xFF, 0xD8, 0xFF]) {
                    return Err(ValidationError::new("File signature does not match JPEG extension"));
                }
            }
            "png" => {
                if !header.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
                    return Err(ValidationError::new("File signature does not match PNG extension"));
                }
            }
            "gif" => {
                if !header.starts_with(b"GIF87a") && !header.starts_with(b"GIF89a") {
                    return Err(ValidationError::new("File signature does not match GIF extension"));
                }
            }
            "pdf" => {
                if !header.starts_with(b"%PDF") {
                    return Err(ValidationError::new("File signature does not match PDF extension"));
                }
            }
            "zip" => {
                if !header.starts_with(&[0x50, 0x4B, 0x03, 0x04]) &&
                   !header.starts_with(&[0x50, 0x4B, 0x05, 0x06]) &&
                   !header.starts_with(&[0x50, 0x4B, 0x07, 0x08]) {
                    return Err(ValidationError::new("File signature does not match ZIP extension"));
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn should_rate_limit(path: &str) -> bool {
        RATE_LIMIT_KEYS.iter().any(|&key| path.starts_with(key))
    }

    pub fn validate_input_security(input: &str) -> ValidationResult {
        let mut result = ValidationResult::success();

        if let Err(_) = Self::validate_sql_injection(input) {
            result.add_error("input", "Input contains potentially dangerous SQL patterns");
        }

        if let Err(_) = Self::validate_xss(input) {
            result.add_error("input", "Input contains potentially dangerous script patterns");
        }

        if let Err(_) = Self::validate_ldap_injection(input) {
            result.add_error("input", "Input contains potentially dangerous LDAP patterns");
        }

        if let Err(_) = Self::validate_command_injection(input) {
            result.add_error("input", "Input contains potentially dangerous command patterns");
        }

        if let Err(_) = Self::validate_path_traversal(input) {
            result.add_error("input", "Input contains path traversal patterns");
        }

        result
    }

    pub fn validate_request_security(context: &SecurityContext) -> ValidationResult {
        let mut result = ValidationResult::success();

        if let Some(user_agent) = &context.user_agent {
            if let Err(err) = Self::validate_user_agent(user_agent) {
                result.add_error("user_agent", &err.to_string());
            }
        }

        let headers_result = Self::validate_headers(&context.headers);
        result.merge(headers_result);

        if context.request_path.len() > 2048 {
            result.add_error("request_path", "Request path is too long");
        }

        if let Err(_) = Self::validate_path_traversal(&context.request_path) {
            result.add_error("request_path", "Request path contains traversal patterns");
        }

        result
    }
}

pub struct IpSecurityValidator;

impl IpSecurityValidator {
    pub fn is_private_ip(ip: &str) -> bool {
        if let Ok(addr) = ip.parse::<std::net::IpAddr>() {
            match addr {
                std::net::IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                    octets[0] == 10 ||
                    (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31) ||
                    (octets[0] == 192 && octets[1] == 168) ||
                    octets[0] == 127 // localhost
                }
                std::net::IpAddr::V6(ipv6) => {
                    ipv6.is_loopback() || ipv6.segments()[0] == 0xfc00 || ipv6.segments()[0] == 0xfd00
                }
            }
        } else {
            false
        }
    }

    pub fn validate_ip_format(ip: &str) -> Result<(), ValidationError> {
        if ip.parse::<std::net::IpAddr>().is_err() {
            return Err(ValidationError::new("Invalid IP address format"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_validation() {
        assert!(SecurityValidator::validate_sql_injection("normal text").is_ok());
        assert!(SecurityValidator::validate_sql_injection("'; DROP TABLE users; --").is_err());
    }

    #[test]
    fn test_xss_validation() {
        assert!(SecurityValidator::validate_xss("normal text").is_ok());
        assert!(SecurityValidator::validate_xss("<script>alert('xss')</script>").is_err());
    }

    #[test]
    fn test_command_injection_validation() {
        assert!(SecurityValidator::validate_command_injection("normal text").is_ok());
        assert!(SecurityValidator::validate_command_injection("; rm -rf /").is_err());
    }

    #[test]
    fn test_path_traversal_validation() {
        assert!(SecurityValidator::validate_path_traversal("normal/path").is_ok());
        assert!(SecurityValidator::validate_path_traversal("../../../etc/passwd").is_err());
    }

    #[test]
    fn test_user_agent_validation() {
        assert!(SecurityValidator::validate_user_agent("Mozilla/5.0").is_ok());
        assert!(SecurityValidator::validate_user_agent("sqlmap/1.0").is_err());
    }

    #[test]
    fn test_file_signature_validation() {
        let jpeg_content = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert!(SecurityValidator::validate_file_signature("test.jpg", &jpeg_content).is_ok());

        let fake_jpeg = vec![0x00, 0x00, 0x00, 0x00];
        assert!(SecurityValidator::validate_file_signature("test.jpg", &fake_jpeg).is_err());
    }

    #[test]
    fn test_private_ip_detection() {
        assert!(IpSecurityValidator::is_private_ip("192.168.1.1"));
        assert!(IpSecurityValidator::is_private_ip("10.0.0.1"));
        assert!(IpSecurityValidator::is_private_ip("127.0.0.1"));
        assert!(!IpSecurityValidator::is_private_ip("8.8.8.8"));
    }
}
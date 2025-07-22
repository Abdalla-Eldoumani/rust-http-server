//! Validation middleware for automatic input checking

use super::{ValidationResult, ValidationContext, ContextValidatable, SecurityValidator, SecurityContext};
use crate::error::{AppError, Result};
use axum::{
    extract::{Request, ConnectInfo},
    http::{HeaderMap, Method, Uri},
    middleware::Next,
    response::Response,
};
use std::{collections::HashMap, net::SocketAddr};
use tracing::{warn, debug};

pub async fn validation_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    request: Request,
    next: Next,
) -> std::result::Result<Response, AppError> {
    let ip_address = addr.ip().to_string();
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let referer = headers
        .get("referer")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let mut header_map = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            header_map.insert(name.to_string(), value_str.to_string());
        }
    }

    let security_context = SecurityContext {
        ip_address: Some(ip_address.clone()),
        user_agent: user_agent.clone(),
        referer: referer.clone(),
        request_path: uri.path().to_string(),
        request_method: method.to_string(),
        headers: header_map,
    };

    let security_result = SecurityValidator::validate_request_security(&security_context);
    if !security_result.is_valid {
        warn!(
            "Security validation failed for request from {}: {:?}",
            ip_address, security_result.errors
        );
        return Err(AppError::BadRequest(format!(
            "Request failed security validation: {}",
            serde_json::to_string(&security_result.errors).unwrap_or_default()
        )));
    }

    if let Some(ua) = &user_agent {
        if SecurityValidator::validate_user_agent(ua).is_err() {
            warn!("Suspicious user agent detected from {}: {}", ip_address, ua);
        }
    }

    if SecurityValidator::should_rate_limit(uri.path()) {
        debug!("Rate limiting check for path: {}", uri.path());
    }

    let response = next.run(request).await;
    Ok(response)
}

pub async fn json_validation_middleware<T>(
    payload: T,
    context: ValidationContext,
) -> Result<T>
where
    T: ContextValidatable,
{
    let validation_result = payload.validate_with_context(&context);
    
    if !validation_result.is_valid {
        return Err(AppError::BadRequest(format!(
            "Validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    Ok(payload)
}

pub async fn file_upload_validation_middleware(
    filename: &str,
    content_type: &str,
    content: &[u8],
) -> Result<()> {
    let validation_result = SecurityValidator::validate_file_upload_security(
        filename,
        content_type,
        content,
    );
    
    if !validation_result.is_valid {
        return Err(AppError::BadRequest(format!(
            "File upload validation failed: {}",
            serde_json::to_string(&validation_result.errors).unwrap_or_default()
        )));
    }
    
    Ok(())
}

pub fn extract_validation_context(
    headers: &HeaderMap,
    addr: &SocketAddr,
    user_id: Option<u64>,
    user_role: Option<String>,
) -> ValidationContext {
    let mut context = ValidationContext::default();
    
    context.user_id = user_id;
    context.user_role = user_role;
    context.request_ip = Some(addr.ip().to_string());
    
    if let Some(user_agent) = headers.get("user-agent").and_then(|h| h.to_str().ok()) {
        context.additional_data.insert(
            "user_agent".to_string(),
            serde_json::Value::String(user_agent.to_string()),
        );
    }
    
    if let Some(referer) = headers.get("referer").and_then(|h| h.to_str().ok()) {
        context.additional_data.insert(
            "referer".to_string(),
            serde_json::Value::String(referer.to_string()),
        );
    }
    
    context
}

pub struct ValidationResponse;

impl ValidationResponse {
    pub fn validation_error(result: ValidationResult) -> AppError {
        AppError::BadRequest(format!(
            "Validation failed: {}",
            serde_json::to_string(&result).unwrap_or_else(|_| "Unknown validation error".to_string())
        ))
    }
    
    pub fn security_error(message: &str) -> AppError {
        AppError::BadRequest(format!("Security validation failed: {}", message))
    }
    
    pub fn file_error(message: &str) -> AppError {
        AppError::BadRequest(format!("File validation failed: {}", message))
    }
}

pub trait ValidatableRequest {
    fn validate_request(&self, context: &ValidationContext) -> ValidationResult;
}

pub trait ValidatableResponse {
    fn validate_response(&self, context: &ValidationContext) -> ValidationResult;
}

pub fn sanitize_input(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

pub fn validate_request_size(content_length: Option<u64>, max_size: u64) -> Result<()> {
    if let Some(length) = content_length {
        if length > max_size {
            return Err(AppError::BadRequest(format!(
                "Request too large: {} bytes (max: {} bytes)",
                length, max_size
            )));
        }
    }
    Ok(())
}

pub fn validate_content_type(content_type: Option<&str>, allowed_types: &[&str]) -> Result<()> {
    match content_type {
        Some(ct) => {
            if !allowed_types.iter().any(|&allowed| ct.starts_with(allowed)) {
                return Err(AppError::BadRequest(format!(
                    "Unsupported content type: {}. Allowed types: {:?}",
                    ct, allowed_types
                )));
            }
        }
        None => {
            return Err(AppError::BadRequest("Content-Type header is required".to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_extract_validation_context() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static("test-agent"));
        headers.insert("referer", HeaderValue::from_static("https://example.com"));
        
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        
        let context = extract_validation_context(&headers, &addr, Some(123), Some("admin".to_string()));
        
        assert_eq!(context.user_id, Some(123));
        assert_eq!(context.user_role, Some("admin".to_string()));
        assert_eq!(context.request_ip, Some("127.0.0.1".to_string()));
        assert!(context.additional_data.contains_key("user_agent"));
        assert!(context.additional_data.contains_key("referer"));
    }

    #[test]
    fn test_sanitize_input() {
        let input = "<script>alert('xss')</script>";
        let sanitized = sanitize_input(input);
        assert_eq!(sanitized, "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;&#x2F;script&gt;");
    }

    #[test]
    fn test_validate_request_size() {
        assert!(validate_request_size(Some(1000), 2000).is_ok());
        assert!(validate_request_size(Some(3000), 2000).is_err());
        assert!(validate_request_size(None, 2000).is_ok());
    }

    #[test]
    fn test_validate_content_type() {
        let allowed = &["application/json", "text/plain"];
        
        assert!(validate_content_type(Some("application/json"), allowed).is_ok());
        assert!(validate_content_type(Some("application/json; charset=utf-8"), allowed).is_ok());
        assert!(validate_content_type(Some("application/xml"), allowed).is_err());
        assert!(validate_content_type(None, allowed).is_err());
    }
}
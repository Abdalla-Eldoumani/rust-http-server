//! Request validation middleware for content type and size limits

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::convert::Infallible;

const MAX_BODY_SIZE: usize = 1024 * 1024;

pub async fn request_validation_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, Infallible> {
    let (parts, body) = request.into_parts();
    
    if matches!(parts.method.as_str(), "POST" | "PUT" | "PATCH") {
        let path = parts.uri.path();
        let has_body = parts.headers.get("content-length")
            .and_then(|cl| cl.to_str().ok())
            .and_then(|cl| cl.parse::<usize>().ok())
            .map(|len| len > 0)
            .unwrap_or(false);
        
        let skip_validation = path.ends_with("/cleanup") || path.ends_with("/logout") || path.ends_with("/retry") || path.ends_with("/clear") || (!has_body && !path.contains("/items") && !path.contains("/register") && !path.contains("/login"));
        
        if !skip_validation {
            if let Some(content_type) = parts.headers.get("content-type") {
                let content_type_str = content_type.to_str().unwrap_or("");
                
                if !content_type_str.starts_with("application/json") 
                    && !content_type_str.starts_with("application/x-www-form-urlencoded")
                    && !content_type_str.starts_with("multipart/form-data") {
                    
                    let error_response = Json(json!({
                        "error": "Unsupported content type. Expected application/json, application/x-www-form-urlencoded, or multipart/form-data",
                        "status": 415
                    }));
                    
                    return Ok((StatusCode::UNSUPPORTED_MEDIA_TYPE, error_response).into_response());
                }
            } else if has_body {
                let error_response = Json(json!({
                    "error": "Missing Content-Type header. Expected application/json, application/x-www-form-urlencoded, or multipart/form-data",
                    "status": 415
                }));
                
                return Ok((StatusCode::UNSUPPORTED_MEDIA_TYPE, error_response).into_response());
            }
        }
    }
    
    if let Some(content_length) = parts.headers.get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                if length > MAX_BODY_SIZE {
                    let error_response = Json(json!({
                        "error": format!("Request body too large. Maximum size is {} bytes", MAX_BODY_SIZE),
                        "status": 413
                    }));
                    
                    return Ok((StatusCode::PAYLOAD_TOO_LARGE, error_response).into_response());
                }
            }
        }
    }
    
    let request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

pub async fn security_headers_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, Infallible> {
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    headers.insert("Content-Security-Policy", "default-src 'self'".parse().unwrap());
    
    headers.insert("X-API-Version", "1.0".parse().unwrap());
    headers.insert("API-Version", "1.0".parse().unwrap());
    
    Ok(response)
}
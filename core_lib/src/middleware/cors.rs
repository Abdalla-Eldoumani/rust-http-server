//! CORS (Cross-Origin Resource Sharing) middleware configuration

use crate::config::CorsConfig;
use tower_http::cors::{Any, CorsLayer as TowerCorsLayer};
use axum::http::{HeaderValue, Method, HeaderName};

pub fn cors_layer_from_config(config: &CorsConfig) -> TowerCorsLayer {
    if config.enable_permissive_mode {
        return cors_layer_permissive();
    }

    let origins: Vec<HeaderValue> = config.allowed_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    let methods: Vec<Method> = config.allowed_methods
        .iter()
        .filter_map(|method| method.parse().ok())
        .collect();

    let allowed_headers: Vec<HeaderName> = config.allowed_headers
        .iter()
        .filter_map(|header| header.parse().ok())
        .collect();

    let exposed_headers: Vec<HeaderName> = config.exposed_headers
        .iter()
        .filter_map(|header| header.parse().ok())
        .collect();

    TowerCorsLayer::new()
        .allow_origin(origins)
        .allow_methods(methods)
        .allow_headers(allowed_headers)
        .expose_headers(exposed_headers)
        .allow_credentials(config.allow_credentials)
        .max_age(std::time::Duration::from_secs(config.max_age_seconds))
}

pub fn cors_layer() -> TowerCorsLayer {
    let allowed_origins = vec![
        "http://localhost:3000",
        "http://localhost:3001", 
        "http://localhost:5173",
        "http://localhost:5174",
        "http://localhost:8080",
        "http://127.0.0.1:3000",
        "http://127.0.0.1:8080",
        "https://localhost:3000",
        "https://localhost:8080",
    ];
    
    let origins: Vec<HeaderValue> = allowed_origins
        .into_iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    TowerCorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::HEAD,
            Method::OPTIONS,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("accept"),
            HeaderName::from_static("x-requested-with"),
            HeaderName::from_static("user-agent"),
            HeaderName::from_static("origin"),
            HeaderName::from_static("referer"),
            HeaderName::from_static("cache-control"),
        ])
        .expose_headers([
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-response-time"),
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600))
}

#[allow(dead_code)]
pub fn cors_layer_permissive() -> TowerCorsLayer {
    TowerCorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any)
        .allow_credentials(false)
        .max_age(std::time::Duration::from_secs(3600))
}

#[allow(dead_code)]
pub fn cors_layer_production(allowed_origins: Vec<&str>) -> TowerCorsLayer {
    let origins: Vec<HeaderValue> = allowed_origins
        .into_iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    TowerCorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("accept"),
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600))
}
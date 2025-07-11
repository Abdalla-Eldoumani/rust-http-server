//! CORS (Cross-Origin Resource Sharing) middleware configuration

use tower_http::cors::{Any, CorsLayer as TowerCorsLayer};
use axum::http::{HeaderValue, Method, HeaderName};

pub fn cors_layer() -> TowerCorsLayer {
    TowerCorsLayer::new()
        .allow_origin(Any)
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
        .allow_credentials(false)
        .max_age(std::time::Duration::from_secs(3600))
}
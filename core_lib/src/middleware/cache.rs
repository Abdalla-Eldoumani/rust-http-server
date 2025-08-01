use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Method, Request},
    middleware::Next,
    response::Response,
};
use std::time::Duration;
use tracing::{debug, warn};

use crate::{cache::CacheManager, AppState};

#[derive(Debug, Clone)]
pub struct CacheMiddlewareConfig {
    pub default_ttl: Duration,
    pub cache_get: bool,
    pub cache_post: bool,
    pub max_response_size: usize,
    pub key_prefix: String,
}

impl Default for CacheMiddlewareConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300),
            cache_get: true,
            cache_post: false,
            max_response_size: 1024 * 1024,
            key_prefix: "http_cache".to_string(),
        }
    }
}

pub async fn cache_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> std::result::Result<Response, std::convert::Infallible> {
    debug!("Cache middleware called for: {} {}", request.method(), request.uri());
    
    let cache_manager = match &state.cache_manager {
        Some(cache) => {
            debug!("Cache manager found, proceeding with caching logic");
            cache
        },
        None => {
            debug!("No cache manager found, skipping cache");
            return Ok(next.run(request).await);
        }
    };

    let config = CacheMiddlewareConfig::default();
    
    if !should_cache_request(&request, &config) {
        return Ok(next.run(request).await);
    }

    let cache_key = generate_cache_key(&request, &config);
    
    if let Some(cached_response) = get_cached_response(cache_manager, &cache_key).await {
        debug!("Cache hit for key: {}", cache_key);
        return Ok(cached_response);
    }

    debug!("Cache miss for key: {}", cache_key);
    let response = next.run(request).await;
    
    if response.status().is_success() {
        let (parts, body) = response.into_parts();
        
        match axum::body::to_bytes(body, config.max_response_size).await {
            Ok(body_bytes) => {
                let cached_response = CachedResponse {
                    status_code: parts.status.as_u16(),
                    headers: parts.headers
                        .iter()
                        .filter_map(|(name, value)| {
                            let name_str = name.as_str().to_lowercase();
                            if should_cache_header(&name_str) {
                                value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
                            } else {
                                None
                            }
                        })
                        .collect(),
                    body: body_bytes.to_vec(),
                };

                if let Err(e) = cache_manager.set_with_ttl(&cache_key, &cached_response, Some(config.default_ttl)) {
                    warn!("Failed to cache response for key {}: {}", cache_key, e);
                } else {
                    debug!("Cached response for key: {} ({} bytes)", cache_key, cached_response.body.len());
                }

                let mut response_builder = Response::builder().status(parts.status);
                
                for (name, value) in parts.headers.iter() {
                    response_builder = response_builder.header(name, value);
                }
                
                response_builder = response_builder.header("X-Cache", "MISS");
                
                Ok(match response_builder.body(Body::from(body_bytes.clone())) {
                    Ok(response) => response,
                    Err(_) => {
                        Response::builder()
                            .status(parts.status)
                            .header("X-Cache", "MISS")
                            .body(Body::from(body_bytes))
                            .unwrap_or_else(|_| Response::new(Body::empty()))
                    }
                })
            }
            Err(e) => {
                warn!("Failed to extract response body for caching: {}", e);
                let mut response = Response::from_parts(parts, Body::empty());
                response.headers_mut().insert("X-Cache", HeaderValue::from_static("MISS"));
                Ok(response)
            }
        }
    } else {
        let (mut parts, body) = response.into_parts();
        parts.headers.insert("X-Cache", HeaderValue::from_static("MISS"));
        Ok(Response::from_parts(parts, body))
    }
}

fn should_cache_request(request: &Request<Body>, config: &CacheMiddlewareConfig) -> bool {
    let path = request.uri().path();
    
    if path.starts_with("/auth/") {
        return false;
    }
    
    if request.headers().contains_key("authorization") {
        return false;
    }
    
    match request.method() {
        &Method::GET => config.cache_get,
        &Method::POST => config.cache_post,
        _ => false,
    }
}

fn generate_cache_key(request: &Request<Body>, config: &CacheMiddlewareConfig) -> String {
    let method = request.method().as_str();
    let path = request.uri().path();
    let query = request.uri().query().unwrap_or("");
    
    if query.is_empty() {
        format!("{}:{}:{}", config.key_prefix, method, path)
    } else {
        format!("{}:{}:{}?{}", config.key_prefix, method, path, query)
    }
}

async fn get_cached_response(
    cache_manager: &CacheManager,
    cache_key: &str,
) -> Option<Response> {
    let cached_data: Option<CachedResponse> = cache_manager.get(cache_key);
    
    if let Some(cached) = cached_data {
        let mut response = Response::builder()
            .status(cached.status_code);
        
        for (name, value) in cached.headers {
            if let (Ok(header_name), Ok(header_value)) = (
                name.parse::<axum::http::HeaderName>(),
                value.parse::<HeaderValue>(),
            ) {
                response = response.header(header_name, header_value);
            }
        }
        
        response = response
            .header("X-Cache", "HIT")
            .header("X-Cache-Key", cache_key);
        
        if let Ok(response) = response.body(Body::from(cached.body)) {
            return Some(response);
        }
    }
    
    None
}

fn should_cache_header(header_name: &str) -> bool {
    !matches!(
        header_name,
        "date" | "server" | "x-request-id" | "x-trace-id" | "set-cookie" | "authorization"
    )
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedResponse {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl CacheManager {
    pub fn invalidate_path(&self, path: &str) {
        let pattern = format!("http_cache:GET:{}", path);
        self.invalidate_pattern(&pattern);
        
        let pattern = format!("http_cache:POST:{}", path);
        self.invalidate_pattern(&pattern);
    }

    pub fn invalidate_items_cache(&self) {
        self.invalidate_pattern("http_cache:GET:/api/items");
        self.invalidate_pattern("http_cache:GET:/api/v1/items");
        self.invalidate_pattern("http_cache:GET:/api/v2/items");
    }

    pub fn invalidate_item_cache(&self, item_id: u64) {
        let patterns = [
            format!("http_cache:GET:/api/items/{}", item_id),
            format!("http_cache:GET:/api/v1/items/{}", item_id),
            format!("http_cache:GET:/api/v2/items/{}", item_id),
        ];
        
        for pattern in patterns {
            self.invalidate_pattern(&pattern);
        }
    }

    pub fn invalidate_search_cache(&self) {
        self.invalidate_pattern("http_cache:GET:/api/items/search");
        self.invalidate_pattern("http_cache:GET:/api/v1/items/search");
        self.invalidate_pattern("http_cache:GET:/api/v2/items/search");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Method};

    #[test]
    fn test_should_cache_request() {
        let config = CacheMiddlewareConfig::default();
        
        let get_request = Request::builder()
            .method(Method::GET)
            .uri("/api/items")
            .body(Body::empty())
            .unwrap();
        
        let post_request = Request::builder()
            .method(Method::POST)
            .uri("/api/items")
            .body(Body::empty())
            .unwrap();
        
        assert!(should_cache_request(&get_request, &config));
        assert!(!should_cache_request(&post_request, &config));
    }

    #[test]
    fn test_generate_cache_key() {
        let config = CacheMiddlewareConfig::default();
        
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/items?page=1&limit=10")
            .body(Body::empty())
            .unwrap();
        
        let key = generate_cache_key(&request, &config);
        assert_eq!(key, "http_cache:GET:/api/items?page=1&limit=10");
    }

    #[test]
    fn test_should_cache_header() {
        assert!(should_cache_header("content-type"));
        assert!(should_cache_header("content-length"));
        assert!(should_cache_header("cache-control"));
        
        assert!(!should_cache_header("date"));
        assert!(!should_cache_header("server"));
        assert!(!should_cache_header("set-cookie"));
        assert!(!should_cache_header("authorization"));
    }
}
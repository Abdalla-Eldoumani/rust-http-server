//! Rate limiting middleware

use crate::config::RateLimitConfig;
use crate::middleware::auth::AuthUser;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::net::IpAddr;
use parking_lot::Mutex;
use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
    body::Body,
};
use serde_json::json;
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RateLimitKey {
    Ip(IpAddr),
    User(i64),
}

#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<RateLimitKey, Vec<Instant>>>>,
    config: RateLimitConfig,
    window: Duration,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            config,
            window: Duration::from_secs(60),
        }
    }

    pub fn check(&self, key: RateLimitKey) -> Result<(), RateLimitError> {
        if !self.config.enable {
            return Ok(());
        }

        let max_requests = self.get_limit_for_key(&key);
        let now = Instant::now();
        let mut requests = self.requests.lock();
        
        let entries = requests.entry(key.clone()).or_insert_with(Vec::new);
        
        entries.retain(|&instant| now.duration_since(instant) < self.window);
        
        tracing::debug!("Rate limit check: key={:?}, current_requests={}, max_requests={}", key, entries.len(), max_requests);
        
        if entries.len() >= max_requests {
            let oldest = entries.first().copied().unwrap_or(now);
            let reset_in = self.window.saturating_sub(now.duration_since(oldest));
            
            tracing::warn!("Rate limit exceeded for {:?}: {} >= {}", key, entries.len(), max_requests);
            
            return Err(RateLimitError {
                retry_after_seconds: reset_in.as_secs(),
                limit: max_requests,
                remaining: 0,
                key_type: self.get_key_type(&key),
            });
        }
        
        entries.push(now);
        
        Ok(())
    }

    pub fn get_current_usage(&self, key: &RateLimitKey) -> (usize, usize) {
        let max_requests = self.get_limit_for_key(key);
        let now = Instant::now();
        let mut requests = self.requests.lock();
        
        let entries = requests.entry(key.clone()).or_insert_with(Vec::new);
        entries.retain(|&instant| now.duration_since(instant) < self.window);
        
        let used = entries.len();
        let remaining = max_requests.saturating_sub(used);
        
        (used, remaining)
    }

    fn get_limit_for_key(&self, key: &RateLimitKey) -> usize {
        match key {
            RateLimitKey::Ip(_) => self.config.requests_per_minute,
            RateLimitKey::User(_) => {
                if self.config.enable_user_based_limits {
                    self.config.user_requests_per_minute
                } else {
                    self.config.requests_per_minute
                }
            }
        }
    }

    fn get_key_type(&self, key: &RateLimitKey) -> String {
        match key {
            RateLimitKey::Ip(_) => "ip".to_string(),
            RateLimitKey::User(_) => "user".to_string(),
        }
    }

    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut requests = self.requests.lock();
        
        requests.retain(|_, entries| {
            entries.retain(|&instant| now.duration_since(instant) < self.window);
            !entries.is_empty()
        });
    }
}

#[derive(Debug)]
pub struct RateLimitError {
    pub retry_after_seconds: u64,
    pub limit: usize,
    pub remaining: usize,
    pub key_type: String,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": "Too many requests",
            "message": format!("Rate limit exceeded for {}. Please retry after {} seconds", self.key_type, self.retry_after_seconds),
            "retry_after": self.retry_after_seconds,
            "limit": self.limit,
            "remaining": self.remaining,
            "limit_type": self.key_type,
        }));

        let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
        
        response.headers_mut().insert(
            "x-ratelimit-limit",
            self.limit.to_string().parse().unwrap(),
        );
        response.headers_mut().insert(
            "x-ratelimit-remaining",
            self.remaining.to_string().parse().unwrap(),
        );
        response.headers_mut().insert(
            "retry-after",
            self.retry_after_seconds.to_string().parse().unwrap(),
        );
        
        response
    }
}

pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, RateLimitError> {
    let ip = addr.ip();
    
    tracing::debug!("Rate limit middleware called for IP: {}", ip);
    
    let rate_limit_key = if limiter.config.enable_user_based_limits {
        if let Some(auth_user) = request.extensions().get::<AuthUser>() {
            RateLimitKey::User(auth_user.user_id)
        } else {
            RateLimitKey::Ip(ip)
        }
    } else {
        RateLimitKey::Ip(ip)
    };
    
    tracing::debug!("Using rate limit key: {:?}", rate_limit_key);
    
    if let Err(rate_limit_error) = limiter.check(rate_limit_key.clone()) {
        tracing::warn!("Rate limit exceeded, returning 429");
        return Err(rate_limit_error);
    }
    
    let (used, remaining) = limiter.get_current_usage(&rate_limit_key);
    let limit = limiter.get_limit_for_key(&rate_limit_key);
    
    request.headers_mut().insert(
        "x-ratelimit-used",
        used.to_string().parse().unwrap(),
    );
    
    let mut response = next.run(request).await;
    
    response.headers_mut().insert(
        "x-ratelimit-limit",
        limit.to_string().parse().unwrap(),
    );
    response.headers_mut().insert(
        "x-ratelimit-remaining",
        remaining.to_string().parse().unwrap(),
    );
    response.headers_mut().insert(
        "x-ratelimit-type",
        limiter.get_key_type(&rate_limit_key).parse().unwrap(),
    );
    
    Ok(response)
}
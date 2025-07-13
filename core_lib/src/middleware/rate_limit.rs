//! Rate limiting middleware

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
};
use serde_json::json;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    pub fn check(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let now = Instant::now();
        let mut requests = self.requests.lock();
        
        let entries = requests.entry(ip).or_insert_with(Vec::new);
        
        entries.retain(|&instant| now.duration_since(instant) < self.window);
        
        if entries.len() >= self.max_requests {
            let oldest = entries.first().copied().unwrap_or(now);
            let reset_in = self.window.saturating_sub(now.duration_since(oldest));
            
            return Err(RateLimitError {
                retry_after_seconds: reset_in.as_secs(),
                limit: self.max_requests,
                remaining: 0,
            });
        }
        
        entries.push(now);
        
        Ok(())
    }

    pub fn get_current_usage(&self, ip: IpAddr) -> (usize, usize) {
        let now = Instant::now();
        let mut requests = self.requests.lock();
        
        let entries = requests.entry(ip).or_insert_with(Vec::new);
        entries.retain(|&instant| now.duration_since(instant) < self.window);
        
        let used = entries.len();
        let remaining = self.max_requests.saturating_sub(used);
        
        (used, remaining)
    }
}

#[derive(Debug)]
pub struct RateLimitError {
    pub retry_after_seconds: u64,
    pub limit: usize,
    pub remaining: usize,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": "Too many requests",
            "message": format!("Rate limit exceeded. Please retry after {} seconds", self.retry_after_seconds),
            "retry_after": self.retry_after_seconds,
            "limit": self.limit,
            "remaining": self.remaining,
        }));

        let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
        
        response.headers_mut().insert(
            "X-RateLimit-Limit",
            self.limit.to_string().parse().unwrap(),
        );
        response.headers_mut().insert(
            "X-RateLimit-Remaining",
            self.remaining.to_string().parse().unwrap(),
        );
        response.headers_mut().insert(
            "Retry-After",
            self.retry_after_seconds.to_string().parse().unwrap(),
        );
        
        response
    }
}

pub async fn rate_limit_middleware<B>(
    State(limiter): State<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, RateLimitError> {
    let ip = addr.ip();
    
    limiter.check(ip)?;
    
    let (used, remaining) = limiter.get_current_usage(ip);
    request.headers_mut().insert(
        "X-RateLimit-Used",
        used.to_string().parse().unwrap(),
    );
    
    let mut response = next.run(request).await;
    
    response.headers_mut().insert(
        "X-RateLimit-Limit",
        limiter.max_requests.to_string().parse().unwrap(),
    );
    response.headers_mut().insert(
        "X-RateLimit-Remaining",
        remaining.to_string().parse().unwrap(),
    );
    
    Ok(response)
}
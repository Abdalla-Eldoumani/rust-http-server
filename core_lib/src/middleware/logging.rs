//! Request logging middleware configuration

use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, info_span, Instrument};

pub async fn log_request(
    req: Request<Body>,
    next: Next,
) -> Result<Response, std::convert::Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let version = req.version();
    
    let span = info_span!(
        "http_request",
        method = %method,
        uri = %uri,
        version = ?version,
    );
    
    let start = Instant::now();
    
    let response = next.run(req).instrument(span.clone()).await;
    
    let latency = start.elapsed();
    let status = response.status();
    
    span.in_scope(|| {
        if status.is_success() {
            info!(
                status = status.as_u16(),
                latency_ms = latency.as_millis(),
                "request completed successfully"
            );
        } else if status.is_client_error() {
            info!(
                status = status.as_u16(),
                latency_ms = latency.as_millis(),
                "client error"
            );
        } else if status.is_server_error() {
            info!(
                status = status.as_u16(),
                latency_ms = latency.as_millis(),
                "server error"
            );
        }
    });
    
    Ok(response)
}
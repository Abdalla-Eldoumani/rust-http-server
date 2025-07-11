//! Request logging middleware configuration

use axum::{
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};
use std::time::Instant;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span, Instrument};

pub fn logging_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
> {
    TraceLayer::new_for_http()
}

pub fn custom_logging_middleware() -> axum::middleware::FromFnLayer<
    impl Fn(Request<Body>, Next) -> impl std::future::Future<Output = Result<Response, std::convert::Infallible>> + Clone + Send,
> {
    middleware::from_fn(log_request)
}

async fn log_request(
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
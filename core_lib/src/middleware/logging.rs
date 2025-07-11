//! Request logging middleware configuration

use tower_http::trace::TraceLayer;
use tracing::info_span;
use http::Request;
use std::time::Duration;

pub fn logging_layer() -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
        .make_span_with(|request: &Request<_>| {
            info_span!(
                "http_request",
                method = %request.method(),
                path = %request.uri().path(),
                query = ?request.uri().query(),
                version = ?request.version(),
            )
        })
        .on_request(|request: &Request<_>, _span: &tracing::Span| {
            tracing::info!(
                "started processing request {} {}",
                request.method(),
                request.uri().path()
            );
        })
        .on_response(|response: &http::Response<_>, latency: Duration, _span: &tracing::Span| {
            let status = response.status();
            let latency_ms = latency.as_millis();
            
            if status.is_success() {
                tracing::info!(
                    status = status.as_u16(),
                    latency_ms = latency_ms,
                    "request completed successfully"
                );
            } else if status.is_client_error() {
                tracing::warn!(
                    status = status.as_u16(),
                    latency_ms = latency_ms,
                    "client error response"
                );
            } else {
                tracing::error!(
                    status = status.as_u16(),
                    latency_ms = latency_ms,
                    "server error response"
                );
            }
        })
        .on_failure(
            |error: tower_http::classify::ServerErrorsFailureClass,
             latency: Duration,
             _span: &tracing::Span| {
                tracing::error!(
                    latency_ms = latency.as_millis(),
                    error = ?error,
                    "request failed"
                );
            },
        )
}

#[allow(dead_code)]
pub async fn simple_logger(
    req: Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, std::convert::Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = std::time::Instant::now();

    let response = next.run(req).await;
    
    let latency = start.elapsed();
    tracing::info!(
        method = %method,
        path = %uri.path(),
        status = response.status().as_u16(),
        latency_ms = latency.as_millis(),
        "request processed"
    );

    Ok(response)
}
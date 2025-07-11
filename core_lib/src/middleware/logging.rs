//! Request logging middleware configuration

use tower_http::trace::{self, TraceLayer};
use tracing::{info_span, Level, Span};
use axum::http::Request;
use std::time::Duration;

pub fn logging_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    impl Fn(&Request<()>) -> Span + Clone,
    impl Fn(&Request<()>, &Duration) + Clone,
    impl Fn(&trace::OnResponse<'_>, &Duration) + Clone,
    impl Fn(&trace::OnFailure<'_>, &Duration) + Clone,
> {
    TraceLayer::new_for_http()
        .make_span_with(|request: &Request<()>| {
            info_span!(
                "http_request",
                method = %request.method(),
                path = %request.uri().path(),
                query = ?request.uri().query(),
                version = ?request.version(),
            )
        })
        .on_request(|request: &Request<()>, _span: &Span| {
            tracing::info!(
                "started processing request {} {}",
                request.method(),
                request.uri().path()
            );
        })
        .on_response(|response: &trace::OnResponse<'_>, latency: Duration, _span: &Span| {
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
        .on_failure(|failure: &trace::OnFailure<'_>, latency: Duration, _span: &Span| {
            tracing::error!(
                latency_ms = latency.as_millis(),
                error = %failure,
                "request failed"
            );
        })
}

#[allow(dead_code)]
pub fn simple_logger<B>(
    req: Request<B>,
    next: axum::middleware::Next<B>,
) -> impl std::future::Future<Output = Result<axum::response::Response, std::convert::Infallible>> {
    async move {
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
}
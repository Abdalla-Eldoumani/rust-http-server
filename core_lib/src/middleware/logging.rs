//! Request logging middleware configuration

use tower_http::trace::TraceLayer;
use tracing::info_span;

pub fn logging_layer() -> TraceLayer {
    TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<_>| {
            info_span!(
                "http_request",
                method = %request.method(),
                path = %request.uri().path(),
                query = ?request.uri().query(),
                version = ?request.version(),
            )
        })
}

#[allow(dead_code)]
pub async fn simple_logger(
    req: axum::http::Request<axum::body::Body>,
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
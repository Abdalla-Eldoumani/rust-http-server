//! Request logging middleware configuration

use crate::config::LoggingConfig;
use crate::middleware::auth::AuthUser;
use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};

use std::time::Instant;
use tracing::{info, warn, error, info_span, Instrument};
use uuid::Uuid;

pub fn log_request_with_config(
    config: LoggingConfig,
) -> impl Fn(Request<Body>, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, std::convert::Infallible>> + Send>> + Clone {
    move |mut req: Request<Body>, next: Next| {
        let config = config.clone();
        Box::pin(async move {
            let method = req.method().clone();
            let uri = req.uri().clone();
            let version = req.version();
            let user_agent = req.headers()
                .get("user-agent")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("unknown")
                .to_string();
            
            let request_id = if config.include_request_id {
                let id = Uuid::new_v4().to_string();
                req.headers_mut().insert(
                    "x-request-id",
                    id.parse().unwrap(),
                );
                Some(id)
            } else {
                None
            };
            
            let user_info = if config.include_user_info {
                req.extensions().get::<AuthUser>().map(|user| {
                    (user.user_id, user.username.clone(), user.role.clone())
                })
            } else {
                None
            };
            
            let mut span_fields = vec![
                ("method", method.to_string()),
                ("uri", uri.to_string()),
                ("version", format!("{:?}", version)),
                ("user_agent", user_agent.clone()),
            ];
            
            if let Some(ref req_id) = request_id {
                span_fields.push(("request_id", req_id.clone()));
            }
            
            if let Some((user_id, username, role)) = &user_info {
                span_fields.push(("user_id", user_id.to_string()));
                span_fields.push(("username", username.clone()));
                span_fields.push(("user_role", format!("{:?}", role)));
            }
            
            let span = info_span!(
                "http_request",
                method = %method,
                uri = %uri,
                version = ?version,
                user_agent = %user_agent,
                request_id = request_id.as_deref().unwrap_or(""),
                user_id = user_info.as_ref().map(|(id, _, _)| id.to_string()).unwrap_or_default(),
                username = user_info.as_ref().map(|(_, name, _)| name.clone()).unwrap_or_default(),
                user_role = user_info.as_ref().map(|(_, _, role)| format!("{:?}", role)).unwrap_or_default(),
            );
            
            let start = Instant::now();
            
            let mut response = next.run(req).instrument(span.clone()).await;
            
            let latency = start.elapsed();
            let status = response.status();
            
            if let Some(req_id) = request_id {
                response.headers_mut().insert(
                    "x-request-id",
                    req_id.parse().unwrap(),
                );
            }
            
            if config.include_timing {
                response.headers_mut().insert(
                    "x-response-time",
                    format!("{}ms", latency.as_millis()).parse().unwrap(),
                );
            }
            
            span.in_scope(|| {
                let _base_fields = [
                    ("status", status.as_u16().to_string()),
                    ("latency_ms", latency.as_millis().to_string()),
                ];
                
                if status.is_success() {
                    info!(
                        status = status.as_u16(),
                        latency_ms = latency.as_millis(),
                        "request completed successfully"
                    );
                } else if status.is_client_error() {
                    warn!(
                        status = status.as_u16(),
                        latency_ms = latency.as_millis(),
                        "client error"
                    );
                } else if status.is_server_error() {
                    error!(
                        status = status.as_u16(),
                        latency_ms = latency.as_millis(),
                        "server error"
                    );
                }
            });
            
            Ok(response)
        })
    }
}

pub async fn log_request(
    req: Request<Body>,
    next: Next,
) -> Result<Response, std::convert::Infallible> {
    let config = LoggingConfig::default();
    log_request_with_config(config)(req, next).await
}
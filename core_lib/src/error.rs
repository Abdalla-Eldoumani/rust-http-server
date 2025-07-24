//! Application error types and handling

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error")]
    InternalServerError,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Job processing error: {0}")]
    Job(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("File validation error: {0}")]
    FileValidation(String),

    #[error("Security validation error: {0}")]
    SecurityValidation(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Middleware error: {0}")]
    Middleware(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Authentication(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::Authorization(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::InternalServerError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
            }
            AppError::WebSocket(msg) => {
                tracing::error!("WebSocket error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "WebSocket error".to_string())
            }
            AppError::Job(msg) => {
                tracing::error!("Job processing error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Job processing error".to_string())
            }
            AppError::IoError(err) => {
                tracing::error!("IO error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::JsonError(err) => {
                tracing::error!("JSON error: {:?}", err);
                (StatusCode::BAD_REQUEST, "Invalid JSON data".to_string())
            }
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::FileValidation(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::SecurityValidation(msg) => {
                tracing::warn!("Security validation failed: {}", msg);
                (StatusCode::BAD_REQUEST, "Request failed security validation".to_string())
            }
            AppError::Cache(msg) => {
                tracing::error!("Cache error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Cache error".to_string())
            }
            AppError::Configuration(msg) => {
                tracing::error!("Configuration error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error".to_string())
            }
            AppError::RateLimit(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
            AppError::Middleware(msg) => {
                tracing::error!("Middleware error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Middleware error".to_string())
            }
            AppError::Other(err) => {
                tracing::error!("Unexpected error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found".to_string()),
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                AppError::BadRequest("Resource already exists".to_string())
            }
            _ => AppError::Database(err.to_string()),
        }
    }
}
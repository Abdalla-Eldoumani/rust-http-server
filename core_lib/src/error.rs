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

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

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
            AppError::IoError(err) => {
                tracing::error!("IO error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::JsonError(err) => {
                tracing::error!("JSON error: {:?}", err);
                (StatusCode::BAD_REQUEST, "Invalid JSON data".to_string())
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
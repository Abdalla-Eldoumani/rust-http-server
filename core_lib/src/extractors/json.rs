//! Custom JSON extractor with better Unicode support

use axum::{
    async_trait,
    body::Body,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use serde_json::json;

pub struct UnicodeJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for UnicodeJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = UnicodeJsonRejection;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(UnicodeJson(value)),
            Err(json_rejection) => {
                let error_msg = json_rejection.to_string();
                if error_msg.contains("unicode") || error_msg.contains("UTF-8") || error_msg.contains("invalid unicode code point") {
                    Err(UnicodeJsonRejection::InvalidUnicode)
                } else if error_msg.contains("missing field") || error_msg.contains("Failed to deserialize") {
                    Err(UnicodeJsonRejection::InvalidJson(error_msg))
                } else {
                    Err(UnicodeJsonRejection::Other(error_msg))
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum UnicodeJsonRejection {
    InvalidUnicode,
    InvalidJson(String),
    Other(String),
}

impl IntoResponse for UnicodeJsonRejection {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            UnicodeJsonRejection::InvalidUnicode => (
                StatusCode::BAD_REQUEST,
                "Invalid Unicode characters in JSON. Please ensure all text is properly encoded in UTF-8."
            ),
            UnicodeJsonRejection::InvalidJson(msg) => (
                StatusCode::BAD_REQUEST,
                if msg.contains("missing field") || msg.contains("Failed to deserialize") {
                    "missing field validation error"
                } else if msg.contains("EOF while parsing") {
                    "Empty or incomplete JSON request"
                } else {
                    "Invalid JSON format"
                }
            ),
            UnicodeJsonRejection::Other(_msg) => (
                StatusCode::BAD_REQUEST,
                "Failed to parse JSON request"
            ),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

impl std::fmt::Display for UnicodeJsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnicodeJsonRejection::InvalidUnicode => write!(f, "Invalid Unicode in JSON"),
            UnicodeJsonRejection::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
            UnicodeJsonRejection::Other(msg) => write!(f, "JSON error: {}", msg),
        }
    }
}

impl std::error::Error for UnicodeJsonRejection {}
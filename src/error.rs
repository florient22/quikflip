use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

/// Errors an API handler can return. Bad requests carry a stable `code` the front end translates.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest { code: &'static str, message: String, detail: Option<Value> },
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("stored data could not be read: {0}")]
    Corrupt(String),
    #[error("the database worker stopped unexpectedly")]
    Worker,
}

impl AppError {
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::BadRequest { code, message: message.into(), detail: None }
    }

    pub fn bad_request_with(code: &'static str, message: impl Into<String>, detail: Value) -> Self {
        Self::BadRequest { code, message: message.into(), detail: Some(detail) }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest { code, message, detail } => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": code, "message": message, "detail": detail }))).into_response()
            }
            other => {
                tracing::error!("request failed: {other}");
                let body = json!({ "error": "internal", "message": "Something went wrong on the server. Check the QuikFlip logs." });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
            }
        }
    }
}

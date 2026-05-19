use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("{message}")]
    Conflict { code: String, message: String },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
    #[error("{message}")]
    Unrecoverable {
        status: StatusCode,
        code: String,
        message: String,
    },
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Unrecoverable { status, .. } => *status,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self.to_string();
        if status.is_server_error() {
            tracing::error!(error = %message, "request failed");
        }
        let body = match &self {
            AppError::Unrecoverable { code, .. } | AppError::Conflict { code, .. } => {
                json!({ "error": message, "code": code })
            }
            _ => json!({ "error": message }),
        };
        (status, Json(body)).into_response()
    }
}

impl From<tokio_rusqlite::Error> for AppError {
    fn from(e: tokio_rusqlite::Error) -> Self {
        AppError::Internal(anyhow::anyhow!(e))
    }
}

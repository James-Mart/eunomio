// SPDX-License-Identifier: Apache-2.0

use axum::{
    response::{IntoResponse, Response},
    Json,
};
use eunomio_core::AppError;
use serde_json::json;

pub struct ServerError(pub AppError);

impl From<AppError> for ServerError {
    fn from(e: AppError) -> Self {
        ServerError(e)
    }
}

pub type ApiResult<T> = Result<T, ServerError>;

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.0.status();
        let message = self.0.to_string();
        if status.is_server_error() {
            tracing::error!(error = %message, "request failed");
        }
        let body = match &self.0 {
            AppError::Unauthorized => json!({ "error": message, "code": "unauthenticated" }),
            AppError::CsrfRejected => json!({ "error": message, "code": "csrf_rejected" }),
            AppError::Unrecoverable { code, .. } | AppError::Conflict { code, .. } => {
                json!({ "error": message, "code": code })
            }
            AppError::Blocked { run_id, reason } => json!({
                "error": message,
                "code": "construct_blocked",
                "runId": run_id,
                "reason": reason,
            }),
            _ => json!({ "error": message }),
        };
        (status, Json(body)).into_response()
    }
}

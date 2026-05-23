// SPDX-License-Identifier: Apache-2.0

use http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("unauthenticated")]
    Unauthorized,
    #[error("csrf rejected")]
    CsrfRejected,
    #[error("{message}")]
    Conflict { code: String, message: String },
    #[error("construct blocked: {reason}")]
    Blocked { run_id: String, reason: String },
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
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::CsrfRejected => StatusCode::FORBIDDEN,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::Blocked { .. } => StatusCode::CONFLICT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Unrecoverable { status, .. } => *status,
        }
    }
}

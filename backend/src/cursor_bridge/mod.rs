use crate::error::AppError;
use axum::http::StatusCode;

mod fake;
mod fold;
mod helper_assets;
mod list_models;
mod runner;
mod wire;

pub use fake::FakeSubagentRunner;
pub use fold::fold_sdk_event;
pub use list_models::list_models;
pub use runner::{CursorHelperRunner, HelperEvent, RunHandle, RunRequest, SubagentRunner};

pub(crate) fn unavailable(message: &str) -> AppError {
    AppError::Unrecoverable {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "cursor_sdk_unavailable".into(),
        message: message.into(),
    }
}

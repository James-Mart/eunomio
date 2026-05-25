// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use eunomio_core::AppError;

pub use eunomio_helper_protocol::{
    HelperEvent, ListModelsRequest, ListModelsResponse, RunHandle, RunRequest, SubagentRunner,
};

mod fake;
mod fold;
mod helper_assets;
mod helper_stdio;
mod runner;
mod wire;

pub use fake::FakeSubagentRunner;
pub use fold::fold_sdk_event;
pub use runner::CursorHelperRunner;

pub(crate) fn unavailable(message: &str) -> AppError {
    AppError::Unrecoverable {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "cursor_sdk_unavailable".into(),
        message: message.into(),
    }
}

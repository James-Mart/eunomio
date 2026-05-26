// SPDX-License-Identifier: Apache-2.0

use eunomio_core::{traits::quota::TokenUsage, types::CursorModel, AppError};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum HelperEvent {
    Started {
        run_id: String,
        agent_id: String,
    },
    SdkMessage {
        run_id: String,
        message: serde_json::Value,
    },
    UsageReported {
        run_id: String,
        usage: TokenUsage,
    },
    Finished {
        run_id: String,
        result: String,
        duration_ms: Option<u64>,
    },
    Error {
        run_id: String,
        code: String,
        message: String,
    },
    Cancelled {
        run_id: String,
    },
}

pub struct RunHandle {
    pub cancel: Box<dyn Fn() + Send + Sync>,
}

#[async_trait::async_trait]
pub trait SubagentRunner: Send + Sync {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError>;

    async fn list_models(&self, cursor_api_key: &str) -> Result<Vec<CursorModel>, AppError>;
}

impl HelperEvent {
    /// Re-stamp a previously-built `HelperEvent` with a new `run_id`.
    pub fn with_run_id(self, run_id: String) -> Self {
        match self {
            HelperEvent::Started { agent_id, .. } => HelperEvent::Started { run_id, agent_id },
            HelperEvent::SdkMessage { message, .. } => HelperEvent::SdkMessage { run_id, message },
            HelperEvent::UsageReported { usage, .. } => {
                HelperEvent::UsageReported { run_id, usage }
            }
            HelperEvent::Finished {
                result,
                duration_ms,
                ..
            } => HelperEvent::Finished {
                run_id,
                result,
                duration_ms,
            },
            HelperEvent::Error { code, message, .. } => HelperEvent::Error {
                run_id,
                code,
                message,
            },
            HelperEvent::Cancelled { .. } => HelperEvent::Cancelled { run_id },
        }
    }
}

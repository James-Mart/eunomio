// SPDX-License-Identifier: Apache-2.0

use super::partition::{PhaseName, PhaseState};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SseEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        session_id: String,
        target_node_id: String,
        partition_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Phase {
        session_id: String,
        target_node_id: String,
        partition_id: String,
        name: PhaseName,
        state: PhaseState,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
    },
    #[serde(rename_all = "camelCase")]
    TranscriptDelta {
        session_id: String,
        target_node_id: String,
        partition_id: String,
        run_id: String,
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Finished {
        session_id: String,
        target_node_id: String,
        partition_id: String,
    },
    #[serde(rename_all = "camelCase")]
    ShavingReady {
        session_id: String,
        target_node_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Cancelled {
        session_id: String,
        target_node_id: String,
        partition_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        session_id: String,
        target_node_id: String,
        partition_id: String,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelStateName {
    Idle,
    Running,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub enabled: bool,
    pub state: TunnelStateName,
    pub token_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

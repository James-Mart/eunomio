use super::partition::{PhaseName, PhaseState};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SseEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        session_id: String,
        target_node_id: String,
        partition_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    Phase {
        session_id: String,
        target_node_id: String,
        partition_id: i64,
        name: PhaseName,
        state: PhaseState,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
    },
    #[serde(rename_all = "camelCase")]
    SdkMessage {
        session_id: String,
        target_node_id: String,
        partition_id: i64,
        message: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    Finished {
        session_id: String,
        target_node_id: String,
        partition_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    Cancelled {
        session_id: String,
        target_node_id: String,
        partition_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        session_id: String,
        target_node_id: String,
        partition_id: i64,
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

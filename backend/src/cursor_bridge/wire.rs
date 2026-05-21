use super::runner::HelperEvent;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum HelperWireEvent {
    #[serde(rename_all = "camelCase")]
    Started { agent_id: String },
    #[serde(rename_all = "camelCase")]
    SdkMessage { message: serde_json::Value },
    #[serde(rename_all = "camelCase")]
    Finished {
        result: String,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Error { code: String, message: String },
    #[serde(rename_all = "camelCase")]
    Cancelled,
}

impl HelperWireEvent {
    pub(super) fn into_helper_event(self, run_id: i64) -> HelperEvent {
        match self {
            HelperWireEvent::Started { agent_id } => HelperEvent::Started { run_id, agent_id },
            HelperWireEvent::SdkMessage { message } => HelperEvent::SdkMessage { run_id, message },
            HelperWireEvent::Finished {
                result,
                duration_ms,
            } => HelperEvent::Finished {
                run_id,
                result,
                duration_ms,
            },
            HelperWireEvent::Error { code, message } => HelperEvent::Error {
                run_id,
                code,
                message,
            },
            HelperWireEvent::Cancelled => HelperEvent::Cancelled { run_id },
        }
    }
}

impl HelperEvent {
    /// Re-stamp a previously-built `HelperEvent` with a new `run_id`. Used by
    /// the fake runner's scripted events (which carry placeholder ids) to
    /// match the live run, and indirectly by the real runner's wire mapping.
    pub fn with_run_id(self, run_id: i64) -> Self {
        match self {
            HelperEvent::Started { agent_id, .. } => HelperEvent::Started { run_id, agent_id },
            HelperEvent::SdkMessage { message, .. } => HelperEvent::SdkMessage { run_id, message },
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

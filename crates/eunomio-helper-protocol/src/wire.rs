// SPDX-License-Identifier: Apache-2.0

use crate::{runner::HelperEvent, usage::parse_turn_ended_usage};
use eunomio_core::traits::quota::TokenUsage;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HelperWireEvent {
    #[serde(rename_all = "camelCase")]
    Started { agent_id: String },
    #[serde(rename_all = "camelCase")]
    SdkMessage { message: serde_json::Value },
    #[serde(rename_all = "camelCase")]
    TurnEnded { usage: TokenUsage },
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
    pub fn into_helper_event(self, run_id: String) -> HelperEvent {
        match self {
            HelperWireEvent::Started { agent_id } => HelperEvent::Started { run_id, agent_id },
            HelperWireEvent::SdkMessage { message } => {
                if let Some(usage) = parse_turn_ended_usage(&message) {
                    HelperEvent::UsageReported { run_id, usage }
                } else {
                    HelperEvent::SdkMessage { run_id, message }
                }
            }
            HelperWireEvent::TurnEnded { usage } => HelperEvent::UsageReported { run_id, usage },
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

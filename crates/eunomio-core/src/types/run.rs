// SPDX-License-Identifier: Apache-2.0

use super::partition::PartitionStrategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    #[default]
    Plan,
    Construct,
}

impl RunKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunKind::Plan => "plan",
            RunKind::Construct => "construct",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "plan" => Some(RunKind::Plan),
            "construct" => Some(RunKind::Construct),
            _ => None,
        }
    }

    pub fn phase(&self) -> super::partition::PhaseName {
        match self {
            RunKind::Plan => super::partition::PhaseName::Plan,
            RunKind::Construct => super::partition::PhaseName::Construct,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Finished,
    Error,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
            RunStatus::Error => "error",
            RunStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(RunStatus::Running),
            "finished" => Some(RunStatus::Finished),
            "error" => Some(RunStatus::Error),
            "cancelled" => Some(RunStatus::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StartRunRequest {
    pub kind: RunKind,
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub user_feedback: Option<String>,
    #[serde(default)]
    pub strategy_override: Option<PartitionStrategy>,
    #[serde(default)]
    pub prompt_override: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptPlanRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub partition_id: String,
    pub kind: RunKind,
    pub status: RunStatus,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: String,
    pub partition_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub kind: RunKind,
    pub parent_run_id: Option<String>,
    pub status: RunStatus,
    pub result_json: Option<String>,
    pub result_text: Option<String>,
    pub error_message: Option<String>,
    pub transcript_text: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

impl From<RunRow> for Run {
    fn from(row: RunRow) -> Self {
        let result = row
            .result_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        Self {
            id: row.id,
            partition_id: row.partition_id,
            kind: row.kind,
            status: row.status,
            result,
            error_message: row.error_message,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub run_id: String,
    pub kind: RunKind,
    pub prompt: Option<String>,
    pub transcript_text: Option<String>,
    pub raw_result: Option<String>,
    pub parsed_result: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

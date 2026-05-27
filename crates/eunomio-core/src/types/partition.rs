// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PartitionStrategy {
    Synthetic,
    Vertical,
    Horizontal,
}

impl PartitionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionStrategy::Synthetic => "synthetic",
            PartitionStrategy::Vertical => "vertical",
            PartitionStrategy::Horizontal => "horizontal",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "synthetic" => Some(PartitionStrategy::Synthetic),
            "vertical" => Some(PartitionStrategy::Vertical),
            "horizontal" => Some(PartitionStrategy::Horizontal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PhaseName {
    Plan,
    Construct,
}

impl PhaseName {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseName::Plan => "plan",
            PhaseName::Construct => "construct",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "plan" => Some(PhaseName::Plan),
            "construct" => Some(PhaseName::Construct),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseState {
    Running,
    AwaitingReview,
    Error,
}

impl PhaseState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseState::Running => "running",
            PhaseState::AwaitingReview => "awaiting_review",
            PhaseState::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(PhaseState::Running),
            "awaiting_review" => Some(PhaseState::AwaitingReview),
            "error" => Some(PhaseState::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    pub id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub strategy: Option<PartitionStrategy>,
    pub plan: Option<serde_json::Value>,
    pub phase: PhaseName,
    pub phase_state: PhaseState,
    pub candidate_slice_tree_sha: Option<String>,
    pub candidate_slice_commit_sha: Option<String>,
    pub remaining_depth: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct PartitionRow {
    pub id: String,
    pub org_id: String,
    pub user_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub strategy: Option<PartitionStrategy>,
    pub plan_json: Option<String>,
    pub candidate_slice_tree_sha: Option<String>,
    pub candidate_slice_commit_sha: Option<String>,
    pub phase: PhaseName,
    pub phase_state: PhaseState,
    pub worktree_path: String,
    pub remaining_depth: Option<i64>,
    pub created_at: i64,
}

impl From<PartitionRow> for Partition {
    fn from(row: PartitionRow) -> Self {
        let plan = row
            .plan_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        Self {
            id: row.id,
            session_id: row.session_id,
            target_node_id: row.target_node_id,
            strategy: row.strategy,
            plan,
            phase: row.phase,
            phase_state: row.phase_state,
            candidate_slice_tree_sha: row.candidate_slice_tree_sha,
            candidate_slice_commit_sha: row.candidate_slice_commit_sha,
            remaining_depth: row.remaining_depth,
            created_at: row.created_at,
        }
    }
}

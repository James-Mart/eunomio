use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub base_ref: String,
    pub source_ref: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub id: String,
    pub base_ref: String,
    pub source_ref: String,
    pub base_node_id: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDto {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub tree_sha: String,
    pub commit_sha: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdgeDto {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDto {
    pub nodes: Vec<NodeDto>,
    pub edges: Vec<GraphEdgeDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeDto {
    pub target_node_id: String,
    pub parent_node_id: Option<String>,
    pub diff: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffDto {
    pub from_tree: String,
    pub to_tree: String,
    pub diff: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameNodeRequest {
    pub title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchFromNodeRequest {
    pub branch_name: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchFromNodeResponse {
    pub branch_name: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionSettings {
    #[serde(default)]
    pub coordinator: CoordinatorSettings,
    #[serde(default)]
    pub surveyor: SubagentSettings,
    #[serde(default)]
    pub planner: SubagentSettings,
    #[serde(default)]
    pub constructor: SubagentSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorSettings {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub human_in_the_loop: HumanInTheLoop,
}

impl Default for CoordinatorSettings {
    fn default() -> Self {
        Self {
            model: default_model(),
            human_in_the_loop: HumanInTheLoop::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanInTheLoop {
    #[serde(default = "default_true")]
    pub after_survey: bool,
    #[serde(default = "default_true")]
    pub after_planning: bool,
    #[serde(default = "default_true")]
    pub after_construct: bool,
}

impl Default for HumanInTheLoop {
    fn default() -> Self {
        Self {
            after_survey: true,
            after_planning: true,
            after_construct: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSettings {
    #[serde(default)]
    pub override_model: bool,
    #[serde(default = "default_model")]
    pub model: String,
}

impl Default for SubagentSettings {
    fn default() -> Self {
        Self {
            override_model: false,
            model: default_model(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_model() -> String {
    "composer-2".to_string()
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PartitionSettingsPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<CoordinatorSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surveyor: Option<SubagentSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner: Option<SubagentSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constructor: Option<SubagentSettings>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PartitionStrategy {
    Semantic,
    Vertical,
    Horizontal,
}

impl PartitionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionStrategy::Semantic => "semantic",
            PartitionStrategy::Vertical => "vertical",
            PartitionStrategy::Horizontal => "horizontal",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PhaseName {
    Survey,
    Plan,
    Construct,
}

impl PhaseName {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseName::Survey => "survey",
            PhaseName::Plan => "plan",
            PhaseName::Construct => "construct",
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
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    #[default]
    Survey,
    Plan,
    Construct,
}

impl RunKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunKind::Survey => "survey",
            RunKind::Plan => "plan",
            RunKind::Construct => "construct",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "survey" => Some(RunKind::Survey),
            "plan" => Some(RunKind::Plan),
            "construct" => Some(RunKind::Construct),
            _ => None,
        }
    }

    pub fn phase(&self) -> PhaseName {
        match self {
            RunKind::Survey => PhaseName::Survey,
            RunKind::Plan => PhaseName::Plan,
            RunKind::Construct => PhaseName::Construct,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    pub id: i64,
    pub session_id: String,
    pub target_node_id: String,
    pub strategy: Option<PartitionStrategy>,
    pub change_survey: Option<serde_json::Value>,
    pub plan: Option<serde_json::Value>,
    pub phase: PhaseName,
    pub phase_state: PhaseState,
    pub candidate_slice_tree_sha: Option<String>,
    pub candidate_slice_commit_sha: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct PartitionRow {
    pub id: i64,
    pub session_id: String,
    pub target_node_id: String,
    pub strategy: Option<PartitionStrategy>,
    pub change_survey_json: Option<String>,
    pub plan_json: Option<String>,
    pub candidate_slice_tree_sha: Option<String>,
    pub candidate_slice_commit_sha: Option<String>,
    pub phase: PhaseName,
    pub phase_state: PhaseState,
    pub worktree_path: String,
    pub created_at: i64,
}

impl From<PartitionRow> for Partition {
    fn from(row: PartitionRow) -> Self {
        let change_survey = row
            .change_survey_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        let plan = row
            .plan_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        Self {
            id: row.id,
            session_id: row.session_id,
            target_node_id: row.target_node_id,
            strategy: row.strategy,
            change_survey,
            plan,
            phase: row.phase,
            phase_state: row.phase_state,
            candidate_slice_tree_sha: row.candidate_slice_tree_sha,
            candidate_slice_commit_sha: row.candidate_slice_commit_sha,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StartRunRequest {
    pub kind: RunKind,
    #[serde(default)]
    pub parent_run_id: Option<i64>,
    #[serde(default)]
    pub user_feedback: Option<String>,
    #[serde(default)]
    pub strategy_override: Option<PartitionStrategy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptSurveyRequest {
    pub run_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptPlanRequest {
    pub run_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: i64,
    pub partition_id: i64,
    pub kind: RunKind,
    pub status: RunStatus,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: i64,
    pub partition_id: i64,
    pub session_id: String,
    pub target_node_id: String,
    pub kind: RunKind,
    pub parent_run_id: Option<i64>,
    pub status: RunStatus,
    pub result_json: Option<String>,
    pub result_text: Option<String>,
    pub error_message: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorModelDto {
    pub id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorModelsDto {
    pub models: Vec<CursorModelDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfoDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<String>,
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
pub struct TunnelStatusDto {
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

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
    pub is_favorite: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionSettingsDto {
    #[serde(default)]
    pub coordinator: CoordinatorSettingsDto,
    #[serde(default = "default_surveyor")]
    pub surveyor: SurveyorSettingsDto,
    #[serde(default)]
    pub planner: serde_json::Value,
    #[serde(default)]
    pub constructor: serde_json::Value,
}

impl Default for PartitionSettingsDto {
    fn default() -> Self {
        Self {
            coordinator: CoordinatorSettingsDto::default(),
            surveyor: default_surveyor(),
            planner: serde_json::Value::Null,
            constructor: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorSettingsDto {
    #[serde(default)]
    pub human_in_the_loop: HumanInTheLoopDto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HumanInTheLoopDto {
    #[serde(default)]
    pub after_survey: bool,
    #[serde(default)]
    pub after_planning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurveyorSettingsDto {
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_surveyor() -> SurveyorSettingsDto {
    SurveyorSettingsDto {
        model: default_model(),
    }
}

fn default_model() -> String {
    "composer-2".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionSettingsPatchDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<CoordinatorSettingsDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surveyor: Option<SurveyorSettingsDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constructor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PartitionStrategy {
    Semantic,
    Vertical,
    Horizontal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginMockPartitionRequest {
    pub strategy: PartitionStrategy,
    #[serde(default)]
    pub user_concern: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RerunMockPartitionRequest {
    #[serde(default)]
    pub user_feedback: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockPartitionDto {
    pub session_id: String,
    pub target_node_id: String,
    pub strategy: PartitionStrategy,
    pub user_concern: Option<String>,
    pub started_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PhaseName {
    Survey,
    Plan,
    Construct,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseState {
    Pending,
    Running,
    AwaitingReview,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SseEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        session_id: String,
        target_node_id: String,
        strategy: PartitionStrategy,
        user_concern: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Phase {
        session_id: String,
        target_node_id: String,
        name: PhaseName,
        state: PhaseState,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
    },
    #[serde(rename_all = "camelCase")]
    SdkMessage {
        session_id: String,
        target_node_id: String,
        message: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    LoopProgress {
        session_id: String,
        target_node_id: String,
        item_id: String,
        status: String,
    },
    #[serde(rename_all = "camelCase")]
    Finished {
        session_id: String,
        target_node_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Cancelled {
        session_id: String,
        target_node_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        session_id: String,
        target_node_id: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

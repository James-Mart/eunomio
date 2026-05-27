// SPDX-License-Identifier: Apache-2.0

use super::partition::PartitionStrategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBlob {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesizedRanges {
    pub child: Vec<FileLineRanges>,
    pub parent: Vec<FileLineRanges>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLineRanges {
    pub path: String,
    pub lines: Vec<LineRanges>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineRanges {
    pub line: u32,
    pub spans: Vec<(u32, u32)>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub remote_url: String,
    pub base_ref: String,
    pub source_ref: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub normalized_remote: String,
    pub literal_remote: String,
    pub is_local: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_owner: Option<String>,
    pub repo_name: String,
    pub base_ref: String,
    pub source_ref: String,
    pub base_node_id: String,
    pub created_at: i64,
    pub session_partition_complete_at: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub tree_sha: String,
    pub commit_sha: String,
    pub title: String,
    pub description: String,
    pub strategy: Option<PartitionStrategy>,
    pub has_shaving_track: bool,
    pub reviewed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderAudit {
    pub status: ReorderAuditStatus,
    pub original_order: Vec<String>,
    pub proposed_order: Vec<String>,
    pub applied_order: Vec<String>,
    pub hard_deps: Vec<ReorderRelation>,
    pub soft_prefs: Vec<ReorderRelation>,
    pub uncertain_pairs: Vec<[String; 2]>,
    pub rationale: String,
    pub fallback_reason: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReorderAuditStatus {
    Disabled,
    Applied,
    NoChange,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderRelation {
    pub before: String,
    pub after: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeViewedFiles {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetEdgeFileViewedRequest {
    pub viewed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetNodeReviewedRequest {
    pub reviewed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub target_node_id: String,
    pub parent_node_id: Option<String>,
    pub diff: String,
    pub files: Vec<FileBlob>,
    pub synthesized: SynthesizedRanges,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub from_tree: String,
    pub to_tree: String,
    pub diff: String,
    pub files: Vec<FileBlob>,
    pub synthesized: SynthesizedRanges,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffQuery {
    pub from_tree: String,
    pub to_tree: String,
    #[serde(default)]
    pub before_ref: Option<String>,
    #[serde(default)]
    pub after_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShavingStep {
    pub tree_sha: String,
    pub commit_sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShavingTrack {
    pub target_node_id: String,
    pub parent_tree_sha: String,
    pub head_tree_sha: String,
    pub steps: Vec<ShavingStep>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShavingTrackResponse {
    pub target_node_id: String,
    pub parent_tree_sha: String,
    pub head_tree_sha: String,
    pub steps: Vec<ShavingStep>,
    pub step_diffs: Vec<Diff>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListPartitionsQuery {
    #[serde(default)]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelParamValue {
    pub id: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelParamValueOption {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelParamDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub values: Vec<ModelParamValueOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelVariant {
    pub params: Vec<ModelParamValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ModelParamValue>,
}

impl Default for ModelSelection {
    fn default() -> Self {
        Self {
            id: "composer-2.5".to_string(),
            params: vec![ModelParamValue {
                id: "fast".to_string(),
                value: "true".to_string(),
            }],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CursorModel {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<ModelParamDef>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants: Option<Vec<ModelVariant>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorModels {
    pub models: Vec<CursorModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvePullRequestRequest {
    pub pull_request_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPullRequest {
    pub remote_url: String,
    pub source_ref: String,
    pub base_ref: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoHints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_remote_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_source_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_base_ref: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDefaultPrompts {
    pub planner: String,
    pub constructor: String,
    pub shaver: String,
    pub reorder: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSessionLookup {
    pub session_id: String,
}

#[cfg(test)]
mod model_selection_tests {
    use super::{ModelParamValue, ModelSelection};

    #[test]
    fn model_selection_round_trip_with_params() {
        let sel = ModelSelection {
            id: "composer-2.5".to_string(),
            params: vec![ModelParamValue {
                id: "fast".to_string(),
                value: "false".to_string(),
            }],
        };
        let json = serde_json::to_string(&sel).unwrap();
        assert!(json.contains("\"fast\""));
        let back: ModelSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "composer-2.5");
        assert_eq!(back.params[0].value, "false");
    }

    #[test]
    fn model_selection_omits_empty_params() {
        let sel = ModelSelection {
            id: "gpt-5.5".to_string(),
            params: vec![],
        };
        let json = serde_json::to_string(&sel).unwrap();
        assert!(!json.contains("params"));
    }
}

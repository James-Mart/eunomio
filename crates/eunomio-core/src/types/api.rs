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
pub struct CursorModel {
    pub id: String,
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
    pub surveyor: String,
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

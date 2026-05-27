// SPDX-License-Identifier: Apache-2.0

use crate::types::*;

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct NodeBasic {
    pub node_id: String,
    pub tree_sha: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone)]
pub struct NodeRewrite {
    pub node_id: String,
    pub parent_node_id: String,
    pub tree_sha: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone)]
pub struct SiblingInfo {
    pub id: String,
    pub target_node_id: String,
    pub worktree_path: String,
}

#[derive(Debug, Clone)]
pub struct CreatedSessionRow {
    pub id: String,
    pub base_node_id: String,
    pub created_at: i64,
}

// insert_seed_nodes returns () in the current implementation

#[derive(Debug, Clone)]
pub struct SessionRepoFields {
    pub normalized_remote: String,
    pub literal_remote: String,
}

#[derive(Debug, Clone)]
pub struct SessionRepoIdentity {
    pub org_id: String,
    pub normalized_remote: String,
}

#[derive(Debug, Clone)]
pub struct NewPartitionInsert {
    pub org_id: String,
    pub user_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub worktree_path: String,
    pub initial_phase: PhaseName,
    pub remaining_depth: Option<i64>,
    pub now: i64,
}

#[derive(Debug, Clone)]
pub struct NewRunInsert {
    pub org_id: String,
    pub user_id: String,
    pub partition_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub kind: RunKind,
    pub parent_run_id: Option<String>,
    pub prompt_text: String,
    pub started_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewShaverRunInsert {
    pub org_id: String,
    pub user_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub worktree_path: String,
    pub prompt_text: String,
    pub started_at: i64,
}

#[derive(Debug, Clone)]
pub struct RunningShaverRun {
    pub id: String,
    pub org_id: String,
    pub session_id: String,
    pub worktree_path: String,
}

#[derive(Debug, Clone)]
pub struct NewShavingTrackInsert {
    pub org_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub parent_tree_sha: String,
    pub head_tree_sha: String,
    pub steps: Vec<ShavingStep>,
    pub ref_name: String,
    pub created_at: i64,
}

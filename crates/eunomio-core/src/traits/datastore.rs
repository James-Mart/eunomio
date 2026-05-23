// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::AppError,
    principal::AuthSessionRow,
    types::*,
};
use async_trait::async_trait;

#[async_trait]
pub trait OrgRepo: Send + Sync {
    async fn ensure_singleton_local(&self) -> Result<(), AppError>;
}

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn get_by_id(&self, user_id: &str) -> Result<Option<UserRow>, AppError>;
    async fn get_by_username(&self, username: &str) -> Result<Option<UserRow>, AppError>;
    async fn insert(&self, username: &str) -> Result<UserRow, AppError>;
    async fn ensure_membership(
        &self,
        org_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<(), AppError>;
    async fn membership_role(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<String>, AppError>;
}

#[async_trait]
pub trait AuthSessionRepo: Send + Sync {
    async fn load(&self, session_id: &str) -> Result<Option<AuthSessionRow>, AppError>;
    async fn refresh_last_seen(&self, session_id: &str, now: i64) -> Result<(), AppError>;
    async fn delete(&self, session_id: &str) -> Result<(), AppError>;
    /// Single transaction: DELETE sessions for user, INSERT new session,
    /// INSERT auth_events `login_success` then `session_rotated` (that order).
    #[allow(clippy::too_many_arguments)]
    async fn rotate_with_audit(
        &self,
        user_id: &str,
        org_id: &str,
        new_session_id: &str,
        expires_at: i64,
        ip: &str,
        user_agent: &str,
        username_for_audit: &str,
    ) -> Result<(), AppError>;
    /// Single transaction: DELETE session by id, INSERT auth_events `logout`.
    async fn delete_with_audit(
        &self,
        session_id: &str,
        org_id: &str,
        user_id: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError>;
}

#[async_trait]
pub trait AuthEventRepo: Send + Sync {
    async fn insert(
        &self,
        org_id: Option<&str>,
        user_id: Option<&str>,
        event_type: &str,
        ip: &str,
        user_agent: &str,
        details: serde_json::Value,
    ) -> Result<(), AppError>;
    async fn list_by_event_type(&self, event_type: &str) -> Result<Vec<String>, AppError>;
}

#[async_trait]
pub trait SessionRepo: Send + Sync {
    async fn exists(&self, org_id: &str, session_id: &str) -> Result<bool, AppError>;
    async fn ensure(&self, org_id: &str, session_id: &str) -> Result<(), AppError>;
    async fn user_id(&self, org_id: &str, session_id: &str) -> Result<String, AppError>;
    async fn repo_fields(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<SessionRepoFields, AppError>;
    async fn list(&self, org_id: &str) -> Result<Vec<Session>, AppError>;
    async fn get(&self, org_id: &str, session_id: &str) -> Result<Option<Session>, AppError>;
    async fn final_tree(&self, org_id: &str, session_id: &str) -> Result<Option<String>, AppError>;
    async fn base_tree(&self, org_id: &str, session_id: &str) -> Result<Option<String>, AppError>;
    async fn seed_trees(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<(String, String), AppError>;
    async fn find_by_refs(
        &self,
        org_id: &str,
        normalized_remote: &str,
        base_ref: &str,
        source_ref: &str,
    ) -> Result<Option<Session>, AppError>;
    async fn count_for_normalized(
        &self,
        org_id: &str,
        normalized_remote: &str,
    ) -> Result<i64, AppError>;
    #[allow(clippy::too_many_arguments)]
    async fn insert_seed_nodes(
        &self,
        org_id: String,
        user_id: String,
        session_id: String,
        normalized_remote: String,
        literal_remote: String,
        is_local: bool,
        base_ref: String,
        source_ref: String,
        base_tree: String,
        final_tree: String,
        base_node_id: String,
        final_node_id: String,
        base_commit: String,
        final_commit: String,
        now: i64,
    ) -> Result<(), AppError>;
    async fn list_partition_worktrees(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<Vec<String>, AppError>;
    async fn delete_cascade(&self, org_id: &str, session_id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait NodeRepo: Send + Sync {
    async fn list_for_session(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<Vec<GraphNode>, AppError>;
    async fn get(
        &self,
        org_id: &str,
        session_id: &str,
        node_id: &str,
    ) -> Result<Option<GraphNode>, AppError>;
    async fn target_and_parent(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<(NodeBasic, Option<NodeBasic>), AppError>;
    async fn target_tree_and_parent(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Option<(String, Option<String>, Option<String>)>, AppError>;
    async fn update_title(
        &self,
        org_id: &str,
        session_id: &str,
        node_id: &str,
        title: &str,
    ) -> Result<GraphNode, AppError>;
    async fn walk_to_base(
        &self,
        org_id: &str,
        session_id: &str,
        node_id: &str,
    ) -> Result<Vec<WalkNode>, AppError>;
    async fn session_for_node_id(&self, org_id: &str, node_id: &str) -> Result<String, AppError>;
    async fn distinct_trees_in_session(
        &self,
        org_id: &str,
        session_id: &str,
        trees: &[&str],
    ) -> Result<bool, AppError>;
}

#[async_trait]
pub trait PartitionRepo: Send + Sync {
    async fn get(&self, org_id: &str, partition_id: &str) -> Result<PartitionRow, AppError>;
    async fn list(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: Option<&str>,
    ) -> Result<Vec<PartitionRow>, AppError>;
    async fn list_siblings(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
        accepted_partition_id: &str,
    ) -> Result<Vec<SiblingInfo>, AppError>;
    async fn delete(&self, org_id: &str, partition_id: &str) -> Result<(), AppError>;
    async fn delete_with_runs(&self, org_id: &str, partition_id: &str) -> Result<(), AppError>;
    async fn delete_many_with_runs(
        &self,
        org_id: &str,
        partition_ids: Vec<String>,
    ) -> Result<(), AppError>;
    async fn insert_pending(&self, row: NewPartitionInsert) -> Result<String, AppError>;
    async fn clear_plan_and_slice(
        &self,
        org_id: &str,
        partition_id: &str,
    ) -> Result<(), AppError>;
    async fn accept_survey(
        &self,
        org_id: &str,
        partition_id: &str,
        change_survey_json: String,
    ) -> Result<(), AppError>;
    #[allow(clippy::too_many_arguments)]
    async fn finalize_construct_accept(
        &self,
        org_id: String,
        session_id: String,
        partition_id: String,
        target_node_id: String,
        slice_node_id: String,
        parent_node_id: String,
        candidate_tree: String,
        candidate_commit: String,
        slice_title: String,
        slice_description: String,
        slice_strategy: Option<PartitionStrategy>,
        leftover_title: String,
        leftover_description: String,
        sibling_ids: Vec<String>,
        now: i64,
    ) -> Result<(), AppError>;
    async fn accept_plan(
        &self,
        org_id: &str,
        partition_id: &str,
        plan_json: String,
        strategy: PartitionStrategy,
    ) -> Result<(), AppError>;
    async fn set_phase_state(
        &self,
        org_id: &str,
        partition_id: &str,
        phase_state: PhaseState,
    ) -> Result<(), AppError>;
    async fn set_phase_running(
        &self,
        org_id: &str,
        partition_id: &str,
        phase: PhaseName,
    ) -> Result<(), AppError>;
    async fn set_worktree_path(
        &self,
        org_id: &str,
        partition_id: &str,
        worktree_path: String,
    ) -> Result<(), AppError>;
    #[allow(clippy::too_many_arguments)]
    async fn accept_constructor_ok(
        &self,
        org_id: &str,
        partition_id: &str,
        tree_sha: String,
        commit_sha: String,
        run_id: &str,
        result_json: String,
        result_text: String,
    ) -> Result<(), AppError>;
    async fn accept_constructor_blocked(
        &self,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        result_json: String,
        result_text: String,
    ) -> Result<(), AppError>;
    async fn fail_run(
        &self,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        error_message: String,
        result_text: Option<String>,
    ) -> Result<(), AppError>;
    async fn cancel_run(
        &self,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
    ) -> Result<(), AppError>;
    async fn list_id_session_worktree(
        &self,
        org_id: &str,
    ) -> Result<Vec<(String, String, String)>, AppError>;
}

#[async_trait]
pub trait RunRepo: Send + Sync {
    async fn get(&self, org_id: &str, run_id: &str) -> Result<RunRow, AppError>;
    async fn list_for_partition(
        &self,
        org_id: &str,
        partition_id: &str,
    ) -> Result<Vec<RunRow>, AppError>;
    async fn start(&self, row: NewRunInsert) -> Result<String, AppError>;
    async fn get_prompt(
        &self,
        org_id: &str,
        run_id: &str,
    ) -> Result<Option<String>, AppError>;
    async fn append_transcript_text(
        &self,
        org_id: &str,
        run_id: &str,
        chunk: &str,
    ) -> Result<(), AppError>;
    async fn finish_success(
        &self,
        org_id: &str,
        run_id: &str,
        result_json: String,
        result_text: Option<String>,
    ) -> Result<(), AppError>;
    async fn finish_error(
        &self,
        org_id: &str,
        run_id: &str,
        error_message: String,
    ) -> Result<(), AppError>;
    async fn list_running_ids(&self, org_id: &str) -> Result<Vec<String>, AppError>;
    async fn mark_errored(
        &self,
        org_id: &str,
        run_ids: Vec<String>,
        error_message: &'static str,
    ) -> Result<(), AppError>;
    async fn cancel_running_for_partition(
        &self,
        org_id: &str,
        partition_id: &str,
    ) -> Result<(), AppError>;
    async fn cancel(&self, org_id: &str, run_id: &str) -> Result<(), AppError>;
}

/// Umbrella datastore trait.
pub trait Datastore: Send + Sync {
    fn orgs(&self) -> &dyn OrgRepo;
    fn users(&self) -> &dyn UserRepo;
    fn auth_sessions(&self) -> &dyn AuthSessionRepo;
    fn auth_events(&self) -> &dyn AuthEventRepo;
    fn sessions(&self) -> &dyn SessionRepo;
    fn nodes(&self) -> &dyn NodeRepo;
    fn partitions(&self) -> &dyn PartitionRepo;
    fn runs(&self) -> &dyn RunRepo;
}

use crate::{error::AppError, repo, state::AppState, types::*, worktree};
use serde::Serialize;
use std::path::PathBuf;

use super::{parse_split_plan, Coordinator};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConstructOkPayload {
    outcome: &'static str,
    candidate_tree_sha: String,
    candidate_commit_sha: String,
}

impl ConstructOkPayload {
    fn new(tree_sha: String, commit_sha: String) -> Self {
        Self {
            outcome: "ok",
            candidate_tree_sha: tree_sha,
            candidate_commit_sha: commit_sha,
        }
    }
}

impl Coordinator {
    pub(super) async fn constructor_capture_ok(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        raw: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<(), AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        let (_t, parent) = repo::node::target_and_parent(
            state,
            org_id,
            &row.session_id,
            &row.target_node_id,
        )
        .await?;
        let parent =
            parent.ok_or_else(|| AppError::BadRequest("no parent".into()))?;
        let plan_json = row
            .plan_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("no plan".into()))?;
        let split = parse_split_plan(plan_json).map_err(|e| match e {
            AppError::BadRequest(_) => AppError::BadRequest(
                "constructor produced OK for an indivisible plan".into(),
            ),
            other => other,
        })?;
        let slice_title = split.edges[0].title.clone();

        let git_root = repo::session::git_root(state, org_id, &row.session_id).await?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        let (tree_sha, commit_sha) = worktree::capture_slice_commit(
            &git_root,
            &worktree_path,
            &parent.commit_sha,
            &slice_title,
        )
        .await?;
        worktree::reset_to_parent(&worktree_path, &parent.commit_sha, true).await?;

        let payload = ConstructOkPayload::new(tree_sha, commit_sha);
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("construct-ok json: {e}")))?;
        repo::partition::accept_constructor_ok(
            state,
            org_id,
            partition_id,
            payload.candidate_tree_sha.clone(),
            payload.candidate_commit_sha.clone(),
            run_id,
            payload_json,
            raw.to_string(),
        )
        .await?;

        self.handle_phase_terminal(
            state,
            org_id,
            partition_id,
            RunKind::Construct,
            run_id,
            session_id,
            target_node_id,
            serde_json::to_value(&payload).ok(),
        )
        .await?;
        Ok(())
    }

    pub(super) async fn constructor_capture_blocked(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        raw: &str,
        reason: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<(), AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        let (_t, parent) = repo::node::target_and_parent(
            state,
            org_id,
            &row.session_id,
            &row.target_node_id,
        )
        .await?;
        let parent =
            parent.ok_or_else(|| AppError::BadRequest("no parent".into()))?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        worktree::reset_to_parent(&worktree_path, &parent.commit_sha, false).await?;
        let result_json = serde_json::json!({
            "outcome": "blocked",
            "reason": reason,
        });
        repo::partition::accept_constructor_blocked(
            state,
            org_id,
            partition_id,
            run_id,
            result_json.to_string(),
            raw.to_string(),
        )
        .await?;
        self.emit(
            session_id,
            SseEvent::Phase {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id: partition_id.to_string(),
                name: PhaseName::Construct,
                state: PhaseState::AwaitingReview,
                payload: Some(serde_json::json!({"outcome": "blocked", "reason": reason})),
            },
        );
        Ok(())
    }
}

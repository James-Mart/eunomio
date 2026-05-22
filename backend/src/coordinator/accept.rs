use crate::{db, error::AppError, repo, state::AppState, subagents::surveyor::SurveyOutput, types::*, worktree};
use std::path::PathBuf;
use uuid::Uuid;

use super::{ensure_at_gate, parse_split_plan, Coordinator};

impl Coordinator {
    pub async fn accept_survey(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        req: AcceptSurveyRequest,
    ) -> Result<Partition, AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        ensure_at_gate(&row, PhaseName::Survey, "survey")?;
        self.do_accept_survey(state, org_id, partition_id, &req.run_id)
            .await
    }

    pub(super) async fn do_accept_survey(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
    ) -> Result<Partition, AppError> {
        let run = repo::run::get(state, org_id, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::BadRequest(
                "runId does not belong to this partition".into(),
            ));
        }
        let result_json = run
            .result_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("survey run has no parsed result".into()))?;
        let _: SurveyOutput = serde_json::from_str(result_json)
            .map_err(|e| AppError::BadRequest(format!("invalid survey result: {e}")))?;
        repo::partition::accept_survey(state, org_id, partition_id, result_json.to_string())
            .await?;
        let new_row = repo::partition::get(state, org_id, partition_id).await?;
        self.spawn_run_boxed(
            state.clone(),
            org_id.to_string(),
            partition_id.to_string(),
            StartRunRequest {
                kind: RunKind::Plan,
                parent_run_id: Some(run_id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        Ok(new_row.into())
    }

    pub async fn accept_plan(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        req: AcceptPlanRequest,
    ) -> Result<Partition, AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        ensure_at_gate(&row, PhaseName::Plan, "plan")?;
        self.do_accept_plan(state, org_id, partition_id, &req.run_id)
            .await
    }

    pub(super) async fn do_accept_plan(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
    ) -> Result<Partition, AppError> {
        let run = repo::run::get(state, org_id, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::BadRequest(
                "runId does not belong to this partition".into(),
            ));
        }
        let result_json = run
            .result_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("plan run has no parsed result".into()))?;
        let split = parse_split_plan(result_json)?;
        repo::partition::accept_plan(state, org_id, partition_id, result_json.to_string(), split.strategy)
            .await?;
        let new_row = repo::partition::get(state, org_id, partition_id).await?;
        self.spawn_run_boxed(
            state.clone(),
            org_id.to_string(),
            partition_id.to_string(),
            StartRunRequest {
                kind: RunKind::Construct,
                parent_run_id: Some(run_id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        Ok(new_row.into())
    }

    pub async fn accept_construct(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
    ) -> Result<(), AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        if !matches!(row.phase, PhaseName::Construct) {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not at the construct review gate".into(),
            });
        }
        self.do_accept_construct(state, org_id, partition_id).await
    }

    pub(super) async fn do_accept_construct(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
    ) -> Result<(), AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        let candidate_tree = row
            .candidate_slice_tree_sha
            .clone()
            .ok_or_else(|| AppError::BadRequest("no candidate slice to accept".into()))?;
        let candidate_commit = row
            .candidate_slice_commit_sha
            .clone()
            .ok_or_else(|| AppError::BadRequest("no candidate slice to accept".into()))?;
        let plan_json = row
            .plan_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("no plan accepted".into()))?;
        let split = parse_split_plan(plan_json)?;

        let session_id = row.session_id.clone();
        let target_node_id = row.target_node_id.clone();
        let (_target_node, parent_node) = repo::node::target_and_parent(
            state,
            org_id,
            &session_id,
            &target_node_id,
        )
        .await?;
        let parent = parent_node.ok_or_else(|| {
            AppError::BadRequest("target has no parent; cannot insert slice".into())
        })?;

        let siblings = repo::partition::list_siblings(
            state,
            org_id,
            &session_id,
            &target_node_id,
            partition_id,
        )
        .await?;

        self.cancel_siblings(&siblings).await;

        let slice_node_id = Uuid::new_v4().to_string();
        let now = db::unix_seconds();
        let slice_title = split.edges[0].title.clone();
        let slice_description = split.edges[0].description.clone();
        let leftover_title = split.edges[1].title.clone();
        let leftover_description = split.edges[1].description.clone();
        let remaining_depth = row.remaining_depth;

        let sibling_ids: Vec<String> = siblings.iter().map(|s| s.id.clone()).collect();
        repo::partition::finalize_construct_accept(
            state,
            org_id.to_string(),
            session_id.clone(),
            partition_id.to_string(),
            target_node_id.clone(),
            slice_node_id.clone(),
            parent.node_id.clone(),
            candidate_tree,
            candidate_commit,
            slice_title,
            slice_description,
            row.strategy,
            leftover_title,
            leftover_description,
            sibling_ids,
            now,
        )
        .await?;

        teardown_worktrees(state, org_id, &row, &siblings).await;
        self.emit_acceptance_events(&session_id, &target_node_id, partition_id, &siblings);
        self.inner.runs.unmark_abandoning_many(
            &siblings.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        );

        self.maybe_spawn_fanout(
            state,
            org_id.to_string(),
            session_id,
            target_node_id,
            slice_node_id,
            remaining_depth,
        );
        Ok(())
    }

    async fn cancel_siblings(&self, siblings: &[repo::partition::SiblingInfo]) {
        let ids: Vec<String> = siblings.iter().map(|s| s.id.clone()).collect();
        self.inner.runs.mark_abandoning_many(&ids);
        for id in &ids {
            self.inner.runs.take_and_cancel(id).await;
        }
    }

    fn emit_acceptance_events(
        &self,
        session_id: &str,
        target_node_id: &str,
        partition_id: &str,
        siblings: &[repo::partition::SiblingInfo],
    ) {
        self.emit(
            session_id,
            SseEvent::Finished {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id: partition_id.to_string(),
            },
        );
        for sib in siblings {
            self.emit(
                session_id,
                SseEvent::Cancelled {
                    session_id: session_id.to_string(),
                    target_node_id: sib.target_node_id.clone(),
                    partition_id: sib.id.clone(),
                },
            );
        }
    }

    fn maybe_spawn_fanout(
        &self,
        state: &AppState,
        org_id: String,
        session_id: String,
        renamed_target_id: String,
        new_slice_id: String,
        remaining_depth: Option<i64>,
    ) {
        let should_fan_out = match remaining_depth {
            None => true,
            Some(n) => n > 1,
        };
        if !should_fan_out {
            return;
        }
        let coord = self.clone();
        let state_for_children = state.clone();
        tokio::spawn(async move {
            let on_slice = coord
                .begin_child_partition(
                    &state_for_children,
                    &org_id,
                    &session_id,
                    &new_slice_id,
                    remaining_depth,
                )
                .await;
            if let Err(e) = on_slice {
                tracing::warn!(error = %e, "fan-out child on slice failed");
            }
            let on_target = coord
                .begin_child_partition(
                    &state_for_children,
                    &org_id,
                    &session_id,
                    &renamed_target_id,
                    remaining_depth,
                )
                .await;
            if let Err(e) = on_target {
                tracing::warn!(error = %e, "fan-out child on renamed target failed");
            }
        });
    }

    pub async fn abandon_partition(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
    ) -> Result<(), AppError> {
        let row = repo::partition::get(state, org_id, partition_id).await?;
        self.inner.runs.mark_abandoning(partition_id);
        self.inner.runs.take_and_cancel(partition_id).await;
        repo::run::cancel_running_for_partition(state, org_id, partition_id).await?;
        repo::partition::delete_with_runs(state, org_id, partition_id).await?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        let git_root = repo::session::git_root(state, org_id, &row.session_id).await?;
        worktree::teardown(&git_root, &worktree_path).await;
        self.emit(
            &row.session_id,
            SseEvent::Cancelled {
                session_id: row.session_id.clone(),
                target_node_id: row.target_node_id.clone(),
                partition_id: partition_id.to_string(),
            },
        );
        self.inner.runs.unmark_abandoning(partition_id);
        Ok(())
    }
}

async fn teardown_worktrees(
    state: &AppState,
    org_id: &str,
    row: &PartitionRow,
    siblings: &[repo::partition::SiblingInfo],
) {
    let git_root = match repo::session::git_root(state, org_id, &row.session_id).await {
        Ok(p) => p,
        Err(_) => return,
    };
    let worktree_path = PathBuf::from(&row.worktree_path);
    worktree::teardown(&git_root, &worktree_path).await;
    for sib in siblings {
        let sib_path = PathBuf::from(&sib.worktree_path);
        worktree::teardown(&git_root, &sib_path).await;
    }
}

// SPDX-License-Identifier: Apache-2.0

use crate::{
    partition_settings::load_for_partition, repo_store, state::AppState, worktree, AppError,
};
use eunomio_core::types::*;

use super::Coordinator;

fn first_run_kind(settings: &PartitionSettings) -> RunKind {
    if settings.coordinator.surveyor_enabled {
        RunKind::Survey
    } else {
        RunKind::Plan
    }
}

impl Coordinator {
    pub async fn begin_partition(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Partition, AppError> {
        self.begin_partition_internal(state, org_id, session_id, target_node_id, None)
            .await
    }

    pub(super) async fn begin_child_partition(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
        parent_remaining_depth: Option<i64>,
    ) -> Result<Partition, AppError> {
        let remaining_depth = parent_remaining_depth.map(|n| (n - 1).max(0));
        self.begin_partition_internal(
            state,
            org_id,
            session_id,
            target_node_id,
            Some(remaining_depth),
        )
        .await
    }

    async fn begin_partition_internal(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
        remaining_depth: Option<Option<i64>>,
    ) -> Result<Partition, AppError> {
        let settings = load_for_partition(state, org_id, session_id).await?;
        let remaining_depth = remaining_depth.unwrap_or({
            match settings.coordinator.max_iterations {
                IterationLimit::Count { count } => Some(count as i64),
                IterationLimit::Auto => None,
            }
        });
        let first_kind = first_run_kind(&settings);

        let (_, parent_node) = state
            .datastore
            .nodes()
            .target_and_parent(org_id, session_id, target_node_id)
            .await?;
        let parent = parent_node.ok_or_else(|| {
            AppError::BadRequest("base node has no incoming edge to partition".into())
        })?;

        let user_id = state
            .datastore
            .sessions()
            .user_id(org_id, session_id)
            .await?;
        let fields = state
            .datastore
            .sessions()
            .repo_fields(org_id, session_id)
            .await?;
        repo_store::fetch_for_session(
            &state.data_dir,
            &fields.normalized_remote,
            &fields.literal_remote,
            fields.is_local,
        )
        .await?;
        let git_root = crate::repo_store::session_git_root(state, org_id, session_id).await?;

        let now = eunomio_core::unix_seconds();
        let inserted_id = state
            .datastore
            .partitions()
            .insert_pending(NewPartitionInsert {
                org_id: org_id.to_string(),
                user_id,
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                worktree_path: String::new(),
                initial_phase: first_kind.phase(),
                remaining_depth,
                now,
            })
            .await?;

        let worktree_path = match worktree::provision(
            &git_root,
            &state.data_dir,
            session_id,
            &inserted_id,
            &parent.commit_sha,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                let _ = state
                    .datastore
                    .partitions()
                    .delete(org_id, &inserted_id)
                    .await;
                return Err(e);
            }
        };

        state
            .datastore
            .partitions()
            .set_worktree_path(
                org_id,
                &inserted_id,
                worktree_path.to_string_lossy().to_string(),
            )
            .await?;

        self.emit(
            session_id,
            SseEvent::Started {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id: inserted_id.clone(),
            },
        );

        let row = state
            .datastore
            .partitions()
            .get(org_id, &inserted_id)
            .await?;
        let partition: Partition = row.into();

        self.spawn_run_boxed(
            state.clone(),
            org_id.to_string(),
            inserted_id,
            StartRunRequest {
                kind: first_kind,
                ..Default::default()
            },
        )
        .await?;

        Ok(partition)
    }
}

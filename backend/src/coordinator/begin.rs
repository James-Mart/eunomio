use crate::{db, error::AppError, repo, state::AppState, types::*, worktree};

use super::Coordinator;

impl Coordinator {
    pub async fn begin_partition(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Partition, AppError> {
        let settings = state.partition_settings.snapshot().await;
        let remaining_depth = match settings.coordinator.max_iterations {
            IterationLimit::Count { count } => Some(count as i64),
            IterationLimit::Auto => None,
        };
        self.begin_partition_internal(state, session_id, target_node_id, remaining_depth)
            .await
    }

    pub(super) async fn begin_child_partition(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: &str,
        parent_remaining_depth: Option<i64>,
    ) -> Result<Partition, AppError> {
        let remaining_depth = parent_remaining_depth.map(|n| (n - 1).max(0));
        self.begin_partition_internal(state, session_id, target_node_id, remaining_depth)
            .await
    }

    async fn begin_partition_internal(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: &str,
        remaining_depth: Option<i64>,
    ) -> Result<Partition, AppError> {
        let (_, parent_node) =
            repo::node::target_and_parent(state, session_id, target_node_id).await?;
        let parent = parent_node.ok_or_else(|| {
            AppError::BadRequest("base node has no incoming edge to partition".into())
        })?;

        let now = db::unix_seconds();
        let inserted_id = repo::partition::insert_pending(
            state,
            session_id.to_string(),
            target_node_id.to_string(),
            String::new(),
            remaining_depth,
            now,
        )
        .await?;

        let worktree_path = match worktree::provision(
            &state.repo_root,
            &state.data_dir,
            session_id,
            inserted_id,
            &parent.commit_sha,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                let _ = repo::partition::delete(state, inserted_id).await;
                return Err(e);
            }
        };

        repo::partition::set_worktree_path(
            state,
            inserted_id,
            worktree_path.to_string_lossy().to_string(),
        )
        .await?;

        self.emit(
            session_id,
            SseEvent::Started {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id: inserted_id,
            },
        );

        let row = repo::partition::get(state, inserted_id).await?;
        let partition: Partition = row.into();

        self.spawn_run_boxed(state.clone(), inserted_id, RunKind::Survey, None, None, None, None)
            .await?;

        Ok(partition)
    }
}

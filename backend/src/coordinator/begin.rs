use crate::{
    db, error::AppError, partition_settings::load_for_partition, repo, repo_store, state::AppState,
    types::*, worktree,
};

use super::Coordinator;

impl Coordinator {
    pub async fn begin_partition(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Partition, AppError> {
        let settings = load_for_partition(state, org_id, session_id).await?;
        let remaining_depth = match settings.coordinator.max_iterations {
            IterationLimit::Count { count } => Some(count as i64),
            IterationLimit::Auto => None,
        };
        self.begin_partition_internal(
            state,
            org_id,
            session_id,
            target_node_id,
            remaining_depth,
        )
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
            remaining_depth,
        )
        .await
    }

    async fn begin_partition_internal(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
        remaining_depth: Option<i64>,
    ) -> Result<Partition, AppError> {
        let (_, parent_node) =
            repo::node::target_and_parent(state, org_id, session_id, target_node_id).await?;
        let parent = parent_node.ok_or_else(|| {
            AppError::BadRequest("base node has no incoming edge to partition".into())
        })?;

        let user_id = repo::session::user_id(state, org_id, session_id).await?;
        let fields = repo::session::repo_fields(state, org_id, session_id).await?;
        repo_store::fetch_for_session(
            &state.data_dir,
            &fields.normalized_remote,
            &fields.literal_remote,
            fields.is_local,
        )
        .await?;
        let git_root = repo::session::git_root(state, org_id, session_id).await?;

        let now = db::unix_seconds();
        let inserted_id = repo::partition::insert_pending(
            state,
            repo::partition::NewPartitionInsert {
                org_id: org_id.to_string(),
                user_id,
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                worktree_path: String::new(),
                remaining_depth,
                now,
            },
        )
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
                let _ = repo::partition::delete(state, org_id, &inserted_id).await;
                return Err(e);
            }
        };

        repo::partition::set_worktree_path(
            state,
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

        let row = repo::partition::get(state, org_id, &inserted_id).await?;
        let partition: Partition = row.into();

        self.spawn_run_boxed(
            state.clone(),
            org_id.to_string(),
            inserted_id,
            StartRunRequest {
                kind: RunKind::Survey,
                ..Default::default()
            },
        )
        .await?;

        Ok(partition)
    }
}

use crate::{db, error::AppError, repo, state::AppState, worktree};
use std::path::Path;

use super::Coordinator;

impl Coordinator {
    /// Reconcile the database and on-disk worktree tree with reality at boot:
    /// any run still flagged `running` belongs to a process that is no longer
    /// here, any partition row pointing at a missing worktree directory is
    /// dead, and any worktree directory whose partition row is gone is
    /// orphaned. Each step is best-effort and proceeds even if the previous
    /// one logged a warning.
    pub async fn process_startup_recovery(&self, state: &AppState) -> Result<(), AppError> {
        repair_stuck_runs(state).await?;
        let alive = prune_dead_partition_rows(state).await?;
        sweep_orphan_worktree_dirs(state, &alive).await;
        Ok(())
    }
}

async fn repair_stuck_runs(state: &AppState) -> Result<(), AppError> {
    let stuck = repo::run::list_running_ids(state, db::LOCAL_ORG_ID).await?;
    repo::run::mark_errored(state, db::LOCAL_ORG_ID, stuck, "process_restart").await?;
    Ok(())
}

async fn prune_dead_partition_rows(state: &AppState) -> Result<Vec<String>, AppError> {
    let rows = repo::partition::list_id_session_worktree(state, db::LOCAL_ORG_ID).await?;
    let mut alive = Vec::with_capacity(rows.len());
    for (id, _session_id, worktree_path) in rows {
        if Path::new(&worktree_path).exists() {
            alive.push(id);
        } else {
            tracing::warn!(
                partition_id = %id,
                worktree = %worktree_path,
                "partition worktree missing on disk; deleting row"
            );
            repo::partition::delete_with_runs(state, db::LOCAL_ORG_ID, &id).await?;
        }
    }
    Ok(alive)
}

async fn sweep_orphan_worktree_dirs(state: &AppState, alive: &[String]) {
    let worktrees_root = state.data_dir.join("worktrees");
    if !worktrees_root.exists() {
        return;
    }
    let mut sessions_iter = match tokio::fs::read_dir(&worktrees_root).await {
        Ok(it) => it,
        Err(e) => {
            tracing::warn!(error = %e, "reading worktrees root failed");
            return;
        }
    };
    while let Ok(Some(session_entry)) = sessions_iter.next_entry().await {
        let session_path = session_entry.path();
        if !session_path.is_dir() {
            continue;
        }
        let mut parts_iter = match tokio::fs::read_dir(&session_path).await {
            Ok(it) => it,
            Err(_) => continue,
        };
        while let Ok(Some(part_entry)) = parts_iter.next_entry().await {
            let part_path = part_entry.path();
            if !part_path.is_dir() {
                continue;
            }
            let pid_opt = part_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string);
            let worktree_path = part_path.join("worktree");
            if !worktree_path.exists() {
                continue;
            }
            if pid_opt
                .as_ref()
                .map(|p| alive.contains(p))
                .unwrap_or(false)
            {
                continue;
            }
            tracing::info!(path = %worktree_path.display(), "removing orphan partition worktree");
            let session_id = session_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if let Ok(git_root) =
                repo::session::git_root(state, db::LOCAL_ORG_ID, session_id).await
            {
                worktree::teardown(&git_root, &worktree_path).await;
            } else {
                let _ = tokio::fs::remove_dir_all(part_path).await;
            }
        }
    }
}

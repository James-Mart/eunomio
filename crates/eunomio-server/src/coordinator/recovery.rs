// SPDX-License-Identifier: Apache-2.0

use crate::{state::AppState, storage_path, worktree, AppError};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const LOCAL_ORG_ID: &str = "local";

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
        repair_stuck_shaver_runs(state).await?;
        let alive = prune_dead_partition_rows(state).await?;
        sweep_orphan_worktree_dirs(state, &alive).await;
        sweep_orphan_repo_dirs(state).await?;
        recover_missing_shaving_tracks(self, state).await?;
        retry_incomplete_finalizations(self, state).await?;
        Ok(())
    }
}

async fn retry_incomplete_finalizations(
    coord: &Coordinator,
    state: &AppState,
) -> Result<(), AppError> {
    let sessions = state
        .datastore
        .sessions()
        .list_sessions_needing_finalization(LOCAL_ORG_ID)
        .await?;
    for session_id in sessions {
        if let Err(e) = coord
            .recover_session_partition_finalization(state, LOCAL_ORG_ID, &session_id)
            .await
        {
            tracing::warn!(session_id = %session_id, error = %e, "session finalization recovery failed");
        }
    }
    Ok(())
}

async fn recover_missing_shaving_tracks(
    coord: &Coordinator,
    state: &AppState,
) -> Result<(), AppError> {
    let sessions = state
        .datastore
        .sessions()
        .list_completed_session_ids(LOCAL_ORG_ID)
        .await?;
    for session_id in sessions {
        if let Err(e) = coord
            .spawn_missing_timelines_for_session(state, LOCAL_ORG_ID, &session_id)
            .await
        {
            tracing::warn!(session_id = %session_id, error = %e, "shaving recovery failed");
        }
    }
    Ok(())
}

async fn repair_stuck_shaver_runs(state: &AppState) -> Result<(), AppError> {
    let stuck = state
        .datastore
        .shaver_runs()
        .list_running(LOCAL_ORG_ID)
        .await?;
    for row in &stuck {
        if let Ok(git_root) =
            crate::repo_store::session_git_root(state, &row.org_id, &row.session_id).await
        {
            worktree::teardown(&git_root, Path::new(&row.worktree_path)).await;
        }
    }
    state
        .datastore
        .shaver_runs()
        .mark_errored(
            LOCAL_ORG_ID,
            stuck.into_iter().map(|row| row.id).collect(),
            "process_restart",
        )
        .await?;
    Ok(())
}

async fn repair_stuck_runs(state: &AppState) -> Result<(), AppError> {
    let stuck = state
        .datastore
        .runs()
        .list_running_ids(LOCAL_ORG_ID)
        .await?;
    state
        .datastore
        .runs()
        .mark_errored(LOCAL_ORG_ID, stuck, "process_restart")
        .await?;
    Ok(())
}

async fn prune_dead_partition_rows(state: &AppState) -> Result<HashSet<PathBuf>, AppError> {
    let rows = state
        .datastore
        .partitions()
        .list_all_id_org_session_worktree()
        .await?;
    let mut alive = HashSet::with_capacity(rows.len());
    for (id, org_id, _session_id, worktree_path) in rows {
        let path = PathBuf::from(&worktree_path);
        if path.exists() {
            alive.insert(path);
        } else {
            tracing::warn!(
                partition_id = %id,
                org_id = %org_id,
                worktree = %worktree_path,
                "partition worktree missing on disk; deleting row"
            );
            state
                .datastore
                .partitions()
                .delete_with_runs(&org_id, &id)
                .await?;
        }
    }
    Ok(alive)
}

async fn sweep_orphan_worktree_dirs(state: &AppState, alive: &HashSet<PathBuf>) {
    let worktrees_root = state.data_dir.join("worktrees");
    if !worktrees_root.exists() {
        return;
    }
    let worktrees = collect_worktree_paths(&worktrees_root);
    for worktree_path in worktrees {
        if alive.contains(&worktree_path) {
            continue;
        }
        tracing::info!(path = %worktree_path.display(), "removing orphan worktree");
        if let Some(parent) = worktree_path.parent() {
            let _ = tokio::fs::remove_dir_all(parent).await;
        }
    }
}

fn collect_worktree_paths(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_worktree_paths_inner(root, &mut out);
    out
}

fn collect_worktree_paths_inner(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = std::fs::read_dir(path) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("worktree") {
            out.push(path);
        } else {
            collect_worktree_paths_inner(&path, out);
        }
    }
}

async fn sweep_orphan_repo_dirs(state: &AppState) -> Result<(), AppError> {
    let repos_root = state.data_dir.join("repos");
    if !repos_root.exists() {
        return Ok(());
    }
    let identities = state.datastore.sessions().list_repo_identities().await?;
    let expected: HashSet<PathBuf> = identities
        .iter()
        .map(|i| storage_path::clone_path(&state.data_dir, &i.org_id, &i.normalized_remote))
        .collect();

    let mut orgs_iter = match tokio::fs::read_dir(&repos_root).await {
        Ok(it) => it,
        Err(e) => {
            tracing::warn!(error = %e, "reading repos root failed");
            return Ok(());
        }
    };
    while let Ok(Some(org_entry)) = orgs_iter.next_entry().await {
        let org_path = org_entry.path();
        if !org_path.is_dir() {
            continue;
        }
        let old_layout_bare = org_path.join("HEAD").exists() && org_path.join("objects").is_dir();
        if old_layout_bare {
            tracing::info!(path = %org_path.display(), "removing old-layout managed clone");
            let _ = tokio::fs::remove_dir_all(&org_path).await;
            continue;
        }
        let mut remotes_iter = match tokio::fs::read_dir(&org_path).await {
            Ok(it) => it,
            Err(_) => continue,
        };
        while let Ok(Some(remote_entry)) = remotes_iter.next_entry().await {
            let clone_path = remote_entry.path();
            if !clone_path.is_dir() || expected.contains(&clone_path) {
                continue;
            }
            if !storage_path::repo_metadata_path(&clone_path).exists() {
                continue;
            }
            tracing::info!(path = %clone_path.display(), "removing orphan managed clone");
            let _ = tokio::fs::remove_dir_all(&clone_path).await;
        }
    }
    Ok(())
}

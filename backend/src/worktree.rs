use crate::{error::AppError, git};
use anyhow::anyhow;
use std::path::{Path, PathBuf};

/// Layout for a partition's worktree:
///
/// ```text
/// <data_dir>/worktrees/<session_id>/<partition_id>/worktree
/// ```
///
/// Creates the parent directory and runs `git worktree add --detach
/// <path> <parent_commit>` so the partition has an isolated checkout
/// pinned to its parent node.
pub async fn provision(
    repo_root: &Path,
    data_dir: &Path,
    session_id: &str,
    partition_id: i64,
    parent_commit: &str,
) -> Result<PathBuf, AppError> {
    let worktree_path = data_dir
        .join("worktrees")
        .join(session_id)
        .join(partition_id.to_string())
        .join("worktree");
    if let Some(parent_dir) = worktree_path.parent() {
        tokio::fs::create_dir_all(parent_dir)
            .await
            .map_err(|e| AppError::Internal(anyhow!("create worktree parent: {e}")))?;
    }
    git::worktree_add(repo_root, &worktree_path, parent_commit)
        .await
        .map_err(|e| AppError::Internal(anyhow!("worktree add: {e}")))?;
    Ok(worktree_path)
}

/// Resets `worktree` back to `parent_commit` and clears every untracked
/// file. With `strict = true` (used during constructor capture) failures
/// surface as `AppError::Internal`; with `strict = false` (used during
/// blocked-construct cleanup) failures are logged and swallowed since the
/// partition is already being parked.
pub async fn reset_to_parent(
    worktree: &Path,
    parent_commit: &str,
    strict: bool,
) -> Result<(), AppError> {
    let reset = git::run_in(worktree, &["reset", "--hard", parent_commit]).await;
    let clean = git::run_in(worktree, &["clean", "-fdx"]).await;
    if strict {
        reset.map_err(|e| AppError::Internal(anyhow!("reset --hard: {e}")))?;
        clean.map_err(|e| AppError::Internal(anyhow!("clean -fdx: {e}")))?;
    } else {
        if let Err(e) = reset {
            tracing::warn!(error = %e, "reset --hard during back-edge cleanup failed");
        }
        if let Err(e) = clean {
            tracing::warn!(error = %e, "clean -fdx during back-edge cleanup failed");
        }
    }
    Ok(())
}

/// Stages the worktree, writes its tree, and commits it on top of
/// `parent_commit` with `title` as the message. Returns the new
/// `(tree_sha, commit_sha)` pair. Caller is responsible for any
/// follow-up reset.
pub async fn capture_slice_commit(
    repo_root: &Path,
    worktree: &Path,
    parent_commit: &str,
    title: &str,
) -> Result<(String, String), AppError> {
    git::run_in(worktree, &["add", "-A"])
        .await
        .map_err(|e| AppError::Internal(anyhow!("git add -A: {e}")))?;
    let tree_sha = git::write_tree(worktree)
        .await
        .map_err(|e| AppError::Internal(anyhow!("write-tree: {e}")))?;
    let commit_sha = git::commit_tree(repo_root, &tree_sha, &[parent_commit], title)
        .await
        .map_err(|e| AppError::Internal(anyhow!("commit-tree: {e}")))?;
    Ok((tree_sha, commit_sha))
}

/// Removes the worktree from git and deletes the immediate parent
/// directory (`<session_id>/<partition_id>`) on disk. Failures are
/// logged and swallowed because teardown happens in cleanup paths
/// where we don't want to mask the real error.
pub async fn teardown(repo_root: &Path, worktree: &Path) {
    if worktree.exists() {
        if let Err(e) = git::worktree_remove(repo_root, worktree).await {
            tracing::warn!(error = %e, worktree = %worktree.display(), "worktree remove failed");
        }
    }
    if let Some(parent_dir) = worktree.parent() {
        let _ = tokio::fs::remove_dir_all(parent_dir).await;
    }
}

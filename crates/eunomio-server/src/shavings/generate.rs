// SPDX-License-Identifier: Apache-2.0

use crate::{
    git::{self, TreeChange, TreeChangeStatus},
    repo_store,
    shavings::validate,
    storage_path, AppState, NewShavingTrackInsert, ShavingStep,
};
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct AttachTrackInput {
    pub org_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub parent_node_id: String,
    pub parent_tree_sha: String,
    pub parent_commit_sha: String,
    pub candidate_tree_sha: String,
    pub candidate_commit_sha: String,
}

pub async fn try_attach_track(state: &AppState, input: AttachTrackInput) {
    if let Err(e) = attach_track(state, input).await {
        tracing::warn!(error = %e, "shaving track generation skipped");
    }
}

async fn attach_track(state: &AppState, input: AttachTrackInput) -> Result<()> {
    let git_root = repo_store::session_git_root(state, &input.org_id, &input.session_id).await?;
    let mut changes =
        git::changed_entries(&git_root, &input.parent_tree_sha, &input.candidate_tree_sha).await?;
    changes.sort_by(|a, b| display_path(a).cmp(display_path(b)));
    if changes.len() < 2 {
        return Ok(());
    }

    let worktree = provision_shaving_worktree(
        &git_root,
        &state.data_dir,
        &input.org_id,
        &input.session_id,
        &input.parent_commit_sha,
    )
    .await?;

    let build_result = build_steps(&git_root, &worktree, &input, &changes).await;
    let steps = match build_result {
        Ok(steps) => steps,
        Err(e) => {
            cleanup_worktree(&git_root, &worktree).await;
            return Err(e);
        }
    };

    if steps.last().map(|step| step.tree_sha.as_str()) != Some(input.candidate_tree_sha.as_str()) {
        cleanup_worktree(&git_root, &worktree).await;
        return Err(anyhow!(
            "generated shaving head tree does not match candidate tree"
        ));
    }

    if let Err(e) = validate::validate_track(
        &git_root,
        &input.parent_tree_sha,
        &input.parent_commit_sha,
        &input.candidate_tree_sha,
        &steps,
    )
    .await
    {
        cleanup_worktree(&git_root, &worktree).await;
        return Err(anyhow!("shaving validation failed: {e}"));
    }

    let ref_name = format!("refs/eunomio/shavings/{}", input.target_node_id);
    let last_commit = steps
        .last()
        .map(|step| step.commit_sha.as_str())
        .ok_or_else(|| anyhow!("validated shaving track has no steps"))?;
    if let Err(e) = git::update_ref(&git_root, &ref_name, last_commit).await {
        cleanup_worktree(&git_root, &worktree).await;
        return Err(e);
    }

    if let Err(e) = teardown_shaving_worktree(&git_root, &worktree).await {
        if let Err(cleanup_err) = git::delete_ref(&git_root, &ref_name).await {
            tracing::warn!(error = %cleanup_err, ref_name = %ref_name, "shaving ref cleanup failed");
        }
        return Err(e);
    }

    let org_id = input.org_id.clone();
    let session_id = input.session_id.clone();
    let target_node_id = input.target_node_id.clone();
    let row = NewShavingTrackInsert {
        org_id: org_id.clone(),
        session_id: session_id.clone(),
        target_node_id: target_node_id.clone(),
        parent_tree_sha: input.parent_tree_sha,
        head_tree_sha: input.candidate_tree_sha,
        steps,
        ref_name: ref_name.clone(),
        created_at: eunomio_core::unix_seconds(),
    };

    if let Err(e) = state.datastore.shaving_tracks().insert(row).await {
        let _ = state
            .datastore
            .shaving_tracks()
            .delete(&org_id, &session_id, &target_node_id)
            .await
            .map_err(|cleanup_err| {
                tracing::warn!(error = %cleanup_err, "shaving track row cleanup failed");
                cleanup_err
            });
        if let Err(cleanup_err) = git::delete_ref(&git_root, &ref_name).await {
            tracing::warn!(error = %cleanup_err, ref_name = %ref_name, "shaving ref cleanup failed");
        }
        return Err(e.into());
    }

    Ok(())
}

async fn build_steps(
    git_root: &Path,
    worktree: &Path,
    input: &AttachTrackInput,
    changes: &[TreeChange],
) -> Result<Vec<ShavingStep>> {
    let mut steps = Vec::with_capacity(changes.len());
    let mut parent_commit = input.parent_commit_sha.clone();
    for (idx, change) in changes.iter().enumerate() {
        apply_change(worktree, &input.candidate_tree_sha, change).await?;
        git::run_in(worktree, &["add", "-A"]).await?;
        let tree_sha = git::write_tree(worktree).await?;
        let message = format!("eunomio shaving {}/{}", idx + 1, changes.len());
        let commit_sha = git::commit_tree(git_root, &tree_sha, &[&parent_commit], &message).await?;
        parent_commit = commit_sha.clone();
        steps.push(ShavingStep {
            tree_sha,
            commit_sha,
            label: None,
        });
    }
    Ok(steps)
}

async fn apply_change(worktree: &Path, candidate_tree: &str, change: &TreeChange) -> Result<()> {
    match change.status {
        TreeChangeStatus::Added
        | TreeChangeStatus::Modified
        | TreeChangeStatus::Copied
        | TreeChangeStatus::TypeChanged => {
            let path = change
                .new_path
                .as_deref()
                .ok_or_else(|| anyhow!("tree change missing new path"))?;
            git::run_in(worktree, &["checkout", candidate_tree, "--", path]).await?;
        }
        TreeChangeStatus::Deleted => {
            let path = change
                .old_path
                .as_deref()
                .ok_or_else(|| anyhow!("deleted tree change missing old path"))?;
            git::run_in(
                worktree,
                &["rm", "-r", "-f", "--ignore-unmatch", "--", path],
            )
            .await?;
        }
        TreeChangeStatus::Renamed => {
            let old_path = change
                .old_path
                .as_deref()
                .ok_or_else(|| anyhow!("renamed tree change missing old path"))?;
            let new_path = change
                .new_path
                .as_deref()
                .ok_or_else(|| anyhow!("renamed tree change missing new path"))?;
            if old_path != new_path {
                git::run_in(
                    worktree,
                    &["rm", "-r", "-f", "--ignore-unmatch", "--", old_path],
                )
                .await?;
            }
            git::run_in(worktree, &["checkout", candidate_tree, "--", new_path]).await?;
        }
    }
    Ok(())
}

fn display_path(change: &TreeChange) -> &str {
    change
        .new_path
        .as_deref()
        .or(change.old_path.as_deref())
        .unwrap_or("")
}

async fn provision_shaving_worktree(
    repo_root: &Path,
    data_dir: &Path,
    org_id: &str,
    session_id: &str,
    parent_commit: &str,
) -> Result<PathBuf> {
    let id = Uuid::new_v4().to_string();
    let worktree_path =
        storage_path::generated_worktree_path(data_dir, org_id, session_id, "shaving-gen", &id);
    if let Some(parent_dir) = worktree_path.parent() {
        tokio::fs::create_dir_all(parent_dir)
            .await
            .with_context(|| format!("create worktree parent {}", parent_dir.display()))?;
    }
    git::worktree_add(repo_root, &worktree_path, parent_commit).await?;
    Ok(worktree_path)
}

async fn cleanup_worktree(repo_root: &Path, worktree: &Path) {
    if let Err(e) = teardown_shaving_worktree(repo_root, worktree).await {
        tracing::warn!(error = %e, worktree = %worktree.display(), "shaving worktree cleanup failed");
    }
}

async fn teardown_shaving_worktree(repo_root: &Path, worktree: &Path) -> Result<()> {
    if worktree.exists() {
        git::worktree_remove(repo_root, worktree).await?;
    }
    if let Some(parent_dir) = worktree.parent() {
        match tokio::fs::remove_dir_all(parent_dir).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("remove {}", parent_dir.display())),
        }
    }
    Ok(())
}

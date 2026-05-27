// SPDX-License-Identifier: Apache-2.0

use crate::{git, repo_store, state::AppState, AppError};
use eunomio_core::types::*;
use std::path::Path;
use uuid::Uuid;

pub struct CreatedSession {
    pub id: String,
    pub base_node_id: String,
    pub created_at: i64,
}

pub enum CreateOutcome {
    Created,
    Existed,
}

pub async fn validate(_state: &AppState, dto: &CreateSessionRequest) -> Result<(), AppError> {
    let parsed = repo_store::parse_remote_url(&dto.remote_url)?;
    if parsed.is_local {
        git::ensure_repo(Path::new(&parsed.literal_remote))
            .await
            .map_err(|e| {
                AppError::BadRequest(format!(
                    "{} is not a git repository: {e}",
                    parsed.literal_remote
                ))
            })?;
    }
    if dto.base_ref.trim().is_empty() {
        return Err(AppError::BadRequest("baseRef is required".into()));
    }
    if dto.source_ref.trim().is_empty() {
        return Err(AppError::BadRequest("sourceRef is required".into()));
    }
    Ok(())
}

pub async fn create(
    state: &AppState,
    org_id: &str,
    user_id: &str,
    dto: CreateSessionRequest,
) -> Result<(Session, CreateOutcome), AppError> {
    let CreateSessionRequest {
        remote_url,
        base_ref,
        source_ref,
    } = dto;

    let (parsed, seed) = prepare_seed(
        state,
        org_id,
        &CreateSessionRequest {
            remote_url: remote_url.clone(),
            base_ref: base_ref.clone(),
            source_ref: source_ref.clone(),
        },
    )
    .await?;

    if let Some(existing) = state
        .datastore
        .sessions()
        .find_by_snapshot(
            org_id,
            &parsed.normalized_remote,
            &base_ref,
            &source_ref,
            &seed.resolved_base_commit,
            &seed.resolved_source_commit,
        )
        .await?
    {
        return Ok((existing, CreateOutcome::Existed));
    }

    let session_id = Uuid::new_v4().to_string();
    let base_node_id = Uuid::new_v4().to_string();
    let final_node_id = Uuid::new_v4().to_string();
    let now = eunomio_core::unix_seconds();

    state
        .datastore
        .sessions()
        .insert_seed_nodes(
            org_id.to_string(),
            user_id.to_string(),
            session_id.clone(),
            parsed.normalized_remote.clone(),
            parsed.literal_remote.clone(),
            base_ref.clone(),
            source_ref.clone(),
            seed.resolved_base_commit,
            seed.resolved_source_commit,
            seed.base_tree,
            seed.final_tree,
            base_node_id.clone(),
            final_node_id,
            seed.base_commit,
            seed.final_commit,
            now,
        )
        .await?;

    let session = state
        .datastore
        .sessions()
        .get(org_id, &session_id)
        .await?
        .ok_or(AppError::Internal(anyhow::anyhow!(
            "session missing after insert"
        )))?;

    Ok((session, CreateOutcome::Created))
}

pub async fn delete(state: &AppState, org_id: &str, session_id: &str) -> Result<(), AppError> {
    let fields = state
        .datastore
        .sessions()
        .repo_fields(org_id, session_id)
        .await?;
    let git_root = crate::repo_store::session_git_root(state, org_id, session_id).await?;

    let partition_worktrees = state
        .datastore
        .sessions()
        .list_partition_worktrees(org_id, session_id)
        .await?;
    for wt_path in &partition_worktrees {
        let path = std::path::PathBuf::from(wt_path);
        crate::worktree::teardown(&git_root, &path).await;
    }

    state
        .datastore
        .sessions()
        .delete_cascade(org_id, session_id)
        .await?;

    let remaining = state
        .datastore
        .sessions()
        .count_for_normalized(org_id, &fields.normalized_remote)
        .await?;
    repo_store::maybe_remove_clone(
        &state.data_dir,
        org_id,
        &fields.normalized_remote,
        remaining,
    )
    .await?;

    Ok(())
}

async fn prepare_seed(
    state: &AppState,
    org_id: &str,
    dto: &CreateSessionRequest,
) -> Result<(repo_store::ParsedRemote, SessionSeed), AppError> {
    let parsed = repo_store::parse_remote_url(&dto.remote_url)?;
    let git_root = repo_store::materialize_git_root(&state.data_dir, org_id, &parsed).await?;
    let seed = seed_session(&git_root, &dto.base_ref, &dto.source_ref).await?;
    Ok((parsed, seed))
}

struct SessionSeed {
    resolved_base_commit: String,
    resolved_source_commit: String,
    base_tree: String,
    final_tree: String,
    base_commit: String,
    final_commit: String,
}

async fn seed_session(
    git_root: &Path,
    base_ref: &str,
    source_ref: &str,
) -> Result<SessionSeed, AppError> {
    let resolved_source = git::resolve_ref_name(git_root, source_ref)
        .await
        .map_err(|e| AppError::BadRequest(format!("rev-parse {source_ref} failed: {e}")))?;

    let mb = git::merge_base(git_root, base_ref, &resolved_source)
        .await
        .map_err(|e| AppError::BadRequest(format!("merge-base failed: {e}")))?;

    let source_commit = git::rev_parse(git_root, &resolved_source)
        .await
        .map_err(|e| AppError::BadRequest(format!("rev-parse {source_ref} failed: {e}")))?;
    if source_commit == mb {
        return Err(AppError::BadRequest(
            "merge-base equals sourceRef; nothing to review".into(),
        ));
    }

    let base_tree = git::rev_parse(git_root, &format!("{mb}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rev-parse base tree: {e}")))?;
    let final_tree = git::rev_parse(git_root, &format!("{resolved_source}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rev-parse final tree: {e}")))?;

    let base_commit = git::commit_tree(git_root, &base_tree, &[], "base")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree base: {e}")))?;
    let final_commit = git::commit_tree(git_root, &final_tree, &[&base_commit], "final")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree final: {e}")))?;

    Ok(SessionSeed {
        resolved_base_commit: mb,
        resolved_source_commit: source_commit,
        base_tree,
        final_tree,
        base_commit,
        final_commit,
    })
}

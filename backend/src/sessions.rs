use crate::{db, error::AppError, git, repo, state::AppState, types::*};
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

pub async fn create(
    state: &AppState,
    dto: CreateSessionRequest,
) -> Result<(CreatedSession, CreateOutcome), AppError> {
    let CreateSessionRequest { base_ref, source_ref } = dto;

    if let Some(existing) = repo::session::find_by_refs(state, &base_ref, &source_ref).await? {
        return Ok((
            CreatedSession {
                id: existing.id,
                base_node_id: existing.base_node_id,
                created_at: existing.created_at,
            },
            CreateOutcome::Existed,
        ));
    }

    let seed = seed_session(&state.repo_root, &base_ref, &source_ref).await?;

    let session_id = Uuid::new_v4().to_string();
    let base_node_id = Uuid::new_v4().to_string();
    let final_node_id = Uuid::new_v4().to_string();
    let now = db::unix_seconds();

    repo::session::insert_seed_nodes(
        state,
        session_id.clone(),
        base_ref,
        source_ref,
        seed.base_tree,
        seed.final_tree,
        base_node_id.clone(),
        final_node_id,
        seed.base_commit,
        seed.final_commit,
        now,
    )
    .await?;

    Ok((
        CreatedSession {
            id: session_id,
            base_node_id,
            created_at: now,
        },
        CreateOutcome::Created,
    ))
}

pub async fn delete(state: &AppState, session_id: &str) -> Result<(), AppError> {
    repo::session::ensure(state, session_id).await?;

    let partition_worktrees = repo::session::list_partition_worktrees(state, session_id).await?;
    for wt_path in &partition_worktrees {
        let path = std::path::PathBuf::from(wt_path);
        crate::worktree::teardown(&state.repo_root, &path).await;
    }

    repo::session::delete_cascade(state, session_id).await?;
    Ok(())
}

struct SessionSeed {
    base_tree: String,
    final_tree: String,
    base_commit: String,
    final_commit: String,
}

async fn seed_session(
    repo_root: &Path,
    base_ref: &str,
    source_ref: &str,
) -> Result<SessionSeed, AppError> {
    let mb = git::merge_base(repo_root, base_ref, source_ref)
        .await
        .map_err(|e| AppError::BadRequest(format!("merge-base failed: {e}")))?;

    let source_commit = git::rev_parse(repo_root, source_ref)
        .await
        .map_err(|e| AppError::BadRequest(format!("rev-parse {source_ref} failed: {e}")))?;
    if source_commit == mb {
        return Err(AppError::BadRequest(
            "merge-base equals sourceRef; nothing to review".into(),
        ));
    }

    let base_tree = git::rev_parse(repo_root, &format!("{mb}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rev-parse base tree: {e}")))?;
    let final_tree = git::rev_parse(repo_root, &format!("{source_ref}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rev-parse final tree: {e}")))?;

    let base_commit = git::commit_tree(repo_root, &base_tree, &[], "base")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree base: {e}")))?;
    let final_commit = git::commit_tree(repo_root, &final_tree, &[&base_commit], "final")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree final: {e}")))?;

    Ok(SessionSeed {
        base_tree,
        final_tree,
        base_commit,
        final_commit,
    })
}

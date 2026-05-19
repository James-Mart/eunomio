use crate::{error::AppError, git, server::AppState, types::*};
use std::time::{SystemTime, UNIX_EPOCH};
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

    let repo_root_str = state.repo_root.to_string_lossy().to_string();
    let base_ref_lookup = base_ref.clone();
    let source_ref_lookup = source_ref.clone();
    let existing: Option<CreatedSession> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_node_id, created_at FROM sessions \
                 WHERE repo_root = ?1 AND base_ref = ?2 AND source_ref = ?3 \
                 LIMIT 1",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![
                repo_root_str,
                base_ref_lookup,
                source_ref_lookup
            ])?;
            if let Some(row) = rows.next()? {
                Ok(Some(CreatedSession {
                    id: row.get(0)?,
                    base_node_id: row.get(1)?,
                    created_at: row.get(2)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    if let Some(existing) = existing {
        return Ok((existing, CreateOutcome::Existed));
    }

    let mb = git::merge_base(&state.repo_root, &base_ref, &source_ref)
        .await
        .map_err(|e| AppError::BadRequest(format!("merge-base failed: {e}")))?;

    let source_commit = git::rev_parse(&state.repo_root, &source_ref)
        .await
        .map_err(|e| AppError::BadRequest(format!("rev-parse {source_ref} failed: {e}")))?;
    if source_commit == mb {
        return Err(AppError::BadRequest(
            "merge-base equals sourceRef; nothing to review".into(),
        ));
    }

    let base_tree = git::rev_parse_tree(&state.repo_root, &format!("{mb}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rev-parse base tree: {e}")))?;
    let final_tree = git::rev_parse_tree(&state.repo_root, &format!("{source_ref}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rev-parse final tree: {e}")))?;

    let base_commit = git::commit_tree(&state.repo_root, &base_tree, &[], "base")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree base: {e}")))?;
    let final_commit = git::commit_tree(&state.repo_root, &final_tree, &[&base_commit], "final")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree final: {e}")))?;

    let session_id = Uuid::new_v4().to_string();
    let base_node_id = Uuid::new_v4().to_string();
    let final_node_id = Uuid::new_v4().to_string();
    let now = unix_seconds();

    let repo_root = state.repo_root.to_string_lossy().to_string();
    let session_id_for_db = session_id.clone();
    let base_node_id_for_db = base_node_id.clone();
    let final_node_id_for_db = final_node_id.clone();
    let base_ref_for_db = base_ref.clone();
    let source_ref_for_db = source_ref.clone();

    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions (id, repo_root, base_ref, source_ref, base_tree, final_tree, base_node_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                tokio_rusqlite::params![
                    session_id_for_db,
                    repo_root,
                    base_ref_for_db,
                    source_ref_for_db,
                    base_tree,
                    final_tree,
                    base_node_id_for_db,
                    now
                ],
            )?;
            tx.execute(
                "INSERT INTO nodes (session_id, node_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                 VALUES (?1, ?2, NULL, ?3, ?4, 'base', ?5)",
                tokio_rusqlite::params![
                    session_id_for_db,
                    base_node_id_for_db,
                    base_tree,
                    base_commit,
                    now
                ],
            )?;
            tx.execute(
                "INSERT INTO nodes (session_id, node_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'final', ?6)",
                tokio_rusqlite::params![
                    session_id_for_db,
                    final_node_id_for_db,
                    base_node_id_for_db,
                    final_tree,
                    final_commit,
                    now
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
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
    let repo_root_str = state.repo_root.to_string_lossy().to_string();
    let id_for_lookup = session_id.to_string();
    let session_exists: bool = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT 1 FROM sessions WHERE id = ?1 AND repo_root = ?2")?;
            let mut rows = stmt.query(tokio_rusqlite::params![id_for_lookup, repo_root_str])?;
            Ok(rows.next()?.is_some())
        })
        .await?;
    if !session_exists {
        return Err(AppError::NotFound);
    }

    let id_for_partitions = session_id.to_string();
    let partition_worktrees: Vec<String> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT worktree_path FROM partitions WHERE session_id = ?1")?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![id_for_partitions], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    for wt_path in &partition_worktrees {
        let path = std::path::PathBuf::from(wt_path);
        if path.exists() {
            if let Err(e) = git::worktree_remove(&state.repo_root, &path).await {
                tracing::warn!(error = %e, worktree = %wt_path, "removing partition worktree failed");
            }
        }
    }

    let id_for_delete = session_id.to_string();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM runs WHERE session_id = ?1",
                tokio_rusqlite::params![id_for_delete],
            )?;
            tx.execute(
                "DELETE FROM partitions WHERE session_id = ?1",
                tokio_rusqlite::params![id_for_delete],
            )?;
            tx.execute(
                "DELETE FROM nodes WHERE session_id = ?1",
                tokio_rusqlite::params![id_for_delete],
            )?;
            tx.execute(
                "DELETE FROM sessions WHERE id = ?1",
                tokio_rusqlite::params![id_for_delete],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;

    Ok(())
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

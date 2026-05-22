use crate::{error::AppError, git, repo_store, state::AppState, types::*};

pub struct CreatedSessionRow {
    pub id: String,
    pub base_node_id: String,
    pub created_at: i64,
}

pub struct SessionRepoFields {
    pub normalized_remote: String,
    pub literal_remote: String,
    pub is_local: bool,
}

pub async fn exists(state: &AppState, session_id: &str) -> Result<bool, AppError> {
    let session_id = session_id.to_string();
    let exists = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare("SELECT 1 FROM sessions WHERE id = ?1")?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
            Ok(rows.next()?.is_some())
        })
        .await?;
    Ok(exists)
}

pub async fn ensure(state: &AppState, session_id: &str) -> Result<(), AppError> {
    if exists(state, session_id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

pub async fn repo_fields(
    state: &AppState,
    session_id: &str,
) -> Result<SessionRepoFields, AppError> {
    let session_id = session_id.to_string();
    let row = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT normalized_remote, literal_remote, is_local FROM sessions WHERE id = ?1",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(SessionRepoFields {
                    normalized_remote: row.get(0)?,
                    literal_remote: row.get(1)?,
                    is_local: row.get::<_, i64>(2)? != 0,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    row.ok_or(AppError::NotFound)
}

pub async fn git_root(state: &AppState, session_id: &str) -> Result<std::path::PathBuf, AppError> {
    let fields = repo_fields(state, session_id).await?;
    Ok(repo_store::git_root(
        &state.data_dir,
        &repo_store::ParsedRemote {
            literal_remote: fields.literal_remote,
            is_local: fields.is_local,
            normalized_remote: fields.normalized_remote,
        },
    ))
}

pub async fn list(state: &AppState) -> Result<Vec<Session>, AppError> {
    let rows: Vec<Session> = state
        .db
        .call(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_node_id, created_at \
                 FROM sessions ORDER BY created_at DESC",
            )?;
            let rows = stmt
                .query_map([], session_row_mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn get(state: &AppState, session_id: &str) -> Result<Option<Session>, AppError> {
    let session_id = session_id.to_string();
    let row: Option<Session> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_node_id, created_at \
                 FROM sessions WHERE id = ?1",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(session_row_mapper(row)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(row)
}

pub async fn final_tree(state: &AppState, session_id: &str) -> Result<Option<String>, AppError> {
    let session_id = session_id.to_string();
    let row: Option<String> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT final_tree FROM sessions WHERE id = ?1")?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get::<_, String>(0)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(row)
}

pub async fn base_tree(state: &AppState, session_id: &str) -> Result<Option<String>, AppError> {
    let session_id = session_id.to_string();
    let row: Option<String> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare("SELECT base_tree FROM sessions WHERE id = ?1")?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get::<_, String>(0)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(row)
}

pub async fn seed_trees(
    state: &AppState,
    session_id: &str,
) -> Result<Option<(String, String)>, AppError> {
    let session_id = session_id.to_string();
    let row: Option<(String, String)> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT base_tree, final_tree FROM sessions WHERE id = ?1")?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some((row.get(0)?, row.get(1)?)))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(row)
}

pub async fn find_by_refs(
    state: &AppState,
    normalized_remote: &str,
    base_ref: &str,
    source_ref: &str,
) -> Result<Option<CreatedSessionRow>, AppError> {
    let normalized_remote = normalized_remote.to_string();
    let base_ref = base_ref.to_string();
    let source_ref = source_ref.to_string();
    let existing = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_node_id, created_at FROM sessions \
                 WHERE normalized_remote = ?1 AND base_ref = ?2 AND source_ref = ?3 \
                 LIMIT 1",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![
                normalized_remote,
                base_ref,
                source_ref
            ])?;
            if let Some(row) = rows.next()? {
                Ok(Some(CreatedSessionRow {
                    id: row.get(0)?,
                    base_node_id: row.get(1)?,
                    created_at: row.get(2)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(existing)
}

pub async fn count_for_normalized(
    state: &AppState,
    normalized_remote: &str,
) -> Result<i64, AppError> {
    let normalized_remote = normalized_remote.to_string();
    let count: i64 = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT COUNT(*) FROM sessions WHERE normalized_remote = ?1")?;
            let mut rows = stmt.query(tokio_rusqlite::params![normalized_remote])?;
            let row = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            let count: i64 = row.get(0)?;
            Ok(count)
        })
        .await?;
    Ok(count)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_seed_nodes(
    state: &AppState,
    session_id: String,
    normalized_remote: String,
    literal_remote: String,
    is_local: bool,
    base_ref: String,
    source_ref: String,
    base_tree: String,
    final_tree: String,
    base_node_id: String,
    final_node_id: String,
    base_commit: String,
    final_commit: String,
    now: i64,
) -> Result<(), AppError> {
    let is_local_int = i64::from(is_local);
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions (id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_tree, final_tree, base_node_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                tokio_rusqlite::params![
                    session_id,
                    normalized_remote,
                    literal_remote,
                    is_local_int,
                    base_ref,
                    source_ref,
                    base_tree,
                    final_tree,
                    base_node_id,
                    now
                ],
            )?;
            tx.execute(
                "INSERT INTO nodes (session_id, node_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                 VALUES (?1, ?2, NULL, ?3, ?4, 'base', ?5)",
                tokio_rusqlite::params![session_id, base_node_id, base_tree, base_commit, now],
            )?;
            tx.execute(
                "INSERT INTO nodes (session_id, node_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'final', ?6)",
                tokio_rusqlite::params![
                    session_id, final_node_id, base_node_id, final_tree, final_commit, now
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn list_partition_worktrees(
    state: &AppState,
    session_id: &str,
) -> Result<Vec<String>, AppError> {
    let session_id = session_id.to_string();
    let rows: Vec<String> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT worktree_path FROM partitions WHERE session_id = ?1")?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![session_id], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn delete_cascade(state: &AppState, session_id: &str) -> Result<(), AppError> {
    let session_id = session_id.to_string();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM runs WHERE session_id = ?1",
                tokio_rusqlite::params![session_id],
            )?;
            tx.execute(
                "DELETE FROM partitions WHERE session_id = ?1",
                tokio_rusqlite::params![session_id],
            )?;
            tx.execute(
                "DELETE FROM nodes WHERE session_id = ?1",
                tokio_rusqlite::params![session_id],
            )?;
            tx.execute(
                "DELETE FROM sessions WHERE id = ?1",
                tokio_rusqlite::params![session_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

fn session_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let normalized_remote: String = row.get(1)?;
    let literal_remote: String = row.get(2)?;
    let is_local: bool = row.get::<_, i64>(3)? != 0;
    let (repo_owner, repo_name) =
        git::repo_display_parts(&normalized_remote, is_local, &literal_remote);
    Ok(Session {
        id: row.get(0)?,
        normalized_remote,
        literal_remote,
        is_local,
        repo_owner,
        repo_name,
        base_ref: row.get(4)?,
        source_ref: row.get(5)?,
        base_node_id: row.get(6)?,
        created_at: row.get(7)?,
    })
}

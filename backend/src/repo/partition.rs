use crate::{error::AppError, state::AppState, types::*};

#[derive(Debug, Clone)]
pub struct SiblingInfo {
    pub id: i64,
    pub target_node_id: String,
    pub worktree_path: String,
}

pub async fn get(state: &AppState, partition_id: i64) -> Result<PartitionRow, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let row: Option<PartitionRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.session_id, p.target_node_id, p.strategy, p.change_survey_json, p.plan_json, p.candidate_slice_tree_sha, p.candidate_slice_commit_sha, p.phase, p.phase_state, p.worktree_path, p.remaining_depth, p.created_at \
                 FROM partitions p JOIN sessions s ON s.id = p.session_id \
                 WHERE p.id = ?1 AND s.repo_root = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![partition_id, repo_root])?;
            if let Some(r) = rows.next()? {
                Ok(Some(partition_row_mapper(r)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    row.ok_or(AppError::NotFound)
}

pub async fn list(
    state: &AppState,
    session_id: &str,
    target_node_id: Option<&str>,
) -> Result<Vec<PartitionRow>, AppError> {
    let session_id = session_id.to_string();
    let target_owned = target_node_id.map(|s| s.to_string());
    let rows: Vec<PartitionRow> = state
        .db
        .call(move |conn| {
            let (sql, has_filter) = match &target_owned {
                Some(_) => (
                    "SELECT id, session_id, target_node_id, strategy, change_survey_json, plan_json, candidate_slice_tree_sha, candidate_slice_commit_sha, phase, phase_state, worktree_path, remaining_depth, created_at \
                     FROM partitions WHERE session_id = ?1 AND target_node_id = ?2 ORDER BY created_at",
                    true,
                ),
                None => (
                    "SELECT id, session_id, target_node_id, strategy, change_survey_json, plan_json, candidate_slice_tree_sha, candidate_slice_commit_sha, phase, phase_state, worktree_path, remaining_depth, created_at \
                     FROM partitions WHERE session_id = ?1 ORDER BY created_at",
                    false,
                ),
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = if has_filter {
                let target = target_owned.unwrap();
                stmt.query_map(tokio_rusqlite::params![session_id, target], partition_row_mapper)?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(tokio_rusqlite::params![session_id], partition_row_mapper)?
                    .collect::<Result<Vec<_>, _>>()?
            };
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn list_siblings(
    state: &AppState,
    session_id: &str,
    target_node_id: &str,
    accepted_partition_id: i64,
) -> Result<Vec<SiblingInfo>, AppError> {
    let session_id = session_id.to_string();
    let target_node_id = target_node_id.to_string();
    let rows: Vec<SiblingInfo> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, target_node_id, worktree_path FROM partitions \
                 WHERE session_id = ?1 AND target_node_id = ?2 AND id != ?3",
            )?;
            let rows = stmt
                .query_map(
                    tokio_rusqlite::params![session_id, target_node_id, accepted_partition_id],
                    |r| {
                        Ok(SiblingInfo {
                            id: r.get(0)?,
                            target_node_id: r.get(1)?,
                            worktree_path: r.get(2)?,
                        })
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn delete(state: &AppState, partition_id: i64) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            conn.execute(
                "DELETE FROM partitions WHERE id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn delete_with_runs(state: &AppState, partition_id: i64) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM runs WHERE partition_id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            tx.execute(
                "DELETE FROM partitions WHERE id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn delete_many_with_runs(
    state: &AppState,
    partition_ids: Vec<i64>,
) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            for id in &partition_ids {
                tx.execute(
                    "DELETE FROM runs WHERE partition_id = ?1",
                    tokio_rusqlite::params![id],
                )?;
                tx.execute(
                    "DELETE FROM partitions WHERE id = ?1",
                    tokio_rusqlite::params![id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn insert_pending(
    state: &AppState,
    session_id: String,
    target_node_id: String,
    worktree_path: String,
    remaining_depth: Option<i64>,
    now: i64,
) -> Result<i64, AppError> {
    let id: i64 = state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO partitions (session_id, target_node_id, phase, phase_state, worktree_path, remaining_depth, created_at) \
                 VALUES (?1, ?2, 'survey', 'running', ?3, ?4, ?5)",
                tokio_rusqlite::params![session_id, target_node_id, worktree_path, remaining_depth, now],
            )?;
            let id = tx.last_insert_rowid();
            tx.commit()?;
            Ok(id)
        })
        .await?;
    Ok(id)
}

pub async fn clear_plan_and_slice(
    state: &AppState,
    partition_id: i64,
) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE partitions SET plan_json = NULL, strategy = NULL, candidate_slice_tree_sha = NULL, candidate_slice_commit_sha = NULL WHERE id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn accept_survey(
    state: &AppState,
    partition_id: i64,
    change_survey_json: String,
) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE partitions SET change_survey_json = ?1, phase = 'plan', phase_state = 'running' WHERE id = ?2",
                tokio_rusqlite::params![change_survey_json, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Atomically accepts the candidate slice produced by `partition_id`:
/// inserts the new `slice` node, re-parents the original target onto it,
/// and deletes all sibling partitions (plus their runs) along with the
/// accepted partition itself.
#[allow(clippy::too_many_arguments)]
pub async fn finalize_construct_accept(
    state: &AppState,
    session_id: String,
    partition_id: i64,
    target_node_id: String,
    slice_node_id: String,
    parent_node_id: String,
    candidate_tree: String,
    candidate_commit: String,
    slice_title: String,
    slice_description: String,
    leftover_title: String,
    leftover_description: String,
    sibling_ids: Vec<i64>,
    now: i64,
) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO nodes (session_id, node_id, parent_node_id, tree_sha, commit_sha, title, description, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                tokio_rusqlite::params![
                    session_id,
                    slice_node_id,
                    parent_node_id,
                    candidate_tree,
                    candidate_commit,
                    slice_title,
                    slice_description,
                    now
                ],
            )?;
            tx.execute(
                "UPDATE nodes SET parent_node_id = ?1, title = ?2, description = ?3 WHERE session_id = ?4 AND node_id = ?5",
                tokio_rusqlite::params![
                    slice_node_id,
                    leftover_title,
                    leftover_description,
                    session_id,
                    target_node_id
                ],
            )?;
            let mut all_ids = sibling_ids.clone();
            all_ids.push(partition_id);
            for id in &all_ids {
                tx.execute(
                    "DELETE FROM runs WHERE partition_id = ?1",
                    tokio_rusqlite::params![id],
                )?;
                tx.execute(
                    "DELETE FROM partitions WHERE id = ?1",
                    tokio_rusqlite::params![id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn accept_plan(
    state: &AppState,
    partition_id: i64,
    plan_json: String,
    strategy: PartitionStrategy,
) -> Result<(), AppError> {
    let strategy_str = strategy.as_str().to_string();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE partitions SET plan_json = ?1, strategy = ?2, phase = 'construct', phase_state = 'running' WHERE id = ?3",
                tokio_rusqlite::params![plan_json, strategy_str, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn set_phase_state(
    state: &AppState,
    partition_id: i64,
    phase_state: PhaseState,
) -> Result<(), AppError> {
    let s = phase_state.as_str().to_string();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE partitions SET phase_state = ?1 WHERE id = ?2",
                tokio_rusqlite::params![s, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn set_phase_running(
    state: &AppState,
    partition_id: i64,
    phase: PhaseName,
) -> Result<(), AppError> {
    let phase_str = phase.as_str().to_string();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE partitions SET phase = ?1, phase_state = 'running' WHERE id = ?2",
                tokio_rusqlite::params![phase_str, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn set_worktree_path(
    state: &AppState,
    partition_id: i64,
    worktree_path: String,
) -> Result<(), AppError> {
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE partitions SET worktree_path = ?1 WHERE id = ?2",
                tokio_rusqlite::params![worktree_path, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Atomically records a successful construct run together with the candidate
/// slice it produced, leaving the partition in `construct/running` so the
/// run-loop can then transition to `awaiting_review`.
#[allow(clippy::too_many_arguments)]
pub async fn accept_constructor_ok(
    state: &AppState,
    partition_id: i64,
    tree_sha: String,
    commit_sha: String,
    run_id: i64,
    result_json: String,
    result_text: String,
) -> Result<(), AppError> {
    let now = crate::db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE partitions SET candidate_slice_tree_sha = ?1, candidate_slice_commit_sha = ?2, phase = 'construct', phase_state = 'running' WHERE id = ?3",
                tokio_rusqlite::params![tree_sha, commit_sha, partition_id],
            )?;
            tx.execute(
                "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                tokio_rusqlite::params![result_json, result_text, now, run_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Atomically records a constructor `blocked` outcome and parks the
/// partition at `construct/awaiting_review`.
pub async fn accept_constructor_blocked(
    state: &AppState,
    partition_id: i64,
    run_id: i64,
    result_json: String,
    result_text: String,
) -> Result<(), AppError> {
    let now = crate::db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE partitions SET phase = 'construct', phase_state = 'awaiting_review' WHERE id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            tx.execute(
                "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                tokio_rusqlite::params![result_json, result_text, now, run_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Atomically records a run failure (`error`) and flips the owning
/// partition into `phase_state = error`, used for both the cancel and
/// finalize-error paths.
pub async fn fail_run(
    state: &AppState,
    partition_id: i64,
    run_id: i64,
    error_message: String,
    result_text: Option<String>,
) -> Result<(), AppError> {
    let now = crate::db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE runs SET status = 'error', error_message = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                tokio_rusqlite::params![error_message, result_text, now, run_id],
            )?;
            tx.execute(
                "UPDATE partitions SET phase_state = 'error' WHERE id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Atomically cancels a running run and flips its partition into
/// `phase_state = error` (used by the `DELETE /runs/:id` handler).
pub async fn cancel_run(
    state: &AppState,
    partition_id: i64,
    run_id: i64,
) -> Result<(), AppError> {
    let now = crate::db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
                tokio_rusqlite::params![now, run_id],
            )?;
            tx.execute(
                "UPDATE partitions SET phase_state = 'error' WHERE id = ?1",
                tokio_rusqlite::params![partition_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Returns `(id, session_id, worktree_path)` for every partition row.
///
/// Used by startup recovery to align the on-disk worktree directory
/// with the database state without loading full `PartitionRow`s.
pub async fn list_id_session_worktree(
    state: &AppState,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let rows: Vec<(i64, String, String)> = state
        .db
        .call(|conn| {
            let mut stmt = conn.prepare("SELECT id, session_id, worktree_path FROM partitions")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub fn partition_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<PartitionRow> {
    Ok(PartitionRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        target_node_id: row.get(2)?,
        strategy: row
            .get::<_, Option<String>>(3)?
            .and_then(|s| PartitionStrategy::parse(&s)),
        change_survey_json: row.get(4)?,
        plan_json: row.get(5)?,
        candidate_slice_tree_sha: row.get(6)?,
        candidate_slice_commit_sha: row.get(7)?,
        phase: PhaseName::parse(&row.get::<_, String>(8)?).unwrap_or(PhaseName::Survey),
        phase_state: PhaseState::parse(&row.get::<_, String>(9)?).unwrap_or(PhaseState::Error),
        worktree_path: row.get(10)?,
        remaining_depth: row.get(11)?,
        created_at: row.get(12)?,
    })
}

use crate::{db, error::AppError, state::AppState, types::*};

pub async fn get(state: &AppState, run_id: i64) -> Result<RunRow, AppError> {
    let row: Option<RunRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, partition_id, session_id, target_node_id, kind, parent_run_id, status, result_json, result_text, error_message, started_at, finished_at \
                 FROM runs WHERE id = ?1",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![run_id])?;
            if let Some(r) = rows.next()? {
                Ok(Some(run_row_mapper(r)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    row.ok_or(AppError::NotFound)
}

pub async fn list_for_partition(
    state: &AppState,
    partition_id: i64,
) -> Result<Vec<RunRow>, AppError> {
    let rows: Vec<RunRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, partition_id, session_id, target_node_id, kind, parent_run_id, status, result_json, result_text, error_message, started_at, finished_at \
                 FROM runs WHERE partition_id = ?1 ORDER BY started_at DESC, id DESC",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![partition_id], run_row_mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn start(
    state: &AppState,
    partition_id: i64,
    session_id: String,
    target_node_id: String,
    kind: RunKind,
    parent_run_id: Option<i64>,
    started_at: i64,
) -> Result<i64, AppError> {
    let kind_str = kind.as_str().to_string();
    let run_id: i64 = state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO runs (partition_id, session_id, target_node_id, kind, parent_run_id, status, started_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6)",
                tokio_rusqlite::params![
                    partition_id,
                    session_id,
                    target_node_id,
                    kind_str,
                    parent_run_id,
                    started_at
                ],
            )?;
            let id = tx.last_insert_rowid();
            tx.commit()?;
            Ok(id)
        })
        .await?;
    Ok(run_id)
}

pub async fn finish_success(
    state: &AppState,
    run_id: i64,
    result_json: String,
    result_text: Option<String>,
) -> Result<(), AppError> {
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                tokio_rusqlite::params![result_json, result_text, now, run_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn finish_error(
    state: &AppState,
    run_id: i64,
    error_message: String,
) -> Result<(), AppError> {
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3",
                tokio_rusqlite::params![error_message, now, run_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

/// Returns the IDs of all runs currently in `running` status.
///
/// Used by startup recovery to identify runs that survived a process
/// restart so they can be marked as `error`/`process_restart`.
pub async fn list_running_ids(state: &AppState) -> Result<Vec<i64>, AppError> {
    let ids: Vec<i64> = state
        .db
        .call(|conn| {
            let mut stmt = conn.prepare("SELECT id FROM runs WHERE status = 'running'")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(ids)
}

/// Marks the given runs as `error` with the supplied message, in a
/// single transaction. Used by startup recovery.
pub async fn mark_errored(
    state: &AppState,
    run_ids: Vec<i64>,
    error_message: &'static str,
) -> Result<(), AppError> {
    if run_ids.is_empty() {
        return Ok(());
    }
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            for id in &run_ids {
                tx.execute(
                    "UPDATE runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3",
                    tokio_rusqlite::params![error_message, now, id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn cancel_running_for_partition(
    state: &AppState,
    partition_id: i64,
) -> Result<(), AppError> {
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE partition_id = ?2 AND status = 'running'",
                tokio_rusqlite::params![now, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn cancel(state: &AppState, run_id: i64) -> Result<(), AppError> {
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE id = ?2 AND status = 'running'",
                tokio_rusqlite::params![now, run_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub fn run_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRow> {
    Ok(RunRow {
        id: row.get(0)?,
        partition_id: row.get(1)?,
        session_id: row.get(2)?,
        target_node_id: row.get(3)?,
        kind: RunKind::parse(&row.get::<_, String>(4)?).unwrap_or(RunKind::Survey),
        parent_run_id: row.get(5)?,
        status: RunStatus::parse(&row.get::<_, String>(6)?).unwrap_or(RunStatus::Error),
        result_json: row.get(7)?,
        result_text: row.get(8)?,
        error_message: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
    })
}

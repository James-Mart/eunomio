use crate::{db, error::AppError, state::AppState, types::*};
use super::{require_affected_sqlite, DbResultExt};
use uuid::Uuid;

const RUN_SELECT: &str = "SELECT id, partition_id, session_id, target_node_id, kind, parent_run_id, status, result_json, result_text, error_message, transcript_text, started_at, finished_at \
                 FROM runs";

pub async fn get(
    state: &AppState,
    org_id: &str,
    run_id: &str,
) -> Result<RunRow, AppError> {
    let org_id = org_id.to_string();
    let run_id = run_id.to_string();
    let row: Option<RunRow> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare(&format!("{RUN_SELECT} WHERE id = ?1 AND org_id = ?2"))?;
            let mut rows = stmt.query(tokio_rusqlite::params![run_id, org_id])?;
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
    org_id: &str,
    partition_id: &str,
) -> Result<Vec<RunRow>, AppError> {
    let org_id = org_id.to_string();
    let partition_id = partition_id.to_string();
    let rows: Vec<RunRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "{RUN_SELECT} WHERE org_id = ?1 AND partition_id = ?2 ORDER BY started_at DESC, id DESC"
            ))?;
            let rows = stmt
                .query_map(
                    tokio_rusqlite::params![org_id, partition_id],
                    run_row_mapper,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn start(
    state: &AppState,
    org_id: String,
    user_id: String,
    partition_id: String,
    session_id: String,
    target_node_id: String,
    kind: RunKind,
    parent_run_id: Option<String>,
    prompt_text: String,
    started_at: i64,
) -> Result<String, AppError> {
    let id = Uuid::new_v4().to_string();
    let run_id = id.clone();
    let kind_str = kind.as_str().to_string();
    let inserted_id = state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO runs (id, org_id, user_id, partition_id, session_id, target_node_id, kind, parent_run_id, status, prompt_text, started_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'running', ?9, ?10)",
                tokio_rusqlite::params![
                    run_id,
                    org_id,
                    user_id,
                    partition_id,
                    session_id,
                    target_node_id,
                    kind_str,
                    parent_run_id,
                    prompt_text,
                    started_at
                ],
            )?;
            tx.commit()?;
            Ok(id)
        })
        .await?;
    Ok(inserted_id)
}

pub async fn get_prompt(
    state: &AppState,
    org_id: &str,
    run_id: &str,
) -> Result<Option<String>, AppError> {
    let org_id = org_id.to_string();
    let run_id = run_id.to_string();
    let prompt = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT prompt_text FROM runs WHERE id = ?1 AND org_id = ?2")?;
            let mut rows = stmt.query(tokio_rusqlite::params![run_id, org_id])?;
            if let Some(r) = rows.next()? {
                Ok(r.get::<_, Option<String>>(0)?)
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(prompt)
}

pub async fn append_transcript_text(
    state: &AppState,
    org_id: &str,
    run_id: &str,
    chunk: &str,
) -> Result<(), AppError> {
    let org_id = org_id.to_string();
    let run_id = run_id.to_string();
    let chunk = chunk.to_string();
    state
        .db
        .call(move |conn| {
            let n = conn.execute(
                "UPDATE runs SET transcript_text = COALESCE(transcript_text, '') || ?1 WHERE id = ?2 AND org_id = ?3",
                tokio_rusqlite::params![chunk, run_id, org_id],
            )?;
            require_affected_sqlite(n)?;
            Ok(())
        })
        .await
        .map_not_found()?;
    Ok(())
}

pub async fn finish_success(
    state: &AppState,
    org_id: &str,
    run_id: &str,
    result_json: String,
    result_text: Option<String>,
) -> Result<(), AppError> {
    let org_id = org_id.to_string();
    let run_id = run_id.to_string();
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let n = conn.execute(
                "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4 AND org_id = ?5",
                tokio_rusqlite::params![result_json, result_text, now, run_id, org_id],
            )?;
            require_affected_sqlite(n)?;
            Ok(())
        })
        .await
        .map_not_found()?;
    Ok(())
}

pub async fn finish_error(
    state: &AppState,
    org_id: &str,
    run_id: &str,
    error_message: String,
) -> Result<(), AppError> {
    let org_id = org_id.to_string();
    let run_id = run_id.to_string();
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let n = conn.execute(
                "UPDATE runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3 AND org_id = ?4",
                tokio_rusqlite::params![error_message, now, run_id, org_id],
            )?;
            require_affected_sqlite(n)?;
            Ok(())
        })
        .await
        .map_not_found()?;
    Ok(())
}

/// Returns the IDs of all runs currently in `running` status.
///
/// Used by startup recovery to identify runs that survived a process
/// restart so they can be marked as `error`/`process_restart`.
pub async fn list_running_ids(
    state: &AppState,
    org_id: &str,
) -> Result<Vec<String>, AppError> {
    let org_id = org_id.to_string();
    let ids: Vec<String> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT id FROM runs WHERE org_id = ?1 AND status = 'running'")?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![org_id], |row| row.get::<_, String>(0))?
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
    org_id: &str,
    run_ids: Vec<String>,
    error_message: &'static str,
) -> Result<(), AppError> {
    if run_ids.is_empty() {
        return Ok(());
    }
    let org_id = org_id.to_string();
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            for id in &run_ids {
                tx.execute(
                    "UPDATE runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3 AND org_id = ?4",
                    tokio_rusqlite::params![error_message, now, id, org_id],
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
    org_id: &str,
    partition_id: &str,
) -> Result<(), AppError> {
    super::partition::get(state, org_id, partition_id).await?;
    let org_id = org_id.to_string();
    let partition_id = partition_id.to_string();
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE org_id = ?2 AND partition_id = ?3 AND status = 'running'",
                tokio_rusqlite::params![now, org_id, partition_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn cancel(
    state: &AppState,
    org_id: &str,
    run_id: &str,
) -> Result<(), AppError> {
    let org_id = org_id.to_string();
    let run_id = run_id.to_string();
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            let n = conn.execute(
                "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE id = ?2 AND org_id = ?3 AND status = 'running'",
                tokio_rusqlite::params![now, run_id, org_id],
            )?;
            require_affected_sqlite(n)?;
            Ok(())
        })
        .await
        .map_not_found()?;
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
        transcript_text: row.get(10)?,
        started_at: row.get(11)?,
        finished_at: row.get(12)?,
    })
}

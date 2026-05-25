// SPDX-License-Identifier: Apache-2.0

use super::{require_affected_sqlite, DbResultExt};
use crate::db;
use async_trait::async_trait;
use eunomio_core::{traits::ShaverRunRepo, types::*, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;
use uuid::Uuid;

pub struct SqliteShaverRunRepo {
    conn: Arc<Connection>,
}

impl SqliteShaverRunRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl ShaverRunRepo for SqliteShaverRunRepo {
    async fn start(&self, row: NewShaverRunInsert) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        let run_id = id.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO shaver_runs \
                     (id, org_id, user_id, session_id, target_node_id, worktree_path, status, prompt_text, started_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'running', ?7, ?8)",
                    tokio_rusqlite::params![
                        run_id,
                        row.org_id,
                        row.user_id,
                        row.session_id,
                        row.target_node_id,
                        row.worktree_path,
                        row.prompt_text,
                        row.started_at
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(id)
    }

    async fn append_transcript_text(
        &self,
        org_id: &str,
        run_id: &str,
        chunk: &str,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let run_id = run_id.to_string();
        let chunk = chunk.to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE shaver_runs SET transcript_text = COALESCE(transcript_text, '') || ?1 WHERE id = ?2 AND org_id = ?3",
                    tokio_rusqlite::params![chunk, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn finish_success(
        &self,
        org_id: &str,
        run_id: &str,
        result_json: String,
        result_text: Option<String>,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let run_id = run_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE shaver_runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4 AND org_id = ?5",
                    tokio_rusqlite::params![result_json, result_text, now, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn finish_error(
        &self,
        org_id: &str,
        run_id: &str,
        error_message: String,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let run_id = run_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE shaver_runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3 AND org_id = ?4",
                    tokio_rusqlite::params![error_message, now, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn list_running(&self, org_id: &str) -> Result<Vec<RunningShaverRun>, AppError> {
        let org_id = org_id.to_string();
        let rows = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, org_id, session_id, worktree_path \
                     FROM shaver_runs WHERE org_id = ?1 AND status = 'running'",
                )?;
                let rows = stmt
                    .query_map(tokio_rusqlite::params![org_id], |row| {
                        Ok(RunningShaverRun {
                            id: row.get(0)?,
                            org_id: row.get(1)?,
                            session_id: row.get(2)?,
                            worktree_path: row.get(3)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }

    async fn mark_errored(
        &self,
        org_id: &str,
        run_ids: Vec<String>,
        error_message: &'static str,
    ) -> Result<(), AppError> {
        if run_ids.is_empty() {
            return Ok(());
        }
        let org_id = org_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                for id in &run_ids {
                    tx.execute(
                        "UPDATE shaver_runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3 AND org_id = ?4",
                        tokio_rusqlite::params![error_message, now, id, org_id],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }
}

// SPDX-License-Identifier: Apache-2.0

use super::{require_affected_sqlite, DbResultExt};
use crate::db;
use async_trait::async_trait;
use eunomio_core::{traits::PartitionRepo, types::*, AppError};
use rusqlite::types::Type;
use std::sync::Arc;
use tokio_rusqlite::Connection;
use uuid::Uuid;

pub struct SqlitePartitionRepo {
    conn: Arc<Connection>,
}

impl SqlitePartitionRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

fn partition_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<PartitionRow> {
    Ok(PartitionRow {
        id: row.get(0)?,
        org_id: row.get(1)?,
        user_id: row.get(2)?,
        session_id: row.get(3)?,
        target_node_id: row.get(4)?,
        strategy: row
            .get::<_, Option<String>>(5)?
            .and_then(|s| PartitionStrategy::parse(&s)),
        plan_json: row.get(6)?,
        candidate_slice_tree_sha: row.get(7)?,
        candidate_slice_commit_sha: row.get(8)?,
        phase: parse_phase(row, 9)?,
        phase_state: PhaseState::parse(&row.get::<_, String>(10)?).unwrap_or(PhaseState::Error),
        worktree_path: row.get(11)?,
        remaining_depth: row.get(12)?,
        created_at: row.get(13)?,
    })
}

fn parse_phase(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<PhaseName> {
    let raw: String = row.get(idx)?;
    PhaseName::parse(&raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            Type::Text,
            format!("unknown partition phase: {raw}").into(),
        )
    })
}

#[async_trait]
impl PartitionRepo for SqlitePartitionRepo {
    async fn get(&self, org_id: &str, partition_id: &str) -> Result<PartitionRow, AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let row: Option<PartitionRow> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT p.id, p.org_id, p.user_id, p.session_id, p.target_node_id, p.strategy, p.plan_json, p.candidate_slice_tree_sha, p.candidate_slice_commit_sha, p.phase, p.phase_state, p.worktree_path, p.remaining_depth, p.created_at \
                 FROM partitions p \
                 WHERE p.id = ?1 AND p.org_id = ?2",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![partition_id, org_id])?;
                if let Some(r) = rows.next()? {
                    Ok(Some(partition_row_mapper(r)?))
                } else {
                    Ok(None)
                }
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        row.ok_or(AppError::NotFound)
    }

    async fn list(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: Option<&str>,
    ) -> Result<Vec<PartitionRow>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let target_owned = target_node_id.map(|s| s.to_string());
        let rows: Vec<PartitionRow> = self
            .conn
            .call(move |conn| {
                let (sql, has_filter) = match &target_owned {
                    Some(_) => (
                        "SELECT id, org_id, user_id, session_id, target_node_id, strategy, plan_json, candidate_slice_tree_sha, candidate_slice_commit_sha, phase, phase_state, worktree_path, remaining_depth, created_at \
                     FROM partitions WHERE org_id = ?1 AND session_id = ?2 AND target_node_id = ?3 ORDER BY created_at",
                        true,
                    ),
                    None => (
                        "SELECT id, org_id, user_id, session_id, target_node_id, strategy, plan_json, candidate_slice_tree_sha, candidate_slice_commit_sha, phase, phase_state, worktree_path, remaining_depth, created_at \
                     FROM partitions WHERE org_id = ?1 AND session_id = ?2 ORDER BY created_at",
                        false,
                    ),
                };
                let mut stmt = conn.prepare(sql)?;
                let rows = if has_filter {
                    let target = target_owned.unwrap();
                    stmt.query_map(
                        tokio_rusqlite::params![org_id, session_id, target],
                        partition_row_mapper,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
                } else {
                    stmt.query_map(
                        tokio_rusqlite::params![org_id, session_id],
                        partition_row_mapper,
                    )?
                    .collect::<Result<Vec<_>, _>>()?
                };
                Ok(rows)
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }

    async fn list_siblings(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
        accepted_partition_id: &str,
    ) -> Result<Vec<SiblingInfo>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let target_node_id = target_node_id.to_string();
        let accepted_partition_id = accepted_partition_id.to_string();
        let rows: Vec<SiblingInfo> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, target_node_id, worktree_path FROM partitions \
                 WHERE org_id = ?1 AND session_id = ?2 AND target_node_id = ?3 AND id != ?4",
                )?;
                let rows = stmt
                    .query_map(
                        tokio_rusqlite::params![
                            org_id,
                            session_id,
                            target_node_id,
                            accepted_partition_id
                        ],
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
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }

    async fn delete(&self, org_id: &str, partition_id: &str) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "DELETE FROM partitions WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn delete_with_runs(&self, org_id: &str, partition_id: &str) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM runs WHERE partition_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                let n = tx.execute(
                    "DELETE FROM partitions WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn delete_many_with_runs(
        &self,
        org_id: &str,
        partition_ids: Vec<String>,
    ) -> Result<(), AppError> {
        for id in &partition_ids {
            self.get(org_id, id).await?;
        }
        let org_id = org_id.to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                for id in &partition_ids {
                    tx.execute(
                        "DELETE FROM runs WHERE partition_id = ?1 AND org_id = ?2",
                        tokio_rusqlite::params![id, org_id],
                    )?;
                    let n = tx.execute(
                        "DELETE FROM partitions WHERE id = ?1 AND org_id = ?2",
                        tokio_rusqlite::params![id, org_id],
                    )?;
                    require_affected_sqlite(n)?;
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn insert_pending(&self, row: NewPartitionInsert) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        let partition_id = id.clone();
        let NewPartitionInsert {
            org_id,
            user_id,
            session_id,
            target_node_id,
            worktree_path,
            initial_phase,
            remaining_depth,
            now,
        } = row;
        let initial_phase = initial_phase.as_str().to_string();
        let inserted_id = self
            .conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO partitions (id, org_id, user_id, session_id, target_node_id, phase, phase_state, worktree_path, remaining_depth, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'running', ?7, ?8, ?9)",
                    tokio_rusqlite::params![
                        partition_id,
                        org_id,
                        user_id,
                        session_id,
                        target_node_id,
                        initial_phase,
                        worktree_path,
                        remaining_depth,
                        now
                    ],
                )?;
                tx.commit()?;
                Ok(id)
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(inserted_id)
    }

    async fn clear_plan_and_slice(&self, org_id: &str, partition_id: &str) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE partitions SET plan_json = NULL, strategy = NULL, candidate_slice_tree_sha = NULL, candidate_slice_commit_sha = NULL WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn finalize_construct_accept(
        &self,
        org_id: String,
        session_id: String,
        partition_id: String,
        target_node_id: String,
        slice_node_id: String,
        parent_node_id: String,
        candidate_tree: String,
        candidate_commit: String,
        slice_title: String,
        slice_description: String,
        slice_strategy: Option<PartitionStrategy>,
        leftover_title: String,
        leftover_description: String,
        sibling_ids: Vec<String>,
        now: i64,
    ) -> Result<(), AppError> {
        self.get(&org_id, &partition_id).await?;
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                let slice_strategy_db = slice_strategy.map(|s| s.as_str().to_string());
                tx.execute(
                    "INSERT INTO nodes (session_id, node_id, org_id, parent_node_id, tree_sha, commit_sha, title, description, strategy, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    tokio_rusqlite::params![
                        session_id.clone(),
                        slice_node_id.clone(),
                        org_id.clone(),
                        parent_node_id,
                        candidate_tree,
                        candidate_commit,
                        slice_title,
                        slice_description,
                        slice_strategy_db,
                        now
                    ],
                )?;
                let n = tx.execute(
                    "UPDATE nodes SET parent_node_id = ?1, title = ?2, description = ?3 WHERE session_id = ?4 AND node_id = ?5 AND org_id = ?6",
                    tokio_rusqlite::params![
                        slice_node_id,
                        leftover_title,
                        leftover_description,
                        session_id,
                        target_node_id,
                        org_id.clone()
                    ],
                )?;
                require_affected_sqlite(n)?;
                let mut all_ids = sibling_ids.clone();
                all_ids.push(partition_id);
                for id in &all_ids {
                    tx.execute(
                        "DELETE FROM runs WHERE partition_id = ?1 AND org_id = ?2",
                        tokio_rusqlite::params![id, org_id.clone()],
                    )?;
                    let n = tx.execute(
                        "DELETE FROM partitions WHERE id = ?1 AND org_id = ?2",
                        tokio_rusqlite::params![id, org_id.clone()],
                    )?;
                    require_affected_sqlite(n)?;
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn accept_plan(
        &self,
        org_id: &str,
        partition_id: &str,
        plan_json: String,
        strategy: PartitionStrategy,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let strategy_str = strategy.as_str().to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE partitions SET plan_json = ?1, strategy = ?2, phase = 'construct', phase_state = 'running' WHERE id = ?3 AND org_id = ?4",
                    tokio_rusqlite::params![plan_json, strategy_str, partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn set_phase_state(
        &self,
        org_id: &str,
        partition_id: &str,
        phase_state: PhaseState,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let s = phase_state.as_str().to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE partitions SET phase_state = ?1 WHERE id = ?2 AND org_id = ?3",
                    tokio_rusqlite::params![s, partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn set_phase_running(
        &self,
        org_id: &str,
        partition_id: &str,
        phase: PhaseName,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let phase_str = phase.as_str().to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE partitions SET phase = ?1, phase_state = 'running' WHERE id = ?2 AND org_id = ?3",
                    tokio_rusqlite::params![phase_str, partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn set_worktree_path(
        &self,
        org_id: &str,
        partition_id: &str,
        worktree_path: String,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        self.conn
            .call(move |conn| {
                let n = conn.execute(
                    "UPDATE partitions SET worktree_path = ?1 WHERE id = ?2 AND org_id = ?3",
                    tokio_rusqlite::params![worktree_path, partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn accept_constructor_ok(
        &self,
        org_id: &str,
        partition_id: &str,
        tree_sha: String,
        commit_sha: String,
        run_id: &str,
        result_json: String,
        result_text: String,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let run_id = run_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                let n = tx.execute(
                    "UPDATE partitions SET candidate_slice_tree_sha = ?1, candidate_slice_commit_sha = ?2, phase = 'construct', phase_state = 'running' WHERE id = ?3 AND org_id = ?4",
                    tokio_rusqlite::params![tree_sha, commit_sha, partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                let n = tx.execute(
                    "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4 AND org_id = ?5",
                    tokio_rusqlite::params![result_json, result_text, now, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn accept_constructor_blocked(
        &self,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        result_json: String,
        result_text: String,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let run_id = run_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                let n = tx.execute(
                    "UPDATE partitions SET phase = 'construct', phase_state = 'awaiting_review' WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                let n = tx.execute(
                    "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4 AND org_id = ?5",
                    tokio_rusqlite::params![result_json, result_text, now, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn fail_run(
        &self,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        error_message: String,
        result_text: Option<String>,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let run_id = run_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                let n = tx.execute(
                    "UPDATE runs SET status = 'error', error_message = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4 AND org_id = ?5",
                    tokio_rusqlite::params![error_message, result_text, now, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                let n = tx.execute(
                    "UPDATE partitions SET phase_state = 'error' WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn cancel_run(
        &self,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let run_id = run_id.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                let n = tx.execute(
                    "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE id = ?2 AND org_id = ?3",
                    tokio_rusqlite::params![now, run_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                let n = tx.execute(
                    "UPDATE partitions SET phase_state = 'error' WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![partition_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }

    async fn list_id_session_worktree(
        &self,
        org_id: &str,
    ) -> Result<Vec<(String, String, String)>, AppError> {
        let org_id = org_id.to_string();
        let rows: Vec<(String, String, String)> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, worktree_path FROM partitions WHERE org_id = ?1",
                )?;
                let rows = stmt
                    .query_map(tokio_rusqlite::params![org_id], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }

    async fn list_all_id_org_session_worktree(
        &self,
    ) -> Result<Vec<(String, String, String, String)>, AppError> {
        let rows: Vec<(String, String, String, String)> = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT id, org_id, session_id, worktree_path FROM partitions")?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }
}

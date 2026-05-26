// SPDX-License-Identifier: Apache-2.0

use super::{require_affected_sqlite, DbResultExt};
use crate::display;
use async_trait::async_trait;
use eunomio_core::{traits::SessionRepo, types::*, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteSessionRepo {
    conn: Arc<Connection>,
}

impl SqliteSessionRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

fn session_row_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let normalized_remote: String = row.get(1)?;
    let literal_remote: String = row.get(2)?;
    let is_local: bool = row.get::<_, i64>(3)? != 0;
    let (repo_owner, repo_name) =
        display::repo_display_parts(&normalized_remote, is_local, &literal_remote);
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

#[async_trait]
impl SessionRepo for SqliteSessionRepo {
    async fn exists(&self, org_id: &str, session_id: &str) -> Result<bool, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let exists = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT 1 FROM sessions WHERE id = ?1 AND org_id = ?2")?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
                Ok(rows.next()?.is_some())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(exists)
    }

    async fn ensure(&self, org_id: &str, session_id: &str) -> Result<(), AppError> {
        if self.exists(org_id, session_id).await? {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn user_id(&self, org_id: &str, session_id: &str) -> Result<String, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let row: Option<String> = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT user_id FROM sessions WHERE id = ?1 AND org_id = ?2")?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(row.get(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        row.ok_or(AppError::NotFound)
    }

    async fn repo_fields(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<SessionRepoFields, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let row = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT normalized_remote, literal_remote, is_local FROM sessions WHERE id = ?1 AND org_id = ?2",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
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
            .await.map_err(crate::repo::map_sqlite_err)?;
        row.ok_or(AppError::NotFound)
    }

    async fn list(&self, org_id: &str) -> Result<Vec<Session>, AppError> {
        let org_id = org_id.to_string();
        let rows: Vec<Session> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_node_id, created_at \
                     FROM sessions WHERE org_id = ?1 ORDER BY created_at DESC",
                )?;
                let rows = stmt
                    .query_map(tokio_rusqlite::params![org_id], session_row_mapper)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }

    async fn get(&self, org_id: &str, session_id: &str) -> Result<Option<Session>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let row: Option<Session> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_node_id, created_at \
                     FROM sessions WHERE id = ?1 AND org_id = ?2",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(session_row_mapper(row)?))
                } else {
                    Ok(None)
                }
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(row)
    }

    async fn final_tree(&self, org_id: &str, session_id: &str) -> Result<Option<String>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let row: Option<String> = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT final_tree FROM sessions WHERE id = ?1 AND org_id = ?2")?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(row.get::<_, String>(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(row)
    }

    async fn base_tree(&self, org_id: &str, session_id: &str) -> Result<Option<String>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let row: Option<String> = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT base_tree FROM sessions WHERE id = ?1 AND org_id = ?2")?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(row.get::<_, String>(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(row)
    }

    async fn seed_trees(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<(String, String), AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let row: Option<(String, String)> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT base_tree, final_tree FROM sessions WHERE id = ?1 AND org_id = ?2",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id, org_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some((row.get(0)?, row.get(1)?)))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        row.ok_or(AppError::NotFound)
    }

    async fn find_by_refs(
        &self,
        org_id: &str,
        normalized_remote: &str,
        base_ref: &str,
        source_ref: &str,
    ) -> Result<Option<Session>, AppError> {
        let org_id = org_id.to_string();
        let normalized_remote = normalized_remote.to_string();
        let base_ref = base_ref.to_string();
        let source_ref = source_ref.to_string();
        let existing = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_node_id, created_at \
                     FROM sessions \
                     WHERE org_id = ?1 AND normalized_remote = ?2 AND base_ref = ?3 AND source_ref = ?4 \
                     LIMIT 1",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![
                    org_id,
                    normalized_remote,
                    base_ref,
                    source_ref
                ])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(session_row_mapper(row)?))
                } else {
                    Ok(None)
                }
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(existing)
    }

    async fn count_for_normalized(
        &self,
        org_id: &str,
        normalized_remote: &str,
    ) -> Result<i64, AppError> {
        let org_id = org_id.to_string();
        let normalized_remote = normalized_remote.to_string();
        let count: i64 = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT COUNT(*) FROM sessions WHERE org_id = ?1 AND normalized_remote = ?2",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![org_id, normalized_remote])?;
                let row = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                let count: i64 = row.get(0)?;
                Ok(count)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(count)
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_seed_nodes(
        &self,
        org_id: String,
        user_id: String,
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
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO sessions (id, org_id, user_id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_tree, final_tree, base_node_id, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    tokio_rusqlite::params![
                        session_id,
                        org_id.clone(),
                        user_id,
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
                    "INSERT INTO nodes (session_id, node_id, org_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                     VALUES (?1, ?2, ?3, NULL, ?4, ?5, 'base', ?6)",
                    tokio_rusqlite::params![
                        session_id,
                        base_node_id,
                        org_id,
                        base_tree,
                        base_commit,
                        now
                    ],
                )?;
                tx.execute(
                    "INSERT INTO nodes (session_id, node_id, org_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'final', ?7)",
                    tokio_rusqlite::params![
                        session_id,
                        final_node_id,
                        org_id,
                        base_node_id,
                        final_tree,
                        final_commit,
                        now
                    ],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn list_partition_worktrees(
        &self,
        org_id: &str,
        session_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let rows: Vec<String> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT worktree_path FROM partitions WHERE session_id = ?1 AND org_id = ?2",
                )?;
                let rows = stmt
                    .query_map(tokio_rusqlite::params![session_id, org_id], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(rows)
    }

    async fn delete_cascade(&self, org_id: &str, session_id: &str) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM runs WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                tx.execute(
                    "DELETE FROM shaver_runs WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                tx.execute(
                    "DELETE FROM shaving_tracks WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                tx.execute(
                    "DELETE FROM partitions WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                tx.execute(
                    "DELETE FROM edge_file_viewed WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                tx.execute(
                    "DELETE FROM node_reviewed WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                tx.execute(
                    "DELETE FROM nodes WHERE session_id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                let n = tx.execute(
                    "DELETE FROM sessions WHERE id = ?1 AND org_id = ?2",
                    tokio_rusqlite::params![session_id, org_id],
                )?;
                require_affected_sqlite(n)?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_not_found()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use eunomio_core::traits::SessionRepo;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn delete_cascade_removes_session_and_children() {
        let dir = TempDir::new().unwrap();
        let conn = Arc::new(db::open(&dir.path().join("test.db")).await.unwrap());
        let repo = SqliteSessionRepo::new(conn.clone());

        conn.call(|c| {
            c.execute(
                "INSERT INTO orgs (id, display_name, created_at) VALUES ('local', 'Local', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO users (id, username, created_at) VALUES ('u1', 'alice', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO sessions (id, org_id, user_id, normalized_remote, literal_remote, is_local, base_ref, source_ref, base_tree, final_tree, base_node_id, created_at) \
                 VALUES ('s1', 'local', 'u1', 'local:/tmp', 'local:/tmp', 1, 'main', 'main', 't0', 't1', 'base', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO nodes (node_id, session_id, org_id, tree_sha, commit_sha, title, parent_node_id, created_at) \
                 VALUES ('n1', 's1', 'local', 't1', 'c1', 'base', NULL, 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO partitions (id, org_id, user_id, session_id, target_node_id, phase, phase_state, worktree_path, created_at) \
                 VALUES ('p1', 'local', 'u1', 's1', 'n1', 'survey', 'idle', '/tmp/wt', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO runs (id, org_id, user_id, partition_id, session_id, target_node_id, kind, status, started_at) \
                 VALUES ('r1', 'local', 'u1', 'p1', 's1', 'n1', 'survey', 'running', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO edge_file_viewed \
                 (org_id, user_id, session_id, target_node_id, file_path, viewed_at) \
                 VALUES ('local', 'u1', 's1', 'n1', 'src/a.rs', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO node_reviewed \
                 (org_id, user_id, session_id, node_id, reviewed_at) \
                 VALUES ('local', 'u1', 's1', 'n1', 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        repo.delete_cascade("local", "s1").await.unwrap();

        let counts: (i64, i64, i64, i64, i64, i64) = conn
            .call(|c| {
                let sessions: i64 =
                    c.query_row("SELECT COUNT(*) FROM sessions WHERE id = 's1'", [], |r| {
                        r.get(0)
                    })?;
                let nodes: i64 = c.query_row(
                    "SELECT COUNT(*) FROM nodes WHERE session_id = 's1'",
                    [],
                    |r| r.get(0),
                )?;
                let partitions: i64 = c.query_row(
                    "SELECT COUNT(*) FROM partitions WHERE session_id = 's1'",
                    [],
                    |r| r.get(0),
                )?;
                let runs: i64 = c.query_row(
                    "SELECT COUNT(*) FROM runs WHERE session_id = 's1'",
                    [],
                    |r| r.get(0),
                )?;
                let viewed: i64 = c.query_row(
                    "SELECT COUNT(*) FROM edge_file_viewed WHERE session_id = 's1'",
                    [],
                    |r| r.get(0),
                )?;
                let reviewed: i64 = c.query_row(
                    "SELECT COUNT(*) FROM node_reviewed WHERE session_id = 's1'",
                    [],
                    |r| r.get(0),
                )?;
                Ok((sessions, nodes, partitions, runs, viewed, reviewed))
            })
            .await
            .unwrap();
        assert_eq!(counts, (0, 0, 0, 0, 0, 0));
    }
}

// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use eunomio_core::{traits::EdgeFileViewedRepo, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteEdgeFileViewedRepo {
    conn: Arc<Connection>,
}

impl SqliteEdgeFileViewedRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl EdgeFileViewedRepo for SqliteEdgeFileViewedRepo {
    async fn list_paths(
        &self,
        org_id: &str,
        user_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let session_id = session_id.to_string();
        let target_node_id = target_node_id.to_string();
        let paths: Vec<String> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT file_path FROM edge_file_viewed \
                     WHERE org_id = ?1 AND user_id = ?2 AND session_id = ?3 AND target_node_id = ?4 \
                     ORDER BY file_path",
                )?;
                let rows = stmt.query_map(
                    tokio_rusqlite::params![org_id, user_id, session_id, target_node_id],
                    |row| row.get(0),
                )?;
                let mut paths = Vec::new();
                for row in rows {
                    paths.push(row?);
                }
                Ok(paths)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(paths)
    }

    async fn set_viewed(
        &self,
        org_id: &str,
        user_id: &str,
        session_id: &str,
        target_node_id: &str,
        file_path: &str,
        viewed: bool,
        viewed_at: i64,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let session_id = session_id.to_string();
        let target_node_id = target_node_id.to_string();
        let file_path = file_path.to_string();
        self.conn
            .call(move |conn| {
                if viewed {
                    conn.execute(
                        "INSERT INTO edge_file_viewed \
                         (org_id, user_id, session_id, target_node_id, file_path, viewed_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                         ON CONFLICT(org_id, user_id, session_id, target_node_id, file_path) \
                         DO UPDATE SET viewed_at = excluded.viewed_at",
                        tokio_rusqlite::params![
                            org_id,
                            user_id,
                            session_id,
                            target_node_id,
                            file_path,
                            viewed_at
                        ],
                    )?;
                } else {
                    conn.execute(
                        "DELETE FROM edge_file_viewed \
                         WHERE org_id = ?1 AND user_id = ?2 AND session_id = ?3 \
                           AND target_node_id = ?4 AND file_path = ?5",
                        tokio_rusqlite::params![
                            org_id,
                            user_id,
                            session_id,
                            target_node_id,
                            file_path
                        ],
                    )?;
                }
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use eunomio_core::traits::EdgeFileViewedRepo;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn set_viewed_roundtrip() {
        let dir = TempDir::new().unwrap();
        let conn = Arc::new(db::open(&dir.path().join("test.db")).await.unwrap());
        let repo = SqliteEdgeFileViewedRepo::new(conn.clone());

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
                 VALUES ('n1', 's1', 'local', 't1', 'c1', '1', 'base', 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        repo.set_viewed("local", "u1", "s1", "n1", "src/a.rs", true, 1)
            .await
            .unwrap();
        let paths = repo.list_paths("local", "u1", "s1", "n1").await.unwrap();
        assert_eq!(paths, vec!["src/a.rs".to_string()]);

        repo.set_viewed("local", "u1", "s1", "n1", "src/a.rs", false, 1)
            .await
            .unwrap();
        assert!(repo.list_paths("local", "u1", "s1", "n1").await.unwrap().is_empty());
    }
}

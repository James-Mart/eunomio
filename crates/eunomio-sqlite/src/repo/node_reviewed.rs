// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use eunomio_core::{traits::NodeReviewedRepo, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteNodeReviewedRepo {
    conn: Arc<Connection>,
}

impl SqliteNodeReviewedRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl NodeReviewedRepo for SqliteNodeReviewedRepo {
    async fn list_node_ids(
        &self,
        org_id: &str,
        user_id: &str,
        session_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let session_id = session_id.to_string();
        let node_ids: Vec<String> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT node_id FROM node_reviewed \
                     WHERE org_id = ?1 AND user_id = ?2 AND session_id = ?3 \
                     ORDER BY node_id",
                )?;
                let rows = stmt.query_map(
                    tokio_rusqlite::params![org_id, user_id, session_id],
                    |row| row.get(0),
                )?;
                let mut node_ids = Vec::new();
                for row in rows {
                    node_ids.push(row?);
                }
                Ok(node_ids)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(node_ids)
    }

    async fn set_reviewed(
        &self,
        org_id: &str,
        user_id: &str,
        session_id: &str,
        node_id: &str,
        reviewed: bool,
        reviewed_at: i64,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let session_id = session_id.to_string();
        let node_id = node_id.to_string();
        self.conn
            .call(move |conn| {
                if reviewed {
                    conn.execute(
                        "INSERT INTO node_reviewed \
                         (org_id, user_id, session_id, node_id, reviewed_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5) \
                         ON CONFLICT(org_id, user_id, session_id, node_id) \
                         DO UPDATE SET reviewed_at = excluded.reviewed_at",
                        tokio_rusqlite::params![org_id, user_id, session_id, node_id, reviewed_at],
                    )?;
                } else {
                    conn.execute(
                        "DELETE FROM node_reviewed \
                         WHERE org_id = ?1 AND user_id = ?2 AND session_id = ?3 AND node_id = ?4",
                        tokio_rusqlite::params![org_id, user_id, session_id, node_id],
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
    use eunomio_core::traits::NodeReviewedRepo;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn set_reviewed_roundtrip() {
        let dir = TempDir::new().unwrap();
        let conn = Arc::new(db::open(&dir.path().join("test.db")).await.unwrap());
        let repo = SqliteNodeReviewedRepo::new(conn.clone());

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

        repo.set_reviewed("local", "u1", "s1", "n1", true, 1)
            .await
            .unwrap();
        let ids = repo.list_node_ids("local", "u1", "s1").await.unwrap();
        assert_eq!(ids, vec!["n1".to_string()]);

        repo.set_reviewed("local", "u1", "s1", "n1", false, 1)
            .await
            .unwrap();
        assert!(repo
            .list_node_ids("local", "u1", "s1")
            .await
            .unwrap()
            .is_empty());
    }
}

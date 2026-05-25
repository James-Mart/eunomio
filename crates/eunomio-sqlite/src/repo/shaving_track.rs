// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use eunomio_core::{traits::ShavingTrackRepo, types::*, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteShavingTrackRepo {
    conn: Arc<Connection>,
}

impl SqliteShavingTrackRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl ShavingTrackRepo for SqliteShavingTrackRepo {
    async fn insert(&self, row: NewShavingTrackInsert) -> Result<(), AppError> {
        let steps_json =
            serde_json::to_string(&row.steps).map_err(|e| AppError::Internal(e.into()))?;
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO shaving_tracks \
                     (session_id, target_node_id, org_id, parent_tree_sha, head_tree_sha, steps_json, ref_name, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        row.session_id,
                        row.target_node_id,
                        row.org_id,
                        row.parent_tree_sha,
                        row.head_tree_sha,
                        steps_json,
                        row.ref_name,
                        row.created_at
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn get(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Option<ShavingTrack>, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let target_node_id = target_node_id.to_string();
        let row: Option<(String, String, String, String)> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT target_node_id, parent_tree_sha, head_tree_sha, steps_json \
                     FROM shaving_tracks \
                     WHERE org_id = ?1 AND session_id = ?2 AND target_node_id = ?3",
                )?;
                let mut rows = stmt.query(rusqlite::params![org_id, session_id, target_node_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        let Some((target_node_id, parent_tree_sha, head_tree_sha, steps_json)) = row else {
            return Ok(None);
        };
        let steps: Vec<ShavingStep> =
            serde_json::from_str(&steps_json).map_err(|e| AppError::Internal(e.into()))?;
        Ok(Some(ShavingTrack {
            target_node_id,
            parent_tree_sha,
            head_tree_sha,
            steps,
        }))
    }

    async fn delete(
        &self,
        org_id: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let target_node_id = target_node_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM shaving_tracks \
                     WHERE org_id = ?1 AND session_id = ?2 AND target_node_id = ?3",
                    rusqlite::params![org_id, session_id, target_node_id],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }
}

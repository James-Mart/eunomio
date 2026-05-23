// SPDX-License-Identifier: Apache-2.0

use crate::db;
use async_trait::async_trait;
use eunomio_core::{traits::UserRepo, types::UserRow, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;
use uuid::Uuid;

pub struct SqliteUserRepo {
    conn: Arc<Connection>,
}

impl SqliteUserRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl UserRepo for SqliteUserRepo {
    async fn get_by_id(&self, user_id: &str) -> Result<Option<UserRow>, AppError> {
        let user_id = user_id.to_string();
        let row: Option<UserRow> = self
            .conn
            .call(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT id, username, created_at FROM users WHERE id = ?1")?;
                let mut rows = stmt.query(tokio_rusqlite::params![user_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(UserRow {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        created_at: row.get(2)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(row)
    }

    async fn get_by_username(&self, username: &str) -> Result<Option<UserRow>, AppError> {
        let username = username.to_string();
        let row: Option<UserRow> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn
                    .prepare("SELECT id, username, created_at FROM users WHERE username = ?1")?;
                let mut rows = stmt.query(tokio_rusqlite::params![username])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(UserRow {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        created_at: row.get(2)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(row)
    }

    async fn insert(&self, username: &str) -> Result<UserRow, AppError> {
        let id = Uuid::new_v4().to_string();
        let username = username.to_string();
        let now = db::unix_seconds();
        let row: UserRow = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO users (id, username, created_at) VALUES (?1, ?2, ?3)",
                    tokio_rusqlite::params![id, username, now],
                )?;
                Ok(UserRow {
                    id,
                    username,
                    created_at: now,
                })
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(row)
    }

    async fn ensure_membership(
        &self,
        org_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<(), AppError> {
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let role = role.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO org_memberships (org_id, user_id, role, created_at) VALUES (?1, ?2, ?3, ?4)",
                    tokio_rusqlite::params![org_id, user_id, role, now],
                )?;
                Ok(())
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn membership_role(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<String>, AppError> {
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let role: Option<String> = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT role FROM org_memberships WHERE org_id = ?1 AND user_id = ?2",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![org_id, user_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(row.get(0)?))
                } else {
                    Ok(None)
                }
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(role)
    }
}

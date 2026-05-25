// SPDX-License-Identifier: Apache-2.0

use super::auth_event::record_in_tx;
use crate::db;
use async_trait::async_trait;
use eunomio_core::{principal::AuthSessionRow, traits::AuthSessionRepo, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteAuthSessionRepo {
    conn: Arc<Connection>,
}

impl SqliteAuthSessionRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl AuthSessionRepo for SqliteAuthSessionRepo {
    async fn load(&self, session_id: &str) -> Result<Option<AuthSessionRow>, AppError> {
        let session_id = session_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, user_id, org_id, created_at, last_seen_at, expires_at \
                     FROM auth_sessions WHERE id = ?1",
                )?;
                let mut rows = stmt.query(tokio_rusqlite::params![session_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(AuthSessionRow {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        org_id: row.get(2)?,
                        created_at: row.get(3)?,
                        last_seen_at: row.get(4)?,
                        expires_at: row.get(5)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(crate::repo::map_sqlite_err)
    }

    async fn refresh_last_seen(&self, session_id: &str, now: i64) -> Result<(), AppError> {
        let session_id = session_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE auth_sessions SET last_seen_at = ?1 WHERE id = ?2",
                    tokio_rusqlite::params![now, session_id],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn delete(&self, session_id: &str) -> Result<(), AppError> {
        let session_id = session_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM auth_sessions WHERE id = ?1",
                    tokio_rusqlite::params![session_id],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn rotate_with_audit(
        &self,
        user_id: &str,
        org_id: &str,
        new_session_id: &str,
        expires_at: i64,
        ip: &str,
        user_agent: &str,
        username_for_audit: &str,
    ) -> Result<(), AppError> {
        let user_id = user_id.to_string();
        let org_id = org_id.to_string();
        let session_id = new_session_id.to_string();
        let ip = ip.to_string();
        let user_agent = user_agent.to_string();
        let username_json = username_for_audit.to_string();
        let org_id_for_row = org_id.clone();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM auth_sessions WHERE user_id = ?1",
                    tokio_rusqlite::params![user_id.clone()],
                )?;
                let now = db::unix_seconds();
                tx.execute(
                    "INSERT INTO auth_sessions (id, user_id, org_id, created_at, last_seen_at, expires_at, ip, user_agent) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    tokio_rusqlite::params![
                        session_id,
                        user_id.clone(),
                        org_id_for_row,
                        now,
                        now,
                        expires_at,
                        ip.clone(),
                        user_agent.clone()
                    ],
                )?;
                record_in_tx(
                    &tx,
                    Some(&org_id),
                    Some(&user_id),
                    "login_success",
                    &ip,
                    &user_agent,
                    serde_json::json!({ "username": username_json }),
                )?;
                record_in_tx(
                    &tx,
                    Some(&org_id),
                    Some(&user_id),
                    "session_rotated",
                    &ip,
                    &user_agent,
                    serde_json::json!({}),
                )?;
                tx.commit()?;
                Ok(())
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn delete_with_audit(
        &self,
        session_id: &str,
        org_id: &str,
        user_id: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError> {
        let session_id = session_id.to_string();
        let org_id = org_id.to_string();
        let user_id = user_id.to_string();
        let ip = ip.to_string();
        let user_agent = user_agent.to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM auth_sessions WHERE id = ?1",
                    tokio_rusqlite::params![session_id],
                )?;
                record_in_tx(
                    &tx,
                    Some(&org_id),
                    Some(&user_id),
                    "logout",
                    &ip,
                    &user_agent,
                    serde_json::json!({}),
                )?;
                tx.commit()?;
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
    use eunomio_core::traits::AuthEventRepo;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn open_test_repo() -> (TempDir, Arc<Connection>, SqliteAuthSessionRepo) {
        let dir = TempDir::new().unwrap();
        let conn = Arc::new(db::open(&dir.path().join("test.db")).await.unwrap());
        let repo = SqliteAuthSessionRepo::new(conn.clone());
        (dir, conn, repo)
    }

    #[tokio::test]
    async fn rotate_with_audit_emits_login_success_then_session_rotated() {
        let (_dir, conn, repo) = open_test_repo().await;
        let events = super::super::auth_event::SqliteAuthEventRepo::new(conn.clone());

        conn.call(|c| {
            c.execute(
                "INSERT INTO orgs (id, display_name, created_at) VALUES ('local', 'Local', 1)",
                [],
            )?;
            c.execute(
                "INSERT INTO users (id, username, created_at) VALUES ('u1', 'alice', 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        repo.rotate_with_audit(
            "u1",
            "local",
            "sess-1",
            9999999999,
            "127.0.0.1",
            "ua",
            "alice",
        )
        .await
        .unwrap();

        let types = events.list_by_event_type("login_success").await.unwrap();
        assert_eq!(types.len(), 1);
        let rotated = events.list_by_event_type("session_rotated").await.unwrap();
        assert_eq!(rotated.len(), 1);

        let order: Vec<String> = conn
            .call(|c| {
                let mut stmt = c.prepare("SELECT event_type FROM auth_events ORDER BY id")?;
                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .unwrap();
        assert_eq!(order, vec!["login_success", "session_rotated"]);
    }

    #[tokio::test]
    async fn delete_with_audit_emits_logout_in_same_transaction() {
        let (_dir, conn, repo) = open_test_repo().await;
        let events = super::super::auth_event::SqliteAuthEventRepo::new(conn.clone());

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
                "INSERT INTO auth_sessions (id, user_id, org_id, created_at, last_seen_at, expires_at, ip, user_agent) \
                 VALUES ('sess-1', 'u1', 'local', 1, 1, 9999999999, '127.0.0.1', 'ua')",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        repo.delete_with_audit("sess-1", "local", "u1", "127.0.0.1", "ua")
            .await
            .unwrap();

        assert!(repo.load("sess-1").await.unwrap().is_none());
        let logout = events.list_by_event_type("logout").await.unwrap();
        assert_eq!(logout.len(), 1);
    }
}

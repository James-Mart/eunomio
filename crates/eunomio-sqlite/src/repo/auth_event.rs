// SPDX-License-Identifier: Apache-2.0

use crate::db;
use async_trait::async_trait;
use eunomio_core::{traits::AuthEventRepo, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteAuthEventRepo {
    conn: Arc<Connection>,
}

impl SqliteAuthEventRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

pub(crate) fn record_in_tx(
    conn: &rusqlite::Connection,
    org_id: Option<&str>,
    user_id: Option<&str>,
    event_type: &str,
    ip: &str,
    user_agent: &str,
    details: serde_json::Value,
) -> rusqlite::Result<()> {
    let details_json = details.to_string();
    let now = db::unix_seconds();
    conn.execute(
        "INSERT INTO auth_events (org_id, user_id, event_type, ip, user_agent, details_json, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            org_id,
            user_id,
            event_type,
            ip,
            user_agent,
            details_json,
            now
        ],
    )?;
    Ok(())
}

#[async_trait]
impl AuthEventRepo for SqliteAuthEventRepo {
    async fn insert(
        &self,
        org_id: Option<&str>,
        user_id: Option<&str>,
        event_type: &str,
        ip: &str,
        user_agent: &str,
        details: serde_json::Value,
    ) -> Result<(), AppError> {
        let org_id = org_id.map(str::to_string);
        let user_id = user_id.map(str::to_string);
        let event_type = event_type.to_string();
        let ip = ip.to_string();
        let user_agent = user_agent.to_string();
        let details_json = details.to_string();
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO auth_events (org_id, user_id, event_type, ip, user_agent, details_json, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    tokio_rusqlite::params![
                        org_id,
                        user_id,
                        event_type,
                        ip,
                        user_agent,
                        details_json,
                        now
                    ],
                )?;
                Ok(())
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }

    async fn list_by_event_type(&self, event_type: &str) -> Result<Vec<String>, AppError> {
        let event_type = event_type.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT event_type FROM auth_events WHERE event_type = ?1 ORDER BY id",
                )?;
                let rows = stmt
                    .query_map(tokio_rusqlite::params![event_type], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .map_err(super::map_sqlite_err)
    }
}

// SPDX-License-Identifier: Apache-2.0

use crate::db;
use async_trait::async_trait;
use eunomio_core::{traits::OrgRepo, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

const LOCAL_ORG_ID: &str = "local";

pub struct SqliteOrgRepo {
    conn: Arc<Connection>,
}

impl SqliteOrgRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl OrgRepo for SqliteOrgRepo {
    async fn ensure_singleton_local(&self) -> Result<(), AppError> {
        let now = db::unix_seconds();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO orgs (id, display_name, created_at) VALUES (?1, ?2, ?3)",
                    tokio_rusqlite::params![LOCAL_ORG_ID, "Local", now],
                )?;
                Ok(())
            })
            .await.map_err(crate::repo::map_sqlite_err)?;
        Ok(())
    }
}

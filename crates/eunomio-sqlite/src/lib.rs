// SPDX-License-Identifier: Apache-2.0

pub mod db;
pub mod display;
pub mod repo;

use eunomio_core::traits::{
    AuthEventRepo, AuthSessionRepo, Datastore, NodeRepo, OrgRepo, PartitionRepo, RunRepo,
    SessionRepo, UserRepo,
};
use repo::{
    SqliteAuthEventRepo, SqliteAuthSessionRepo, SqliteNodeRepo, SqliteOrgRepo, SqlitePartitionRepo,
    SqliteRunRepo, SqliteSessionRepo, SqliteUserRepo,
};
use std::{path::Path, sync::Arc};
use tokio_rusqlite::Connection;

pub struct SqliteDatastore {
    orgs: Arc<SqliteOrgRepo>,
    users: Arc<SqliteUserRepo>,
    auth_sessions: Arc<SqliteAuthSessionRepo>,
    auth_events: Arc<SqliteAuthEventRepo>,
    sessions: Arc<SqliteSessionRepo>,
    nodes: Arc<SqliteNodeRepo>,
    partitions: Arc<SqlitePartitionRepo>,
    runs: Arc<SqliteRunRepo>,
}

impl SqliteDatastore {
    pub async fn open(db_path: &Path) -> anyhow::Result<Self> {
        let conn = Arc::new(db::open(db_path).await?);
        Ok(Self::new(conn))
    }

    pub fn new(conn: Arc<Connection>) -> Self {
        Self {
            orgs: Arc::new(SqliteOrgRepo::new(conn.clone())),
            users: Arc::new(SqliteUserRepo::new(conn.clone())),
            auth_sessions: Arc::new(SqliteAuthSessionRepo::new(conn.clone())),
            auth_events: Arc::new(SqliteAuthEventRepo::new(conn.clone())),
            sessions: Arc::new(SqliteSessionRepo::new(conn.clone())),
            nodes: Arc::new(SqliteNodeRepo::new(conn.clone())),
            partitions: Arc::new(SqlitePartitionRepo::new(conn.clone())),
            runs: Arc::new(SqliteRunRepo::new(conn)),
        }
    }
}

impl Datastore for SqliteDatastore {
    fn orgs(&self) -> &dyn OrgRepo {
        self.orgs.as_ref()
    }

    fn users(&self) -> &dyn UserRepo {
        self.users.as_ref()
    }

    fn auth_sessions(&self) -> &dyn AuthSessionRepo {
        self.auth_sessions.as_ref()
    }

    fn auth_events(&self) -> &dyn AuthEventRepo {
        self.auth_events.as_ref()
    }

    fn sessions(&self) -> &dyn SessionRepo {
        self.sessions.as_ref()
    }

    fn nodes(&self) -> &dyn NodeRepo {
        self.nodes.as_ref()
    }

    fn partitions(&self) -> &dyn PartitionRepo {
        self.partitions.as_ref()
    }

    fn runs(&self) -> &dyn RunRepo {
        self.runs.as_ref()
    }
}

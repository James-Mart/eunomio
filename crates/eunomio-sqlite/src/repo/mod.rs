// SPDX-License-Identifier: Apache-2.0

use eunomio_core::AppError;

pub(crate) fn map_sqlite_err(e: tokio_rusqlite::Error) -> AppError {
    AppError::Internal(anyhow::anyhow!(e))
}

pub mod auth_event;
pub mod auth_session;
pub mod node;
pub mod org;
pub mod partition;
pub mod run;
pub mod session;
pub mod user;

pub use auth_event::SqliteAuthEventRepo;
pub use auth_session::SqliteAuthSessionRepo;
pub use node::SqliteNodeRepo;
pub use org::SqliteOrgRepo;
pub use partition::SqlitePartitionRepo;
pub use run::SqliteRunRepo;
pub use session::SqliteSessionRepo;
pub use user::SqliteUserRepo;

pub(crate) fn require_affected_sqlite(rows: usize) -> Result<(), tokio_rusqlite::Error> {
    if rows == 0 {
        Err(tokio_rusqlite::Error::Rusqlite(
            rusqlite::Error::QueryReturnedNoRows,
        ))
    } else {
        Ok(())
    }
}

pub(crate) trait DbResultExt<T> {
    fn map_not_found(self) -> Result<T, AppError>;
}

impl<T> DbResultExt<T> for Result<T, tokio_rusqlite::Error> {
    fn map_not_found(self) -> Result<T, AppError> {
        match self {
            Err(tokio_rusqlite::Error::Rusqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                Err(AppError::NotFound)
            }
            Err(e) => Err(map_sqlite_err(e)),
            Ok(v) => Ok(v),
        }
    }
}

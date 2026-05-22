use crate::error::AppError;

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
            Err(e) => Err(e.into()),
            Ok(v) => Ok(v),
        }
    }
}

pub mod node;
pub mod org;
pub mod partition;
pub mod run;
pub mod session;
pub mod tree;
pub mod user;

use crate::{db, error::AppError, state::AppState};

pub const LOCAL_ORG_ID: &str = db::LOCAL_ORG_ID;

pub async fn ensure_singleton_local(state: &AppState) -> Result<(), AppError> {
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO orgs (id, display_name, created_at) VALUES (?1, ?2, ?3)",
                tokio_rusqlite::params![LOCAL_ORG_ID, "Local", now],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

use crate::{db, error::AppError, state::AppState};
use uuid::Uuid;

pub struct UserRow {
    pub id: String,
    pub username: String,
    pub created_at: i64,
}

pub async fn get_by_id(state: &AppState, user_id: &str) -> Result<Option<UserRow>, AppError> {
    let user_id = user_id.to_string();
    let row: Option<UserRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare("SELECT id, username, created_at FROM users WHERE id = ?1")?;
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
        .await?;
    Ok(row)
}

pub async fn get_by_username(
    state: &AppState,
    username: &str,
) -> Result<Option<UserRow>, AppError> {
    let username = username.to_string();
    let row: Option<UserRow> = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT id, username, created_at FROM users WHERE username = ?1")?;
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
        .await?;
    Ok(row)
}

pub async fn insert(state: &AppState, username: &str) -> Result<UserRow, AppError> {
    let id = Uuid::new_v4().to_string();
    let username = username.to_string();
    let now = db::unix_seconds();
    let row: UserRow = state
        .db
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
        .await?;
    Ok(row)
}

pub async fn ensure_membership(
    state: &AppState,
    org_id: &str,
    user_id: &str,
    role: &str,
) -> Result<(), AppError> {
    let org_id = org_id.to_string();
    let user_id = user_id.to_string();
    let role = role.to_string();
    let now = db::unix_seconds();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO org_memberships (org_id, user_id, role, created_at) VALUES (?1, ?2, ?3, ?4)",
                tokio_rusqlite::params![org_id, user_id, role, now],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn membership_role(
    state: &AppState,
    org_id: &str,
    user_id: &str,
) -> Result<Option<String>, AppError> {
    let org_id = org_id.to_string();
    let user_id = user_id.to_string();
    let role: Option<String> = state
        .db
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
        .await?;
    Ok(role)
}

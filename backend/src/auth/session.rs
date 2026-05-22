use crate::{db, error::AppError, state::AppState};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use getrandom::getrandom;

pub const COOKIE_NAME: &str = "eunomio_local_session";
pub const ABSOLUTE_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;
pub const IDLE_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

pub struct AuthSessionRow {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub expires_at: i64,
}

pub fn random_session_id() -> String {
    let mut bytes = [0u8; 32];
    getrandom(&mut bytes).expect("random session id");
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn parse_cookie(cookie_header: &str) -> Option<String> {
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn serialize_cookie(session_id: &str) -> String {
    format!("{COOKIE_NAME}={session_id}; HttpOnly; Path=/; SameSite=Lax")
}

pub fn clear_cookie() -> String {
    format!("{COOKIE_NAME}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0")
}

pub async fn load(
    state: &AppState,
    session_id: &str,
) -> Result<Option<AuthSessionRow>, AppError> {
    let session_id = session_id.to_string();
    state
        .db
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
        .map_err(AppError::from)
}

pub fn is_expired(row: &AuthSessionRow, now: i64) -> bool {
    now >= row.expires_at || now - row.last_seen_at >= IDLE_LIFETIME_SECS
}

pub async fn refresh_last_seen(
    state: &AppState,
    session_id: &str,
    now: i64,
) -> Result<(), AppError> {
    let session_id = session_id.to_string();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE auth_sessions SET last_seen_at = ?1 WHERE id = ?2",
                tokio_rusqlite::params![now, session_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn delete(state: &AppState, session_id: &str) -> Result<(), AppError> {
    let session_id = session_id.to_string();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "DELETE FROM auth_sessions WHERE id = ?1",
                tokio_rusqlite::params![session_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn create(
    state: &AppState,
    session_id: &str,
    user_id: &str,
    org_id: &str,
    ip: &str,
    user_agent: &str,
) -> Result<(), AppError> {
    let session_id = session_id.to_string();
    let user_id = user_id.to_string();
    let org_id = org_id.to_string();
    let ip = ip.to_string();
    let user_agent = user_agent.to_string();
    let now = db::unix_seconds();
    let expires_at = now + ABSOLUTE_LIFETIME_SECS;
    state
        .db
        .call(move |conn| {
            conn.execute(
                "INSERT INTO auth_sessions (id, user_id, org_id, created_at, last_seen_at, expires_at, ip, user_agent) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                tokio_rusqlite::params![
                    session_id,
                    user_id,
                    org_id,
                    now,
                    now,
                    expires_at,
                    ip,
                    user_agent
                ],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn delete_for_user(state: &AppState, user_id: &str) -> Result<(), AppError> {
    let user_id = user_id.to_string();
    state
        .db
        .call(move |conn| {
            conn.execute(
                "DELETE FROM auth_sessions WHERE user_id = ?1",
                tokio_rusqlite::params![user_id],
            )?;
            Ok(())
        })
        .await?;
    Ok(())
}

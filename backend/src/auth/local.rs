use crate::{
    auth::{audit, session},
    credentials::{self},
    db,
    error::AppError,
    repo::{org, user},
    state::AppState,
};
use regex::Regex;
use std::sync::OnceLock;

static USERNAME_RE: OnceLock<Regex> = OnceLock::new();

fn username_re() -> &'static Regex {
    USERNAME_RE.get_or_init(|| Regex::new(r"^[a-z0-9_-]{1,32}$").unwrap())
}

pub fn validate_username(username: &str) -> Result<(), AppError> {
    if username_re().is_match(username) {
        Ok(())
    } else {
        Err(AppError::BadRequest("invalid username".into()))
    }
}

pub async fn login(
    state: &AppState,
    username: &str,
    cursor_api_key: Option<&str>,
    use_env_key: bool,
    ip: &str,
    user_agent: &str,
) -> Result<(String, String), AppError> {
    validate_username(username)?;
    org::ensure_singleton_local(state).await?;

    let key = if use_env_key {
        state
            .keystore
            .take_launch_key_hint()
            .ok_or_else(|| AppError::BadRequest("no env key available".into()))?
    } else if let Some(k) = cursor_api_key {
        k.to_string()
    } else {
        String::new()
    };

    let user_row = match user::get_by_username(state, username).await? {
        Some(u) => u,
        None => {
            if key.is_empty() {
                audit::record(
                    state,
                    Some(org::LOCAL_ORG_ID),
                    None,
                    "login_failure",
                    ip,
                    user_agent,
                    serde_json::json!({ "username": username, "reason": "missing_key" }),
                )
                .await?;
                return Err(AppError::BadRequest("cursor API key required".into()));
            }
            user::insert(state, username).await?
        }
    };

    user::ensure_membership(state, org::LOCAL_ORG_ID, &user_row.id, "Owner").await?;

    if key.is_empty() {
        let existing = state.keystore.get(&user_row.id).await.map_err(AppError::Internal)?;
        if existing.is_none() {
            audit::record(
                state,
                Some(org::LOCAL_ORG_ID),
                Some(&user_row.id),
                "login_failure",
                ip,
                user_agent,
                serde_json::json!({ "username": username, "reason": "missing_key" }),
            )
            .await?;
            return Err(AppError::BadRequest("cursor API key required".into()));
        }
    } else {
        state
            .keystore
            .set(&user_row.id, &key)
            .await
            .map_err(AppError::Internal)?;
    }

    credentials::write_last_username(&state.data_dir, username)
        .await
        .map_err(AppError::Internal)?;

    let session_id = session::random_session_id();
    let returned_session_id = session_id.clone();
    let user_id = user_row.id.clone();
    let username_json = username.to_string();
    let ip = ip.to_string();
    let user_agent = user_agent.to_string();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM auth_sessions WHERE user_id = ?1",
                tokio_rusqlite::params![user_id.clone()],
            )?;
            let now = db::unix_seconds();
            let expires_at = now + session::ABSOLUTE_LIFETIME_SECS;
            tx.execute(
                "INSERT INTO auth_sessions (id, user_id, org_id, created_at, last_seen_at, expires_at, ip, user_agent) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                tokio_rusqlite::params![
                    session_id.clone(),
                    user_id.clone(),
                    org::LOCAL_ORG_ID,
                    now,
                    now,
                    expires_at,
                    ip.clone(),
                    user_agent.clone()
                ],
            )?;
            audit::record_in_tx(
                &*tx,
                Some(org::LOCAL_ORG_ID),
                Some(&user_id),
                "login_success",
                &ip,
                &user_agent,
                serde_json::json!({ "username": username_json }),
            )?;
            audit::record_in_tx(
                &*tx,
                Some(org::LOCAL_ORG_ID),
                Some(&user_id),
                "session_rotated",
                &ip,
                &user_agent,
                serde_json::json!({}),
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;

    Ok((returned_session_id, user_row.id))
}

pub async fn resolve_me(
    state: &AppState,
    cookie_header: &str,
) -> Result<super::principal::CurrentPrincipal, AppError> {
    let session_id = session::parse_cookie(cookie_header).ok_or(AppError::Unauthorized)?;
    let row = session::load(state, &session_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let now = db::unix_seconds();
    if session::is_expired(&row, now) {
        session::delete(state, &session_id).await?;
        return Err(AppError::Unauthorized);
    }
    session::refresh_last_seen(state, &session_id, now).await?;
    super::principal::load_principal_from_session(state, &row).await
}

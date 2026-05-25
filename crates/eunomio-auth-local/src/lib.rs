// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use eunomio_core::{
    principal::{AuthSessionRow, CurrentPrincipal},
    traits::{AuthProvider, Datastore, KeyStore, LoginRequest, SetupResponse},
    unix_seconds, AppError,
};
use getrandom::getrandom;
use regex::Regex;
use std::{path::Path, sync::Arc, sync::OnceLock};

pub const COOKIE_NAME: &str = "eunomio_local_session";
pub const LOCAL_ORG_ID: &str = "local";
pub const ABSOLUTE_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;
pub const IDLE_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

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

fn is_expired(row: &AuthSessionRow, now: i64) -> bool {
    now >= row.expires_at || now - row.last_seen_at >= IDLE_LIFETIME_SECS
}

pub async fn read_last_username(data_dir: &Path) -> Option<String> {
    let path = data_dir.join("last_username");
    tokio::fs::read_to_string(&path)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub async fn write_last_username(data_dir: &Path, username: &str) -> Result<(), AppError> {
    let path = data_dir.join("last_username");
    tokio::fs::write(&path, username)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

pub async fn suggested_username(data_dir: &Path) -> String {
    if let Some(u) = read_last_username(data_dir).await {
        return u;
    }
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into())
}

pub struct LocalAuthProvider {
    datastore: Arc<dyn Datastore>,
    keystore: Arc<dyn KeyStore>,
    data_dir: std::path::PathBuf,
}

impl LocalAuthProvider {
    pub fn new(
        datastore: Arc<dyn Datastore>,
        keystore: Arc<dyn KeyStore>,
        data_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            datastore,
            keystore,
            data_dir,
        }
    }

    async fn load_principal_from_session(
        &self,
        session_row: &AuthSessionRow,
    ) -> Result<CurrentPrincipal, AppError> {
        let user = self
            .datastore
            .users()
            .get_by_id(&session_row.user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let role = self
            .datastore
            .users()
            .membership_role(&session_row.org_id, &session_row.user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        Ok(CurrentPrincipal {
            user_id: session_row.user_id.clone(),
            org_id: session_row.org_id.clone(),
            role,
            username: user.username,
        })
    }
}

#[async_trait]
impl AuthProvider for LocalAuthProvider {
    async fn setup(&self) -> Result<SetupResponse, AppError> {
        Ok(SetupResponse {
            suggested_username: suggested_username(&self.data_dir).await,
            has_env_key: self.keystore.has_launch_key_hint(),
        })
    }

    async fn login(
        &self,
        req: LoginRequest,
        ip: &str,
        user_agent: &str,
    ) -> Result<String, AppError> {
        validate_username(&req.username)?;
        self.datastore.orgs().ensure_singleton_local().await?;

        let key = if req.use_env_key {
            self.keystore
                .take_launch_key_hint()
                .ok_or_else(|| AppError::BadRequest("no env key available".into()))?
        } else {
            req.cursor_api_key.unwrap_or_default()
        };

        let user_row = match self
            .datastore
            .users()
            .get_by_username(&req.username)
            .await?
        {
            Some(u) => u,
            None => {
                if key.is_empty() {
                    self.datastore
                        .auth_events()
                        .insert(
                            Some(LOCAL_ORG_ID),
                            None,
                            "login_failure",
                            ip,
                            user_agent,
                            serde_json::json!({ "username": req.username, "reason": "missing_key" }),
                        )
                        .await?;
                    return Err(AppError::BadRequest("cursor API key required".into()));
                }
                self.datastore.users().insert(&req.username).await?
            }
        };

        self.datastore
            .users()
            .ensure_membership(LOCAL_ORG_ID, &user_row.id, "Owner")
            .await?;

        if key.is_empty() {
            let existing = self
                .keystore
                .get(&user_row.id)
                .await
                .map_err(AppError::Internal)?;
            if existing.is_none() {
                self.datastore
                    .auth_events()
                    .insert(
                        Some(LOCAL_ORG_ID),
                        Some(&user_row.id),
                        "login_failure",
                        ip,
                        user_agent,
                        serde_json::json!({ "username": req.username, "reason": "missing_key" }),
                    )
                    .await?;
                return Err(AppError::BadRequest("cursor API key required".into()));
            }
        } else {
            self.keystore
                .set(&user_row.id, &key)
                .await
                .map_err(AppError::Internal)?;
        }

        write_last_username(&self.data_dir, &req.username).await?;

        let session_id = random_session_id();
        let now = unix_seconds();
        let expires_at = now + ABSOLUTE_LIFETIME_SECS;
        self.datastore
            .auth_sessions()
            .rotate_with_audit(
                &user_row.id,
                LOCAL_ORG_ID,
                &session_id,
                expires_at,
                ip,
                user_agent,
                &req.username,
            )
            .await?;

        Ok(session_id)
    }

    async fn logout(
        &self,
        session_id: &str,
        org_id: &str,
        user_id: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError> {
        self.datastore
            .auth_sessions()
            .delete_with_audit(session_id, org_id, user_id, ip, user_agent)
            .await
    }

    async fn logout_from_cookie(
        &self,
        cookie_header: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError> {
        let session_id = parse_cookie(cookie_header).ok_or(AppError::Unauthorized)?;
        let row = self
            .datastore
            .auth_sessions()
            .load(&session_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        self.logout(&session_id, &row.org_id, &row.user_id, ip, user_agent)
            .await
    }

    async fn resolve_principal(&self, cookie_header: &str) -> Result<CurrentPrincipal, AppError> {
        let session_id = parse_cookie(cookie_header).ok_or(AppError::Unauthorized)?;
        let row = self
            .datastore
            .auth_sessions()
            .load(&session_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let now = unix_seconds();
        if is_expired(&row, now) {
            self.datastore.auth_sessions().delete(&session_id).await?;
            return Err(AppError::Unauthorized);
        }
        self.datastore
            .auth_sessions()
            .refresh_last_seen(&session_id, now)
            .await?;
        self.load_principal_from_session(&row).await
    }

    async fn set_credentials(
        &self,
        principal: &CurrentPrincipal,
        cursor_api_key: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError> {
        if cursor_api_key.trim().is_empty() {
            return Err(AppError::BadRequest("cursor API key required".into()));
        }
        self.keystore
            .set(&principal.user_id, cursor_api_key)
            .await
            .map_err(AppError::Internal)?;
        self.datastore
            .auth_events()
            .insert(
                Some(&principal.org_id),
                Some(&principal.user_id),
                "credentials_changed",
                ip,
                user_agent,
                serde_json::json!({}),
            )
            .await?;
        Ok(())
    }

    fn session_cookie_name(&self) -> &str {
        COOKIE_NAME
    }

    fn serialize_cookie(&self, session_id: &str) -> String {
        format!("{COOKIE_NAME}={session_id}; HttpOnly; Path=/; SameSite=Lax")
    }

    fn clear_cookie(&self) -> String {
        format!("{COOKIE_NAME}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0")
    }

    fn refresh_session_cookie(&self, cookie_header: &str) -> Option<String> {
        parse_cookie(cookie_header).map(|session_id| self.serialize_cookie(&session_id))
    }
}

pub use eunomio_core::traits::{
    LoginRequest as AuthLoginRequest, PatchCredentialsRequest as AuthPatchCredentialsRequest,
};

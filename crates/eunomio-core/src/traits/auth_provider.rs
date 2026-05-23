// SPDX-License-Identifier: Apache-2.0

use crate::{error::AppError, principal::CurrentPrincipal};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResponse {
    pub suggested_username: String,
    pub has_env_key: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    #[serde(default)]
    pub cursor_api_key: Option<String>,
    #[serde(default)]
    pub use_env_key: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchCredentialsRequest {
    pub cursor_api_key: String,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn setup(&self) -> Result<SetupResponse, AppError>;
    async fn login(
        &self,
        req: LoginRequest,
        ip: &str,
        user_agent: &str,
    ) -> Result<String, AppError>;
    async fn logout(
        &self,
        session_id: &str,
        org_id: &str,
        user_id: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError>;
    /// Parse the deployment's session cookie and revoke the server-side session.
    /// Errors are non-fatal for logout UX (handlers clear the cookie regardless).
    async fn logout_from_cookie(
        &self,
        cookie_header: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError>;
    async fn resolve_principal(&self, cookie_header: &str) -> Result<CurrentPrincipal, AppError>;
    async fn set_credentials(
        &self,
        principal: &CurrentPrincipal,
        cursor_api_key: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<(), AppError>;
    fn session_cookie_name(&self) -> &str;
    fn serialize_cookie(&self, session_id: &str) -> String;
    fn clear_cookie(&self) -> String;
    /// Re-serialize the session cookie for Set-Cookie after `resolve_principal`
    /// extended idle timeout. Impl must use `session_cookie_name()` and
    /// `serialize_cookie()` consistently (e.g. local vs `__Host-` hosted cookies).
    fn refresh_session_cookie(&self, cookie_header: &str) -> Option<String>;
}

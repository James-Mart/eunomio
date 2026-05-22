use super::{
    local, principal::{CurrentPrincipal, PrincipalResponse},
    session,
};
use crate::{credentials, error::AppError, state::AppState};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;

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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSetupResponse {
    pub suggested_username: String,
    pub has_env_key: bool,
}

fn request_meta(headers: &HeaderMap) -> (&str, &str) {
    let ip = "127.0.0.1";
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    (ip, user_agent)
}

pub fn public_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/setup", get(get_setup))
        .route("/api/auth/login", post(post_login))
        .route("/api/me", get(get_me))
}

pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/logout", post(post_logout))
        .route("/api/auth/credentials", patch(patch_credentials))
}

async fn get_setup(State(state): State<AppState>) -> Result<Json<AuthSetupResponse>, AppError> {
    Ok(Json(AuthSetupResponse {
        suggested_username: credentials::suggested_username(&state.data_dir).await,
        has_env_key: state.keystore.has_launch_key_hint(),
    }))
}

async fn post_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Response, AppError> {
    let (ip, user_agent) = request_meta(&headers);
    let (session_id, _user_id) = local::login(
        &state,
        &body.username,
        body.cursor_api_key.as_deref(),
        body.use_env_key,
        ip,
        user_agent,
    )
    .await?;
    let cookie = session::serialize_cookie(&session_id);
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response())
}

async fn get_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match local::resolve_me(&state, cookie).await {
        Ok(principal) => Ok((
            StatusCode::OK,
            [(header::SET_COOKIE, session::serialize_cookie(
                &session::parse_cookie(cookie).unwrap_or_default(),
            ))],
            Json(PrincipalResponse::from(principal)),
        )
            .into_response()),
        Err(AppError::Unauthorized) => Ok(unauthorized_response()),
        Err(e) => Err(e),
    }
}

async fn post_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: CurrentPrincipal,
) -> Result<Response, AppError> {
    let (ip, user_agent) = request_meta(&headers);
    if let Some(session_id) = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(session::parse_cookie)
    {
        let org_id = principal.org_id.clone();
        let user_id = principal.user_id.clone();
        let ip = ip.to_string();
        let user_agent = user_agent.to_string();
        state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM auth_sessions WHERE id = ?1",
                    tokio_rusqlite::params![session_id],
                )?;
                super::audit::record_in_tx(
                    &*tx,
                    Some(&org_id),
                    Some(&user_id),
                    "logout",
                    &ip,
                    &user_agent,
                    serde_json::json!({}),
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;
    }
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, session::clear_cookie())],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response())
}

async fn patch_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: CurrentPrincipal,
    Json(body): Json<PatchCredentialsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if body.cursor_api_key.trim().is_empty() {
        return Err(AppError::BadRequest("cursor API key required".into()));
    }
    let (ip, user_agent) = request_meta(&headers);
    state
        .keystore
        .set(&principal.user_id, &body.cursor_api_key)
        .await
        .map_err(AppError::Internal)?;
    super::audit::record(
        &state,
        Some(&principal.org_id),
        Some(&principal.user_id),
        "credentials_changed",
        ip,
        user_agent,
        serde_json::json!({}),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "unauthenticated", "code": "unauthenticated" })),
    )
        .into_response()
}

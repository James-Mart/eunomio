// SPDX-License-Identifier: Apache-2.0

use super::principal_extractor::PrincipalResponse;
use crate::{state::AppState, AppError, ServerError};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use eunomio_core::principal::CurrentPrincipal;
use eunomio_core::traits::{LoginRequest, PatchCredentialsRequest, SetupResponse};

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

async fn get_setup(State(state): State<AppState>) -> Result<Json<SetupResponse>, ServerError> {
    Ok(Json(state.auth.setup().await?))
}

async fn post_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Response, ServerError> {
    let (ip, user_agent) = request_meta(&headers);
    let session_id = state.auth.login(body, ip, user_agent).await?;
    let cookie = state.auth.serialize_cookie(&session_id);
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
) -> Result<Response, ServerError> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match state.auth.resolve_principal(cookie).await {
        Ok(principal) => {
            let set_cookie = state
                .auth
                .refresh_session_cookie(cookie)
                .unwrap_or_else(|| state.auth.clear_cookie());
            Ok((
                StatusCode::OK,
                [(header::SET_COOKIE, set_cookie)],
                Json(PrincipalResponse::from(principal)),
            )
                .into_response())
        }
        Err(AppError::Unauthorized) => Ok(unauthorized_response()),
        Err(e) => Err(e.into()),
    }
}

async fn post_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ServerError> {
    let (ip, user_agent) = request_meta(&headers);
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !cookie.is_empty() {
        let _ = state.auth.logout_from_cookie(cookie, ip, user_agent).await;
    }
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, state.auth.clear_cookie())],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response())
}

async fn patch_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
    principal: CurrentPrincipal,
    Json(body): Json<PatchCredentialsRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let (ip, user_agent) = request_meta(&headers);
    state
        .auth
        .set_credentials(&principal, &body.cursor_api_key, ip, user_agent)
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

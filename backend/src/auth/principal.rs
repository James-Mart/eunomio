use crate::{error::AppError, state::AppState};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CurrentPrincipal {
    pub user_id: String,
    pub org_id: String,
    pub role: String,
    pub username: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrincipalResponse {
    pub user_id: String,
    pub org_id: String,
    pub role: String,
    pub username: String,
}

impl From<CurrentPrincipal> for PrincipalResponse {
    fn from(p: CurrentPrincipal) -> Self {
        Self {
            user_id: p.user_id,
            org_id: p.org_id,
            role: p.role,
            username: p.username,
        }
    }
}

pub struct PrincipalMissing;

impl IntoResponse for PrincipalMissing {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "CurrentPrincipal missing; handler not behind require_principal layer"
            })),
        )
            .into_response()
    }
}

#[async_trait]
impl FromRequestParts<AppState> for CurrentPrincipal {
    type Rejection = PrincipalMissing;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentPrincipal>()
            .cloned()
            .ok_or(PrincipalMissing)
    }
}

pub async fn load_principal_from_session(
    state: &AppState,
    session_row: &super::session::AuthSessionRow,
) -> Result<CurrentPrincipal, AppError> {
    let user = crate::repo::user::get_by_id(state, &session_row.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let role = crate::repo::user::membership_role(state, &session_row.org_id, &session_row.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(CurrentPrincipal {
        user_id: session_row.user_id.clone(),
        org_id: session_row.org_id.clone(),
        role,
        username: user.username,
    })
}

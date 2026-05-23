// SPDX-License-Identifier: Apache-2.0

use crate::{state::AppState, AppError};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
pub use eunomio_core::principal::{CurrentPrincipal, PrincipalResponse};

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
    session_row: &eunomio_core::principal::AuthSessionRow,
) -> Result<CurrentPrincipal, AppError> {
    let user = state
        .datastore
        .users()
        .get_by_id(&session_row.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let role = state
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

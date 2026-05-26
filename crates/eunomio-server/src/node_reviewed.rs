// SPDX-License-Identifier: Apache-2.0

use crate::{auth::CurrentPrincipal, state::AppState, AppError, ServerError};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use eunomio_core::{types::*, unix_seconds};

pub async fn put_node_reviewed(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((session_id, node_id)): Path<(String, String)>,
    Json(body): Json<SetNodeReviewedRequest>,
) -> Result<StatusCode, ServerError> {
    state
        .datastore
        .sessions()
        .ensure(&principal.org_id, &session_id)
        .await?;
    if state
        .datastore
        .nodes()
        .get(&principal.org_id, &session_id, &node_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound.into());
    }
    state
        .datastore
        .node_reviewed()
        .set_reviewed(
            &principal.org_id,
            &principal.user_id,
            &session_id,
            &node_id,
            body.reviewed,
            unix_seconds(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

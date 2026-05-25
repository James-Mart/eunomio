// SPDX-License-Identifier: Apache-2.0

use crate::{auth::CurrentPrincipal, state::AppState, AppError, ServerError};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use eunomio_core::{types::*, unix_seconds};

pub async fn get_edge_viewed(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((session_id, target_node_id)): Path<(String, String)>,
) -> Result<Json<EdgeViewedFiles>, ServerError> {
    state
        .datastore
        .sessions()
        .ensure(&principal.org_id, &session_id)
        .await?;
    if state
        .datastore
        .nodes()
        .get(&principal.org_id, &session_id, &target_node_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound.into());
    }
    let paths = state
        .datastore
        .edge_file_viewed()
        .list_paths(
            &principal.org_id,
            &principal.user_id,
            &session_id,
            &target_node_id,
        )
        .await?;
    Ok(Json(EdgeViewedFiles { paths }))
}

pub async fn put_edge_file_viewed(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((session_id, target_node_id, file_path)): Path<(String, String, String)>,
    Json(body): Json<SetEdgeFileViewedRequest>,
) -> Result<StatusCode, ServerError> {
    if file_path.is_empty() {
        return Err(AppError::BadRequest("file path required".into()).into());
    }
    state
        .datastore
        .sessions()
        .ensure(&principal.org_id, &session_id)
        .await?;
    if state
        .datastore
        .nodes()
        .get(&principal.org_id, &session_id, &target_node_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound.into());
    }
    state
        .datastore
        .edge_file_viewed()
        .set_viewed(
            &principal.org_id,
            &principal.user_id,
            &session_id,
            &target_node_id,
            &file_path,
            body.viewed,
            unix_seconds(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// SPDX-License-Identifier: Apache-2.0

use crate::{state::AppState, ServerError};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

pub async fn require_principal(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ServerError> {
    let cookie_header = req
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let principal = state.auth.resolve_principal(cookie_header).await?;
    req.extensions_mut().insert(principal);
    Ok(next.run(req).await)
}

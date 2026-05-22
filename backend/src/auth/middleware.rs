use crate::{auth::session, db, error::AppError, state::AppState};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

pub async fn require_principal(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let cookie_header = req
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let session_id = session::parse_cookie(cookie_header).ok_or(AppError::Unauthorized)?;
    let row = session::load(&state, &session_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let now = db::unix_seconds();
    if session::is_expired(&row, now) {
        session::delete(&state, &session_id).await?;
        return Err(AppError::Unauthorized);
    }
    session::refresh_last_seen(&state, &session_id, now).await?;
    let principal =
        super::principal::load_principal_from_session(&state, &row).await?;
    req.extensions_mut().insert(principal);
    Ok(next.run(req).await)
}

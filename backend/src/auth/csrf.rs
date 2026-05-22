use crate::error::AppError;
use axum::{extract::Request, middleware::Next, response::Response};

const CSRF_HEADER: &str = "X-Eunomia-Request";

pub async fn require_csrf_header(req: Request, next: Next) -> Result<Response, AppError> {
    let method = req.method().clone();
    if matches!(
        method.as_str(),
        "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        let has_header = req
            .headers()
            .get(CSRF_HEADER)
            .and_then(|v| v.to_str().ok())
            == Some("1");
        if !has_header {
            return Err(AppError::CsrfRejected);
        }
    }
    Ok(next.run(req).await)
}

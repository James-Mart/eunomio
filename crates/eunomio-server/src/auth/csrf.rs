// SPDX-License-Identifier: Apache-2.0

use crate::{AppError, ServerError};
use axum::{extract::Request, middleware::Next, response::Response};

const CSRF_HEADER: &str = "X-Eunomio-Request";

pub async fn require_csrf_header(req: Request, next: Next) -> Result<Response, ServerError> {
    let method = req.method().clone();
    if matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        let has_header = req.headers().get(CSRF_HEADER).and_then(|v| v.to_str().ok()) == Some("1");
        if !has_header {
            return Err(AppError::CsrfRejected.into());
        }
    }
    Ok(next.run(req).await)
}

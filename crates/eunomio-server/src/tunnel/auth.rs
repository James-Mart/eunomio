// SPDX-License-Identifier: Apache-2.0

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use uuid::Uuid;

const SHARE_COOKIE: &str = "eunomio_share_token";
const SHARE_QUERY: &str = "eunomio_token";

pub(super) async fn check_token(
    State(token): State<Uuid>,
    req: Request,
    next: Next,
) -> Response<Body> {
    if cookie_matches(&req, &token) {
        return next.run(req).await;
    }
    if query_matches(&req, &token) {
        return redirect_and_set_cookie(&req, &token);
    }
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "unauthorized\n",
    )
        .into_response()
}

fn cookie_matches(req: &Request, token: &Uuid) -> bool {
    let token_str = token.to_string();
    req.headers().get_all(header::COOKIE).iter().any(|v| {
        v.to_str()
            .ok()
            .and_then(|s| extract_value(s, SHARE_COOKIE, ';'))
            .map(|val| val == token_str)
            .unwrap_or(false)
    })
}

fn query_matches(req: &Request, token: &Uuid) -> bool {
    let Some(query) = req.uri().query() else {
        return false;
    };
    extract_value(query, SHARE_QUERY, '&')
        .map(|v| v == token.to_string())
        .unwrap_or(false)
}

fn redirect_and_set_cookie(req: &Request, token: &Uuid) -> Response<Body> {
    let path = req.uri().path();
    let new_query = req
        .uri()
        .query()
        .map(|q| strip_query_param(q, SHARE_QUERY))
        .unwrap_or_default();
    let target = if new_query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{new_query}")
    };
    let cookie = format!("{SHARE_COOKIE}={token}; HttpOnly; Secure; SameSite=Lax; Path=/");
    let location =
        HeaderValue::from_str(&target).unwrap_or_else(|_| HeaderValue::from_static("/"));
    let cookie_header =
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""));
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            (header::SET_COOKIE, cookie_header),
        ],
    )
        .into_response()
}

/// Iterate `key=value` pairs separated by `sep`, trimming whitespace around
/// keys (the cookie header allows `; ` between pairs while query strings
/// don't). Used by both the cookie and query-string code paths.
fn parse_kv_pairs(input: &str, sep: char) -> impl Iterator<Item = (&str, &str)> {
    input.split(sep).filter_map(|part| {
        let part = part.trim();
        part.split_once('=')
    })
}

fn extract_value<'a>(input: &'a str, key: &str, sep: char) -> Option<&'a str> {
    parse_kv_pairs(input, sep).find_map(|(k, v)| (k == key).then_some(v))
}

fn strip_query_param(query: &str, key: &str) -> String {
    query
        .split('&')
        .filter(|p| !p.starts_with(&format!("{key}=")) && *p != key)
        .collect::<Vec<_>>()
        .join("&")
}

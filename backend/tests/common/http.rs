#![allow(dead_code)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use eunomio::auth::session::COOKIE_NAME;
use http_body_util::BodyExt;
use tower::ServiceExt;

const CSRF_HEADER: &str = "X-Eunomio-Request";

pub fn parse_session_cookie(set_cookie: &str) -> String {
    for part in set_cookie.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
            if !value.is_empty() {
                return format!("{COOKIE_NAME}={value}");
            }
        }
    }
    panic!("no {COOKIE_NAME} in Set-Cookie: {set_cookie}");
}

fn csrf_request(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(CSRF_HEADER, "1")
        .header(header::HOST, "127.0.0.1")
}

async fn send(
    router: &Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            panic!(
                "non-JSON body: {e}; body={}",
                String::from_utf8_lossy(&bytes)
            )
        })
    };
    (status, headers, value)
}

pub async fn login(router: &Router, username: &str, cursor_api_key: &str) -> String {
    let req = csrf_request("POST", "/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "username": username,
                "cursorApiKey": cursor_api_key,
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, headers, body) = send(router, req).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "login failed for {username}: {body}"
    );
    let set_cookie = headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login response missing Set-Cookie");
    parse_session_cookie(set_cookie)
}

pub async fn json_request(
    router: &Router,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

pub async fn empty_request(
    router: &Router,
    method: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path).body(Body::empty()).unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

pub async fn request_with_headers(
    router: &Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    send(router, req).await
}

pub async fn authenticated_json_request(
    router: &Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path)
        .header("content-type", "application/json")
        .header(header::COOKIE, cookie)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

pub async fn authenticated_empty_request(
    router: &Router,
    cookie: &str,
    method: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

use axum::http::{header, Request, StatusCode};
use axum::body::Body;
use eunomio::auth::session::{ABSOLUTE_LIFETIME_SECS, COOKIE_NAME, IDLE_LIFETIME_SECS};
use eunomio::db;
use pretty_assertions::assert_eq;

mod common;
use common::{
    app::{TestApp, TEST_CURSOR_KEY, TEST_USERNAME},
    git::local_session_body,
    http::{
        authenticated_empty_request, authenticated_json_request, empty_request, json_request,
        login, parse_session_cookie, request_with_headers,
    },
};

async fn count_auth_events(state: &eunomio::state::AppState, event_type: &str) -> i64 {
    state
        .db
        .call({
            let event_type = event_type.to_string();
            move |conn| {
                let n: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM auth_events WHERE event_type = ?1",
                    tokio_rusqlite::params![event_type],
                    |r| r.get(0),
                )?;
                Ok(n)
            }
        })
        .await
        .unwrap()
}

async fn count_auth_sessions(state: &eunomio::state::AppState) -> i64 {
    state
        .db
        .call(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM auth_sessions",
                tokio_rusqlite::params![],
                |r| r.get(0),
            )?;
            Ok(n)
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn me_without_cookie_returns_401() {
    let app = TestApp::spawn().await;
    let (status, body) = empty_request(&app.router, "GET", "/api/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"].as_str().unwrap(), "unauthenticated");
}

#[tokio::test]
async fn setup_returns_suggested_username() {
    let app = TestApp::spawn().await;
    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/setup")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = request_with_headers(&app.router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["suggestedUsername"].is_string());
    assert_eq!(body["hasEnvKey"].as_bool().unwrap(), false);
}

#[tokio::test]
async fn setup_has_env_key_when_hint_set() {
    let app = TestApp::spawn_with_launch_key_hint().await;
    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/setup")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = request_with_headers(&app.router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hasEnvKey"].as_bool().unwrap(), true);
    let serialized = body.to_string();
    assert!(
        !serialized.contains("env-launch-key"),
        "response must not leak launch key"
    );
}

#[tokio::test]
async fn login_creates_session_cookie() {
    let app = TestApp::spawn().await;
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::HOST, "127.0.0.1")
        .header("content-type", "application/json")
        .header("X-Eunomio-Request", "1")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "username": TEST_USERNAME,
                "cursorApiKey": TEST_CURSOR_KEY,
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, headers, _) = request_with_headers(&app.router, req).await;
    assert_eq!(status, StatusCode::OK);
    let set_cookie = headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("Set-Cookie");
    assert!(set_cookie.contains(&format!("{COOKIE_NAME}=")));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    let _ = parse_session_cookie(set_cookie);
}

#[tokio::test]
async fn login_rejects_invalid_username() {
    let app = TestApp::spawn().await;
    let (status, _) = json_request(
        &app.router,
        "POST",
        "/api/auth/login",
        serde_json::json!({
            "username": "Bad User!",
            "cursorApiKey": TEST_CURSOR_KEY,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn login_with_use_env_key() {
    let app = TestApp::spawn_with_launch_key_hint().await;
    let (status, _) = json_request(
        &app.router,
        "POST",
        "/api/auth/login",
        serde_json::json!({
            "username": "envuser",
            "useEnvKey": true,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = json_request(
        &app.router,
        "POST",
        "/api/auth/login",
        serde_json::json!({
            "username": "envuser2",
            "useEnvKey": true,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn login_rejects_missing_csrf_header() {
    let app = TestApp::spawn().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::HOST, "127.0.0.1")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "username": TEST_USERNAME,
                "cursorApiKey": TEST_CURSOR_KEY,
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, _, body) = request_with_headers(&app.router, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"].as_str().unwrap(), "csrf_rejected");
}

#[tokio::test]
async fn login_failure_writes_audit_event() {
    let app = TestApp::spawn().await;
    let before = count_auth_events(&app.state, "login_failure").await;
    let (status, _) = json_request(
        &app.router,
        "POST",
        "/api/auth/login",
        serde_json::json!({ "username": "audit-fail-user" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let after = count_auth_events(&app.state, "login_failure").await;
    assert_eq!(after, before + 1);
}

#[tokio::test]
async fn login_requires_key_for_new_user() {
    let app = TestApp::spawn().await;
    let (status, _) = json_request(
        &app.router,
        "POST",
        "/api/auth/login",
        serde_json::json!({ "username": "brand-new-user" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn me_after_login_returns_principal() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let (status, body) =
        authenticated_empty_request(&app.router, &cookie, "GET", "/api/me").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["userId"].is_string());
    assert_eq!(body["orgId"].as_str().unwrap(), "local");
    assert_eq!(body["role"].as_str().unwrap(), "Owner");
    assert_eq!(body["username"].as_str().unwrap(), TEST_USERNAME);
}

#[tokio::test]
async fn protected_route_without_cookie_401() {
    let app = TestApp::spawn().await;
    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        local_session_body(&app.repo_path(), "main", "feature"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"].as_str().unwrap(), "unauthenticated");
}

#[tokio::test]
async fn logout_clears_session() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    assert_eq!(count_auth_sessions(&app.state).await, 1);

    let (status, _) =
        authenticated_empty_request(&app.router, &cookie, "POST", "/api/auth/logout").await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) =
        authenticated_empty_request(&app.router, &cookie, "GET", "/api/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"].as_str().unwrap(), "unauthenticated");
    assert_eq!(count_auth_sessions(&app.state).await, 0);
}

#[tokio::test]
async fn patch_credentials_updates_keystore() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let (_, me) = authenticated_empty_request(&app.router, &cookie, "GET", "/api/me").await;
    let user_id = me["userId"].as_str().unwrap().to_string();

    let (status, _) = authenticated_json_request(
        &app.router,
        &cookie,
        "PATCH",
        "/api/auth/credentials",
        serde_json::json!({ "cursorApiKey": "updated-key" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let stored = app
        .state
        .keystore
        .get(&user_id)
        .await
        .unwrap()
        .expect("credentials file");
    assert_eq!(stored, "updated-key");
}

#[tokio::test]
async fn csrf_rejects_mutating_without_header() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/sessions")
        .header(header::HOST, "127.0.0.1")
        .header("content-type", "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(
            serde_json::to_vec(&local_session_body(
                &app.repo_path(),
                "main",
                "feature",
            ))
            .unwrap(),
        ))
        .unwrap();
    let (status, _, body) = request_with_headers(&app.router, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"].as_str().unwrap(), "csrf_rejected");
}

#[tokio::test]
async fn csrf_allows_get_without_header() {
    let app = TestApp::spawn().await;
    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/setup")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = request_with_headers(&app.router, req).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn login_writes_audit_event() {
    let app = TestApp::spawn().await;
    let before = count_auth_events(&app.state, "login_success").await;
    let _ = login(&app.router, "audit-user", TEST_CURSOR_KEY).await;
    let after = count_auth_events(&app.state, "login_success").await;
    assert_eq!(after, before + 1);
}

#[tokio::test]
async fn logout_writes_audit_event() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let before = count_auth_events(&app.state, "logout").await;
    let _ = authenticated_empty_request(&app.router, &cookie, "POST", "/api/auth/logout").await;
    let after = count_auth_events(&app.state, "logout").await;
    assert_eq!(after, before + 1);
}

#[tokio::test]
async fn credentials_patch_writes_audit_event() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let before = count_auth_events(&app.state, "credentials_changed").await;
    let _ = authenticated_json_request(
        &app.router,
        &cookie,
        "PATCH",
        "/api/auth/credentials",
        serde_json::json!({ "cursorApiKey": "audit-key" }),
    )
    .await;
    let after = count_auth_events(&app.state, "credentials_changed").await;
    assert_eq!(after, before + 1);
}

#[tokio::test]
async fn session_idle_expiry_returns_401() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let session_id = cookie
        .strip_prefix(&format!("{COOKIE_NAME}="))
        .unwrap()
        .to_string();

    let stale = db::unix_seconds() - IDLE_LIFETIME_SECS - 1;
    app.state
        .db
        .call({
            let session_id = session_id.to_string();
            move |conn| {
                conn.execute(
                    "UPDATE auth_sessions SET last_seen_at = ?1 WHERE id = ?2",
                    tokio_rusqlite::params![stale, session_id],
                )?;
                Ok(())
            }
        })
        .await
        .unwrap();

    let (status, body) =
        authenticated_empty_request(&app.router, &cookie, "GET", "/api/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"].as_str().unwrap(), "unauthenticated");
}

#[tokio::test]
async fn session_absolute_expiry_returns_401() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
    let session_id = cookie
        .strip_prefix(&format!("{COOKIE_NAME}="))
        .unwrap()
        .to_string();

    let expired = db::unix_seconds() - ABSOLUTE_LIFETIME_SECS - 1;
    app.state
        .db
        .call(move |conn| {
            conn.execute(
                "UPDATE auth_sessions SET expires_at = ?1 WHERE id = ?2",
                tokio_rusqlite::params![expired, session_id],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let (status, body) =
        authenticated_empty_request(&app.router, &cookie, "GET", "/api/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"].as_str().unwrap(), "unauthenticated");
}

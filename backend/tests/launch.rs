mod common;

use axum::http::StatusCode;
use common::{
    app::TestApp,
    http::{empty_request, json_request},
};

const PR_URL: &str = "https://github.com/org/repo/pull/42";

#[tokio::test]
async fn launch_pull_request_is_one_shot() {
    let app = TestApp::spawn_with_launch_pull_request(PR_URL).await;

    let (status, body) = empty_request(&app.router, "GET", "/api/launch/pull-request").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pullRequestUrl"].as_str(), Some(PR_URL));

    let (status, body) = empty_request(&app.router, "GET", "/api/launch/pull-request").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["pullRequestUrl"].is_null());
}

#[tokio::test]
async fn launch_pull_request_absent_by_default() {
    let app = TestApp::spawn().await;

    let (status, body) = empty_request(&app.router, "GET", "/api/launch/pull-request").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["pullRequestUrl"].is_null());
}

#[tokio::test]
async fn launch_pull_request_does_not_require_auth() {
    let app = TestApp::spawn_with_launch_pull_request(PR_URL).await;

    let (status, _) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        serde_json::json!({
            "remoteUrl": "https://github.com/org/repo.git",
            "baseRef": "main",
            "sourceRef": "feature",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = empty_request(&app.router, "GET", "/api/launch/pull-request").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pullRequestUrl"].as_str(), Some(PR_URL));
}

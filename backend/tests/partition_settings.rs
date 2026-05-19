use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;

mod common;
use common::{empty_request, json_request, TestApp};

#[tokio::test]
async fn partition_settings_round_trip() {
    let app = TestApp::spawn().await;

    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "feature" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create body: {body}");
    let session_id = body["id"].as_str().unwrap().to_string();

    let (status, body) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/partition-settings"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["surveyor"]["model"].as_str().unwrap(), "composer-2");

    let (status, body) = json_request(
        &app.router,
        "PATCH",
        &format!("/api/sessions/{session_id}/partition-settings"),
        json!({ "surveyor": { "model": "composer-3" } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["surveyor"]["model"].as_str().unwrap(), "composer-3");

    let (status, body) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/partition-settings"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["surveyor"]["model"].as_str().unwrap(), "composer-3");

    let (status, body) = json_request(
        &app.router,
        "PATCH",
        &format!("/api/sessions/{session_id}/partition-settings"),
        json!({ "coordinator": { "humanInTheLoop": { "afterSurvey": true } } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["coordinator"]["humanInTheLoop"]["afterSurvey"]
            .as_bool()
            .unwrap(),
        true
    );
    assert_eq!(
        body["coordinator"]["humanInTheLoop"]["afterPlanning"]
            .as_bool()
            .unwrap(),
        false,
        "missing fields default to false"
    );
    assert_eq!(
        body["surveyor"]["model"].as_str().unwrap(),
        "composer-3",
        "surveyor must survive coordinator patch"
    );

    let (status, body) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/partition-settings"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["coordinator"]["humanInTheLoop"]["afterSurvey"]
            .as_bool()
            .unwrap(),
        true
    );
    assert_eq!(body["surveyor"]["model"].as_str().unwrap(), "composer-3");
}

#[tokio::test]
async fn fresh_session_returns_default_hitl_flags() {
    let app = TestApp::spawn().await;

    let (_, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "feature" }),
    )
    .await;
    let session_id = body["id"].as_str().unwrap().to_string();

    let (status, body) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/partition-settings"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["coordinator"]["humanInTheLoop"]["afterSurvey"]
            .as_bool()
            .unwrap(),
        false
    );
    assert_eq!(
        body["coordinator"]["humanInTheLoop"]["afterPlanning"]
            .as_bool()
            .unwrap(),
        false
    );
}

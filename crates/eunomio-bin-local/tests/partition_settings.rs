// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use eunomio_server::partition_settings;
use pretty_assertions::assert_eq;

mod common;
use common::{
    app::{TestApp, TEST_CURSOR_KEY},
    http::{authenticated_empty_request, authenticated_json_request, login},
};

#[tokio::test]
async fn per_user_settings_isolated() {
    let app = TestApp::spawn().await;
    let cookie_a = login(&app.router, "settings-a", TEST_CURSOR_KEY).await;
    let cookie_b = login(&app.router, "settings-b", TEST_CURSOR_KEY).await;

    let (_, me_a) = authenticated_empty_request(&app.router, &cookie_a, "GET", "/api/me").await;
    let (_, me_b) = authenticated_empty_request(&app.router, &cookie_b, "GET", "/api/me").await;
    let user_a = me_a["userId"].as_str().unwrap();
    let user_b = me_b["userId"].as_str().unwrap();

    let (status, _) = authenticated_json_request(
        &app.router,
        &cookie_a,
        "PATCH",
        "/api/partition-settings",
        serde_json::json!({
            "coordinator": { "model": "model-a" }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = authenticated_json_request(
        &app.router,
        &cookie_b,
        "PATCH",
        "/api/partition-settings",
        serde_json::json!({
            "coordinator": { "model": "model-b" }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let path_a = app.data.path().join("users").join(user_a).join("settings.json");
    let path_b = app.data.path().join("users").join(user_b).join("settings.json");
    let text_a = std::fs::read_to_string(&path_a).unwrap();
    let text_b = std::fs::read_to_string(&path_b).unwrap();
    assert_ne!(text_a, text_b);
    assert!(text_a.contains("model-a"));
    assert!(text_b.contains("model-b"));

    let (status, body) =
        authenticated_empty_request(&app.router, &cookie_b, "GET", "/api/partition-settings")
            .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["coordinator"]["model"].as_str().unwrap(), "model-b");
    assert_ne!(body["coordinator"]["model"].as_str().unwrap(), "model-a");
}

#[tokio::test]
async fn global_settings_file_not_read() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, "settings-user", TEST_CURSOR_KEY).await;

    let global_path = app.data.path().join("settings.json");
    std::fs::write(
        &global_path,
        r#"{"coordinator":{"model":"global-should-not-apply"}}"#,
    )
    .unwrap();

    let (status, body) =
        authenticated_empty_request(&app.router, &cookie, "GET", "/api/partition-settings").await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(
        body["coordinator"]["model"].as_str().unwrap(),
        "global-should-not-apply"
    );

    let (_, me) = authenticated_empty_request(&app.router, &cookie, "GET", "/api/me").await;
    let user_id = me["userId"].as_str().unwrap();
    let user_path = app
        .data
        .path()
        .join("users")
        .join(user_id)
        .join("settings.json");
    let settings = partition_settings::load_for_user(app.data.path(), user_id)
        .await
        .unwrap();
    assert_ne!(settings.coordinator.model, "global-should-not-apply");
    assert!(user_path.is_file());
}

#[tokio::test]
async fn surveyor_enabled_defaults_true_and_patches() {
    let app = TestApp::spawn().await;
    let cookie = login(&app.router, "surveyor-setting", TEST_CURSOR_KEY).await;

    let (status, body) =
        authenticated_empty_request(&app.router, &cookie, "GET", "/api/partition-settings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["coordinator"]["surveyorEnabled"].as_bool(),
        Some(true)
    );

    let (status, body) = authenticated_json_request(
        &app.router,
        &cookie,
        "PATCH",
        "/api/partition-settings",
        serde_json::json!({
            "coordinator": { "surveyorEnabled": false }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["coordinator"]["surveyorEnabled"].as_bool(),
        Some(false)
    );
}

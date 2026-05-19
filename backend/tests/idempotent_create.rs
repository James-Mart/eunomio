use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;

mod common;
use common::{empty_request, git, json_request, write, TestApp};

fn repo_with_two_features(path: &Path) {
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "test@example.com"]);
    git(path, &["config", "user.name", "Test"]);
    write(path, "a.txt", "a\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "base commit"]);

    git(path, &["checkout", "-q", "-b", "feature"]);
    write(path, "b.txt", "b\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "add b"]);

    git(path, &["checkout", "-q", "main"]);
    git(path, &["checkout", "-q", "-b", "other"]);
    write(path, "c.txt", "c\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "add c"]);

    git(path, &["checkout", "-q", "main"]);
}

#[tokio::test]
async fn repeated_create_returns_existing_session() {
    let app = TestApp::spawn_with_repo(repo_with_two_features).await;

    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "feature" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "first create body: {body}");
    let first_id = body["id"].as_str().unwrap().to_string();
    let first_created_at = body["createdAt"].as_i64().unwrap();

    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "feature" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "repeat create body: {body}");
    assert_eq!(body["id"].as_str().unwrap(), first_id);
    assert_eq!(body["createdAt"].as_i64().unwrap(), first_created_at);

    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "other" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "other create body: {body}");
    let second_id = body["id"].as_str().unwrap().to_string();
    assert_ne!(second_id, first_id);

    let (status, body) = empty_request(&app.router, "GET", "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let ids: Vec<&str> = arr.iter().map(|v| v["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&first_id.as_str()), "missing first session: {ids:?}");
    assert!(ids.contains(&second_id.as_str()), "missing second session: {ids:?}");
}

// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use std::path::Path;

mod common;
use common::{
    app::TestApp,
    git::{git, local_session_body, write},
};

fn repo_with_feature_branch(path: &Path) {
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
}

#[tokio::test]
async fn validate_session_accepts_valid_local_refs() {
    let app = TestApp::spawn_authenticated_with_repo(repo_with_feature_branch).await;

    let (status, _) = app
        .auth_json(
            "POST",
            "/api/sessions/validate",
            local_session_body(&app.repo_path(), "main", "feature"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn validate_session_rejects_missing_source_ref() {
    let app = TestApp::spawn_authenticated_with_repo(repo_with_feature_branch).await;

    let (status, body) = app
        .auth_json(
            "POST",
            "/api/sessions/validate",
            local_session_body(&app.repo_path(), "main", "no-such-branch"),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

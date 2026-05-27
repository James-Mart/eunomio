// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use pretty_assertions::assert_eq;

mod common;
use common::{
    app::TestApp,
    git::{git, local_session_body},
};

#[tokio::test]
async fn happy_path_create_local_session_without_touching_user_branch() {
    let app = TestApp::spawn_authenticated().await;
    let repo = app.repo_path();

    let (status, body) = app
        .auth_json(
            "POST",
            "/api/sessions",
            local_session_body(&repo, "main", "feature"),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create body: {body}");
    let session_id = body["id"].as_str().unwrap().to_string();
    let base_node_id = body["baseNodeId"].as_str().unwrap().to_string();

    let (status, body) = app
        .auth_empty("GET", &format!("/api/sessions/{session_id}/graph"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let nodes = body["nodes"].as_array().unwrap();
    let edges = body["edges"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "expected base + final nodes");
    assert_eq!(edges.len(), 1, "expected one edge");
    assert_eq!(edges[0]["from"].as_str().unwrap(), base_node_id);

    let final_node_id = nodes.iter().find(|n| n["title"] == "final").unwrap()["nodeId"]
        .as_str()
        .unwrap()
        .to_string();
    let edge_to = edges[0]["to"].as_str().unwrap();
    assert_eq!(edge_to, final_node_id);

    let user_refs = git(&repo, &["for-each-ref", "--format=%(refname)"]);
    assert!(!user_refs.contains("refs/heads/eunomio-test"));
    assert!(!user_refs.contains("refs/eunomio/"));
    let user_worktrees = git(&repo, &["worktree", "list", "--porcelain"]);
    assert!(!user_worktrees.contains(&app.data.path().display().to_string()));

    let (status, body) = app.auth_empty("GET", "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);

    let (status, _) = app
        .auth_empty("DELETE", &format!("/api/sessions/{session_id}"))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = app
        .auth_empty("GET", &format!("/api/sessions/{session_id}/graph"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

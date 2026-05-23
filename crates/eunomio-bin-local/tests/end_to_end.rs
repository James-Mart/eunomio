// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;

mod common;
use common::{
    app::TestApp,
    git::{git, local_session_body},
};

#[tokio::test]
async fn happy_path_create_rename_branch() {
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

    let final_node_id = nodes
        .iter()
        .find(|n| n["title"] == "final")
        .unwrap()["nodeId"]
        .as_str()
        .unwrap()
        .to_string();
    let edge_to = edges[0]["to"].as_str().unwrap();
    assert_eq!(edge_to, final_node_id);

    let (status, body) = app
        .auth_json(
            "PATCH",
            &format!("/api/sessions/{session_id}/nodes/{final_node_id}"),
            json!({ "title": "final renamed" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"].as_str().unwrap(), "final renamed");

    let (status, body) = app
        .auth_json(
            "POST",
            &format!("/api/sessions/{session_id}/nodes/{final_node_id}/branch"),
            json!({ "branchName": "eunomio-test" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "branch body: {body}");

    let log = git(
        &repo,
        &["log", "--format=%H %T %s", "eunomio-test"],
    );
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 commits, got: {log}");

    let tip_tree = lines[0].split(' ').nth(1).unwrap();
    let feature_tree = git(&repo, &["rev-parse", "feature^{tree}"]);
    assert_eq!(tip_tree, feature_tree, "tip tree must equal feature^{{tree}}");

    let tip_subject = lines[0].splitn(3, ' ').nth(2).unwrap();
    assert_eq!(tip_subject, "final renamed");
    let root_subject = lines[1].splitn(3, ' ').nth(2).unwrap();
    assert_eq!(root_subject, "base");

    let (status, body) = app
        .auth_json(
            "POST",
            &format!("/api/sessions/{session_id}/nodes/{final_node_id}/branch"),
            json!({ "branchName": "eunomio-test" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "expected conflict body: {body}");

    let (status, _body) = app
        .auth_json(
            "POST",
            &format!("/api/sessions/{session_id}/nodes/{final_node_id}/branch"),
            json!({ "branchName": "eunomio-test", "force": true }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

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

    let branch_log = git(&repo, &["log", "--format=%H", "eunomio-test"]);
    assert!(!branch_log.is_empty(), "user branch must survive deletion");
}

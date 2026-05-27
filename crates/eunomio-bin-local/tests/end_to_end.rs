// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use eunomio_core::{unix_seconds, NewShavingTrackInsert, NodeRewrite, ShavingStep};
use pretty_assertions::assert_eq;

mod common;
use common::{
    app::TestApp,
    git::{git, local_session_body, write},
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

#[tokio::test]
async fn shaving_track_diffs_include_canonical_synthesized_ranges() {
    let app = TestApp::spawn_authenticated_with_repo(synthetic_branch_repo).await;
    let repo = app.repo_path();

    let synthetic_commit = git(&repo, &["rev-parse", "synthetic"]);
    let synthetic_tree = git(&repo, &["rev-parse", "synthetic^{tree}"]);

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

    let (status, graph) = app
        .auth_empty("GET", &format!("/api/sessions/{session_id}/graph"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let target_node_id = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| !n["parentNodeId"].is_null())
        .unwrap()["nodeId"]
        .as_str()
        .unwrap()
        .to_string();
    let base_tree = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["nodeId"] == base_node_id)
        .unwrap()["treeSha"]
        .as_str()
        .unwrap()
        .to_string();

    app.state
        .datastore
        .nodes()
        .rewrite_chain(
            "local",
            &session_id,
            vec![NodeRewrite {
                node_id: target_node_id.clone(),
                parent_node_id: base_node_id,
                tree_sha: synthetic_tree.clone(),
                commit_sha: synthetic_commit.clone(),
            }],
        )
        .await
        .unwrap();
    app.state
        .datastore
        .shaving_tracks()
        .insert(NewShavingTrackInsert {
            org_id: "local".to_string(),
            session_id: session_id.clone(),
            target_node_id: target_node_id.clone(),
            parent_tree_sha: base_tree,
            head_tree_sha: synthetic_tree.clone(),
            steps: vec![ShavingStep {
                tree_sha: synthetic_tree,
                commit_sha: synthetic_commit,
                label: Some("synthetic step".to_string()),
            }],
            ref_name: format!("refs/eunomio/shavings/{target_node_id}"),
            created_at: unix_seconds(),
        })
        .await
        .unwrap();

    let (status, track) = app
        .auth_empty(
            "GET",
            &format!("/api/sessions/{session_id}/nodes/{target_node_id}/shaving-track"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "track body: {track}");
    let step_diffs = track["stepDiffs"].as_array().unwrap();
    assert_eq!(step_diffs.len(), 2, "one step plus final full diff");

    for diff in step_diffs {
        let child = diff["synthesized"]["child"].as_array().unwrap();
        assert_eq!(child[0]["path"].as_str(), Some("topic.txt"));
        assert!(
            !child[0]["lines"].as_array().unwrap().is_empty(),
            "shaving timeline diff should carry synthesized spans: {diff}"
        );
    }
}

fn synthetic_branch_repo(path: &std::path::Path) {
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "test@example.com"]);
    git(path, &["config", "user.name", "Test"]);
    write(path, "topic.txt", "base\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "base commit"]);

    git(path, &["checkout", "-q", "-b", "synthetic"]);
    write(path, "topic.txt", "synthetic\n");
    git(path, &["commit", "-q", "-am", "synthetic commit"]);

    git(path, &["checkout", "-q", "main"]);
    git(path, &["checkout", "-q", "-b", "feature"]);
    write(path, "topic.txt", "final\n");
    git(path, &["commit", "-q", "-am", "feature commit"]);

    git(path, &["checkout", "-q", "main"]);
}

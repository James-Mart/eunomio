use axum::http::StatusCode;
use eunomia::cursor_bridge::{FakeSubagentRunner, HelperEvent};
use eunomia::types::{PhaseName, PhaseState, SseEvent};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

mod common;
use common::{empty_request, json_request, TestApp};

const SURVEY_RESULT: &str = "```json\n{\"summary\":\"s\",\"themes\":[{\"id\":\"t-1\",\"title\":\"T\",\"description\":\"D\"}]}\n```";

fn plan_result(strategy: &str) -> String {
    format!(
        "```json\n{{\"strategy\":\"{strategy}\",\"strategyRationale\":\"r\",\"edges\":[{{\"id\":\"slice\",\"title\":\"Add b.txt\",\"description\":\"Adds b.\"}},{{\"id\":\"leftover\",\"title\":\"leftover\",\"description\":\"Nothing more.\"}}]}}\n```"
    )
}

fn survey_script() -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: 0,
        result: SURVEY_RESULT.to_string(),
        duration_ms: None,
    }]
}

fn plan_script(strategy: &str) -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: 0,
        result: plan_result(strategy),
        duration_ms: None,
    }]
}

fn construct_ok_script() -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: 0,
        result: "OK\n".into(),
        duration_ms: None,
    }]
}

fn construct_blocked_script(reason: &str) -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: 0,
        result: format!("BLOCKED: {reason}\n"),
        duration_ms: None,
    }]
}

async fn create_session_and_pick_target(app: &TestApp) -> (String, String) {
    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "feature" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create body: {body}");
    let session_id = body["id"].as_str().unwrap().to_string();
    let (_, graph) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/graph"),
    )
    .await;
    let target = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| !n["parentNodeId"].is_null())
        .unwrap()["nodeId"]
        .as_str()
        .unwrap()
        .to_string();
    (session_id, target)
}

async fn next_event(
    rx: &mut tokio::sync::broadcast::Receiver<SseEvent>,
) -> SseEvent {
    timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for SSE event")
        .expect("SSE channel closed")
}

async fn wait_for_phase(
    rx: &mut tokio::sync::broadcast::Receiver<SseEvent>,
    expected_name: PhaseName,
    expected_state: PhaseState,
) -> SseEvent {
    loop {
        let ev = next_event(rx).await;
        if let SseEvent::Phase { name, state, .. } = &ev {
            if *name == expected_name && *state == expected_state {
                return ev;
            }
        }
    }
}

#[tokio::test]
async fn happy_path_drives_partition_to_accept() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("semantic"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "begin body: {body}");
    let partition_id = body["id"].as_i64().unwrap();
    let worktree_root = app
        .data
        .path()
        .canonicalize()
        .unwrap()
        .join("worktrees")
        .join(&session_id)
        .join(partition_id.to_string())
        .join("synthesis");
    assert!(worktree_root.exists(), "worktree should exist after begin");

    let started = next_event(&mut rx).await;
    assert!(matches!(started, SseEvent::Started { .. }));

    let survey_review = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &survey_review {
        assert!(payload.is_some(), "survey awaiting_review must carry payload");
    }

    let (_, runs) = empty_request(
        &app.router,
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = runs.as_array().unwrap()[0]["id"].as_i64().unwrap();

    let (status, _) = json_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;

    let (_, runs) = empty_request(
        &app.router,
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = runs.as_array().unwrap()[0]["id"].as_i64().unwrap();

    let (status, _) = json_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/plan/accept"),
        json!({ "runId": plan_run_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let construct_review =
        wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &construct_review {
        let p = payload.as_ref().unwrap();
        assert_eq!(p["outcome"].as_str().unwrap(), "ok");
        assert!(p["candidateTreeSha"].is_string());
        assert!(p["candidateCommitSha"].is_string());
    } else {
        panic!("expected phase event");
    }

    let (_, partition_body) = empty_request(
        &app.router,
        "GET",
        &format!("/api/partitions/{partition_id}"),
    )
    .await;
    assert_eq!(
        partition_body["candidateSliceTreeSha"].as_str().unwrap().len(),
        40,
        "candidate tree should be populated"
    );

    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/construct/accept"),
    )
    .await;
    assert!(
        status == StatusCode::NO_CONTENT,
        "accept_construct expected 204, got {status}: {body}",
    );

    loop {
        let ev = next_event(&mut rx).await;
        if matches!(ev, SseEvent::Finished { .. }) {
            break;
        }
    }

    let (_, graph) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/graph"),
    )
    .await;
    let nodes = graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3, "should have base + slice + final");
    let target_node = nodes
        .iter()
        .find(|n| n["nodeId"].as_str().unwrap() == target_node_id)
        .unwrap();
    assert_eq!(target_node["title"].as_str().unwrap(), "leftover");
    let slice_node = nodes
        .iter()
        .find(|n| n["nodeId"].as_str().unwrap() != target_node_id && !n["parentNodeId"].is_null())
        .unwrap();
    assert_eq!(slice_node["title"].as_str().unwrap(), "Add b.txt");
    assert_eq!(
        target_node["parentNodeId"].as_str().unwrap(),
        slice_node["nodeId"].as_str().unwrap()
    );

    assert!(!worktree_root.exists(), "worktree should be removed after accept");

    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "session lock should be released: {body}");
    let new_partition_id = body["id"].as_i64().unwrap();
    let (_, _) = empty_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{new_partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn second_begin_returns_partition_in_flight() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![survey_script()]));
    let app = TestApp::spawn_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "begin body: {body}");
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"].as_str().unwrap(), "partition_in_flight");
}

#[tokio::test]
async fn constructor_blocked_parks_at_review_and_can_re_run() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("semantic"),
        construct_blocked_script("can't slice without leftover hunks"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_i64().unwrap();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = empty_request(
        &app.router,
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = runs.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let _ = json_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, runs) = empty_request(
        &app.router,
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = runs.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let _ = json_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/plan/accept"),
        json!({ "runId": plan_run_id }),
    )
    .await;

    let blocked = wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &blocked {
        let p = payload.as_ref().unwrap();
        assert_eq!(p["outcome"].as_str().unwrap(), "blocked");
        assert!(p["reason"].as_str().unwrap().contains("can't slice"));
    }

    let (status, body) = json_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "construct", "userFeedback": "try harder" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "re-run body: {body}");

    let ok = wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &ok {
        assert_eq!(payload.as_ref().unwrap()["outcome"].as_str().unwrap(), "ok");
    }
}

#[tokio::test]
async fn abandon_mid_run_cleans_up() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![vec![
        HelperEvent::SdkMessage {
            run_id: 0,
            message: json!({"text": "thinking"}),
        },
    ]]));
    let app = TestApp::spawn_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_i64().unwrap();
    let worktree_root = app
        .data
        .path()
        .canonicalize()
        .unwrap()
        .join("worktrees")
        .join(&session_id)
        .join(partition_id.to_string())
        .join("synthesis");
    assert!(worktree_root.exists());

    let _ = next_event(&mut rx).await;

    let (status, _) = empty_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!worktree_root.exists());

    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "lock should be free: {body}");
    let new_partition_id = body["id"].as_i64().unwrap();
    let _ = empty_request(
        &app.router,
        "POST",
        &format!("/api/partitions/{new_partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn hitl_off_drives_full_chain_without_explicit_accepts() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("semantic"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let (status, _) = json_request(
        &app.router,
        "PATCH",
        "/api/partition-settings",
        json!({
            "coordinator": {
                "model": "composer-2",
                "humanInTheLoop": {
                    "afterSurvey": false,
                    "afterPlanning": false,
                    "afterConstruct": false
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (status, body) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "begin body: {body}");

    let mut got_finished = false;
    for _ in 0..30 {
        let ev = next_event(&mut rx).await;
        if matches!(ev, SseEvent::Finished { .. }) {
            got_finished = true;
            break;
        }
        if let SseEvent::Phase {
            state: PhaseState::AwaitingReview,
            ..
        } = &ev
        {
            panic!("HITL-off path should never park at awaiting_review, got {ev:?}");
        }
    }
    assert!(got_finished, "did not see Finished event");

    let (_, graph) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/graph"),
    )
    .await;
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 3);
}

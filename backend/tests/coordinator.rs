use async_trait::async_trait;
use axum::http::StatusCode;
use eunomio::cursor_bridge::{
    FakeSubagentRunner, HelperEvent, RunHandle, RunRequest, SubagentRunner,
};
use eunomio::error::AppError;
use eunomio::types::{PhaseName, PhaseState, SseEvent};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::timeout;

mod common;
use common::app::TestApp;

const SURVEY_RESULT: &str = "```json\n{\"summary\":\"s\",\"themes\":[{\"id\":\"t-1\",\"title\":\"T\",\"description\":\"D\"}]}\n```";

fn plan_result(strategy: &str) -> String {
    format!(
        "```json\n{{\"outcome\":\"split\",\"strategy\":\"{strategy}\",\"strategyRationale\":\"r\",\"edges\":[{{\"id\":\"slice\",\"title\":\"Add b.txt\",\"description\":\"Adds b.\"}},{{\"id\":\"leftover\",\"title\":\"leftover\",\"description\":\"Nothing more.\"}}]}}\n```"
    )
}

fn plan_indivisible_result(rationale: &str) -> String {
    format!("```json\n{{\"outcome\":\"indivisible\",\"rationale\":\"{rationale}\"}}\n```")
}

fn finished_run_id(runs: &serde_json::Value, kind: &str) -> String {
    runs.as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == kind && r["status"] == "finished")
        .unwrap_or_else(|| panic!("no finished {kind} run in {runs}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn survey_script() -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: "0".to_string(),
        result: SURVEY_RESULT.to_string(),
        duration_ms: None,
    }]
}

fn plan_script(strategy: &str) -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: "0".to_string(),
        result: plan_result(strategy),
        duration_ms: None,
    }]
}

fn construct_ok_script() -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: "0".to_string(),
        result: "OK\n".into(),
        duration_ms: None,
    }]
}

fn construct_blocked_script(reason: &str) -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: "0".to_string(),
        result: format!("BLOCKED: {reason}\n"),
        duration_ms: None,
    }]
}

async fn create_session_and_pick_target(app: &TestApp) -> (String, String) {
    let (status, body) = app.auth_json(
        "POST",
        "/api/sessions",
        common::git::local_session_body(&app.repo_path(), "main", "feature"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create body: {body}");
    let session_id = body["id"].as_str().unwrap().to_string();
    let (_, graph) = app.auth_empty(
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
        plan_script("synthetic"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (status, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "begin body: {body}");
    let partition_id = body["id"].as_str().unwrap().to_string();
    let worktree_root = app
        .data
        .path()
        .canonicalize()
        .unwrap()
        .join("worktrees")
        .join(&session_id)
        .join(&partition_id)
        .join("worktree");
    assert!(worktree_root.exists(), "worktree should exist after begin");

    let started = next_event(&mut rx).await;
    assert!(matches!(started, SseEvent::Started { .. }));

    let survey_review = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &survey_review {
        assert!(payload.is_some(), "survey awaiting_review must carry payload");
    }

    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");

    let (status, _) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;

    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = finished_run_id(&runs, "plan");

    let (status, _) = app.auth_json(
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

    let (_, partition_body) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}"),
    )
    .await;
    assert_eq!(
        partition_body["candidateSliceTreeSha"].as_str().unwrap().len(),
        40,
        "candidate tree should be populated"
    );

    let (status, body) = app.auth_empty(
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

    let (_, graph) = app.auth_empty(
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

    let (status, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "session lock should be released: {body}");
    let new_partition_id = body["id"].as_str().unwrap().to_string();
    let (_, _) = app.auth_empty(
        "POST",
        &format!("/api/partitions/{new_partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn parallel_begins_on_same_target_succeed() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        survey_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (status1, body1) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED, "first begin body: {body1}");

    let (status2, body2) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status2, StatusCode::CREATED, "second begin body: {body2}");
    let pid1 = body1["id"].as_str().unwrap().to_string();
    let pid2 = body2["id"].as_str().unwrap().to_string();
    assert_ne!(pid1, pid2);

    let mut seen_review_a = false;
    let mut seen_review_b = false;
    for _ in 0..30 {
        let ev = next_event(&mut rx).await;
        if let SseEvent::Phase {
            name: PhaseName::Survey,
            state: PhaseState::AwaitingReview,
            partition_id,
            ..
        } = ev
        {
            if partition_id == pid1 {
                seen_review_a = true;
            }
            if partition_id == pid2 {
                seen_review_b = true;
            }
            if seen_review_a && seen_review_b {
                break;
            }
        }
    }
    assert!(seen_review_a && seen_review_b);
}

#[tokio::test]
async fn constructor_blocked_parks_at_review_and_can_re_run() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("synthetic"),
        construct_blocked_script("can't slice without leftover hunks"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = finished_run_id(&runs, "plan");
    let _ = app.auth_json(
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

    let (status, body) = app.auth_json(
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
            run_id: "0".to_string(),
            message: json!({"text": "thinking"}),
        },
    ]]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let worktree_root = app
        .data
        .path()
        .canonicalize()
        .unwrap()
        .join("worktrees")
        .join(&session_id)
        .join(&partition_id)
        .join("worktree");
    assert!(worktree_root.exists());

    let _ = next_event(&mut rx).await;

    let (status, _) = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!worktree_root.exists());

    let (status, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "lock should be free: {body}");
    let new_partition_id = body["id"].as_str().unwrap().to_string();
    let _ = app.auth_empty(
        "POST",
        &format!("/api/partitions/{new_partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn hitl_off_drives_full_chain_without_explicit_accepts() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("synthetic"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let (status, _) = app.auth_json(
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
    let (status, body) = app.auth_empty(
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

    let (_, graph) = app.auth_empty(
        "GET",
        &format!("/api/sessions/{session_id}/graph"),
    )
    .await;
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 3);
}

async fn set_surveyor_disabled_hitl_off(app: &TestApp) {
    let (status, _) = app.auth_json(
        "PATCH",
        "/api/partition-settings",
        json!({
            "coordinator": {
                "model": "composer-2",
                "surveyorEnabled": false,
                "humanInTheLoop": {
                    "afterSurvey": false,
                    "afterPlanning": false,
                    "afterConstruct": false,
                    "afterIndivisible": false,
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn surveyor_disabled_skips_to_plan() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        plan_script("synthetic"),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    set_surveyor_disabled_hitl_off(&app).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;

    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (status, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "begin body: {body}");
    let partition_id = body["id"].as_str().unwrap().to_string();

    let started = next_event(&mut rx).await;
    assert!(matches!(started, SseEvent::Started { .. }));

    let first_phase = loop {
        let ev = next_event(&mut rx).await;
        if let SseEvent::Phase { name, state, .. } = &ev {
            break (*name, *state);
        }
    };
    assert_eq!(first_phase.0, PhaseName::Plan);
    assert_eq!(first_phase.1, PhaseState::Running);

    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    assert!(
        !runs
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "survey"),
        "expected no survey run, got {runs}"
    );

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
}

fn plan_indivisible_script(rationale: &str) -> Vec<HelperEvent> {
    vec![HelperEvent::Finished {
        run_id: "0".to_string(),
        result: plan_indivisible_result(rationale),
        duration_ms: None,
    }]
}

async fn set_hitl_all_off(app: &TestApp) {
    let (status, _) = app.auth_json(
        "PATCH",
        "/api/partition-settings",
        json!({
            "coordinator": {
                "model": "composer-2",
                "humanInTheLoop": {
                    "afterSurvey": false,
                    "afterPlanning": false,
                    "afterConstruct": false,
                    "afterIndivisible": false,
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

async fn poll_partition_phase(
    app: &TestApp,
    partition_id: &str,
    expected_phase: &str,
    expected_state: &str,
) {
    for _ in 0..200 {
        let (_, body) = app.auth_empty(
            "GET",
            &format!("/api/partitions/{partition_id}"),
        )
        .await;
        if body["phase"].as_str() == Some(expected_phase)
            && body["phaseState"].as_str() == Some(expected_state)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!(
        "partition {partition_id} did not reach ({expected_phase}, {expected_state}) within timeout"
    );
}

async fn drive_partition_to_construct_review(
    app: &TestApp,
    partition_id: &str,
) {
    poll_partition_phase(app, partition_id, "survey", "awaiting_review").await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = runs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "survey" && r["status"] == "finished")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    poll_partition_phase(app, partition_id, "plan", "awaiting_review").await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = runs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "plan" && r["status"] == "finished")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/plan/accept"),
        json!({ "runId": plan_run_id }),
    )
    .await;
    poll_partition_phase(app, partition_id, "construct", "awaiting_review").await;
}

#[tokio::test]
async fn sibling_accept_auto_abandons_other() {
    let runner = Arc::new(KindAwareRunner::new(
        vec![survey_script(), survey_script()],
        vec![plan_script("synthetic"), plan_script("vertical")],
        vec![construct_ok_script(), construct_ok_script()],
    ));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body_a) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let pid_a = body_a["id"].as_str().unwrap().to_string();
    let (_, body_b) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let pid_b = body_b["id"].as_str().unwrap().to_string();
    assert_ne!(pid_a, pid_b);

    drive_partition_to_construct_review(&app, &pid_a).await;
    drive_partition_to_construct_review(&app, &pid_b).await;

    let (status, body) = app.auth_empty(
        "POST",
        &format!("/api/partitions/{pid_a}/construct/accept"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let mut saw_finished_a = false;
    let mut saw_cancelled_b = false;
    for _ in 0..40 {
        let ev = next_event(&mut rx).await;
        match &ev {
            SseEvent::Finished { partition_id, .. } if *partition_id == pid_a => {
                saw_finished_a = true;
            }
            SseEvent::Cancelled { partition_id, .. } if *partition_id == pid_b => {
                saw_cancelled_b = true;
            }
            _ => {}
        }
        if saw_finished_a && saw_cancelled_b {
            break;
        }
    }
    assert!(saw_finished_a, "no Finished for accepted partition");
    assert!(saw_cancelled_b, "no Cancelled for sibling partition");

    let (status_a, _) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{pid_a}"),
    )
    .await;
    assert_eq!(status_a, StatusCode::NOT_FOUND);
    let (status_b, _) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{pid_b}"),
    )
    .await;
    assert_eq!(status_b, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_run_mid_construct_preserves_partition() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("synthetic"),
        vec![HelperEvent::SdkMessage {
            run_id: "0".to_string(),
            message: json!({"text": "thinking"}),
        }],
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();

    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = finished_run_id(&runs, "plan");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/plan/accept"),
        json!({ "runId": plan_run_id }),
    )
    .await;

    let _ = wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::Running).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let running_run = runs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "construct" && r["status"] == "running")
        .expect("expected running construct run");
    let construct_run_id = running_run["id"].as_str().unwrap().to_string();

    let (status, _) = app.auth_empty(
        "DELETE",
        &format!("/api/partitions/{partition_id}/runs/{construct_run_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, partition_body) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}"),
    )
    .await;
    assert_eq!(partition_body["phaseState"].as_str().unwrap(), "error");
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let cancelled = runs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str().unwrap() == construct_run_id)
        .unwrap();
    assert_eq!(cancelled["status"].as_str().unwrap(), "cancelled");

    let (status, _) = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn cancel_run_on_non_running_returns_409() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![survey_script()]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let finished_run = runs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == "finished")
        .unwrap();
    let run_id = finished_run["id"].as_str().unwrap().to_string();
    let (status, body) = app.auth_empty(
        "DELETE",
        &format!("/api/partitions/{partition_id}/runs/{run_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"].as_str().unwrap(), "run_not_running");
}

#[tokio::test]
async fn second_start_run_while_in_flight_returns_409() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        vec![HelperEvent::SdkMessage {
            run_id: "0".to_string(),
            message: json!({"text": "thinking"}),
        }],
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "survey" }),
    )
    .await;

    let (status, body) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "survey" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"].as_str().unwrap(), "partition_run_in_flight");

    let _ = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn invalid_run_kind_returns_409() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("synthetic"),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;

    let (status, body) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "construct" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"].as_str().unwrap(), "invalid_run_kind");
}

#[tokio::test]
async fn back_edge_construct_to_plan_on_blocked_run() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("synthetic"),
        construct_blocked_script("can't slice"),
        plan_script("vertical"),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = finished_run_id(&runs, "plan");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/plan/accept"),
        json!({ "runId": plan_run_id }),
    )
    .await;

    let blocked = wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &blocked {
        assert_eq!(payload.as_ref().unwrap()["outcome"].as_str().unwrap(), "blocked");
    }

    let (status, body) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "plan" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, partition_body) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}"),
    )
    .await;
    assert_eq!(partition_body["phase"].as_str().unwrap(), "plan");
    assert!(partition_body["candidateSliceTreeSha"].is_null());
}

#[tokio::test]
async fn back_edge_construct_to_plan_on_ok_candidate() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("synthetic"),
        construct_ok_script(),
        plan_script("vertical"),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    drive_partition_to_construct_review(&app, &partition_id).await;

    let (status, body) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "plan" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, partition_body) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}"),
    )
    .await;
    assert!(partition_body["candidateSliceTreeSha"].is_null());
    assert!(partition_body["strategy"].is_null());
}

#[tokio::test]
async fn indivisible_with_hitl_on_parks_at_review() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_indivisible_script("one tight refactor"),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;

    let plan_review = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &plan_review {
        let p = payload.as_ref().unwrap();
        assert_eq!(p["outcome"].as_str().unwrap(), "indivisible");
        assert!(p["rationale"].as_str().unwrap().contains("tight"));
    }

    let _ = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn indivisible_with_hitl_off_auto_abandons() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_indivisible_script("indivisible"),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    set_hitl_all_off(&app).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();

    let mut got_cancelled = false;
    for _ in 0..40 {
        let ev = next_event(&mut rx).await;
        if let SseEvent::Cancelled { partition_id: p, .. } = &ev {
            if *p == partition_id {
                got_cancelled = true;
                break;
            }
        }
        if let SseEvent::Phase {
            name: PhaseName::Plan,
            state: PhaseState::AwaitingReview,
            ..
        } = &ev
        {
            panic!("HITL-off indivisible should auto-Abandon, not park");
        }
    }
    assert!(got_cancelled, "expected Cancelled event");

    for _ in 0..20 {
        let (status, _) = app.auth_empty(
            "GET",
            &format!("/api/partitions/{partition_id}"),
        )
        .await;
        if status == StatusCode::NOT_FOUND {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("partition row was not deleted after auto-Abandon");
}

#[tokio::test]
async fn rerun_planner_from_indivisible_succeeds() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_indivisible_script("indivisible at first"),
        plan_script("synthetic"),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;

    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;

    let (status, _) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "plan", "userFeedback": "split anyway" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let plan_review = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    if let SseEvent::Phase { payload, .. } = &plan_review {
        assert_eq!(
            payload.as_ref().unwrap()["outcome"].as_str().unwrap(),
            "split"
        );
    }
}

struct KindAwareRunner {
    surveys: TokioMutex<Vec<Vec<HelperEvent>>>,
    plans: TokioMutex<Vec<Vec<HelperEvent>>>,
    constructs: TokioMutex<Vec<Vec<HelperEvent>>>,
}

impl KindAwareRunner {
    fn new(
        surveys: Vec<Vec<HelperEvent>>,
        plans: Vec<Vec<HelperEvent>>,
        constructs: Vec<Vec<HelperEvent>>,
    ) -> Self {
        Self {
            surveys: TokioMutex::new(surveys),
            plans: TokioMutex::new(plans),
            constructs: TokioMutex::new(constructs),
        }
    }
}

#[async_trait]
impl SubagentRunner for KindAwareRunner {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError> {
        let bucket: &TokioMutex<Vec<Vec<HelperEvent>>> = if request.prompt.contains("**Surveyor**") {
            &self.surveys
        } else if request.prompt.contains("**Planner**") {
            &self.plans
        } else if request.prompt.contains("**Constructor**") {
            &self.constructs
        } else {
            panic!("KindAwareRunner could not classify prompt");
        };
        let run_id = request.run_id.clone();
        let mut q = bucket.lock().await;
        let events = if q.is_empty() {
            vec![HelperEvent::Finished {
                run_id: run_id.clone(),
                result: String::new(),
                duration_ms: None,
            }]
        } else {
            q.remove(0)
        };
        drop(q);
        tokio::spawn(async move {
            for ev in events {
                let rebound = match ev {
                    HelperEvent::Started { agent_id, .. } => HelperEvent::Started {
                        run_id: run_id.clone(),
                        agent_id,
                    },
                    HelperEvent::SdkMessage { message, .. } => HelperEvent::SdkMessage {
                        run_id: run_id.clone(),
                        message,
                    },
                    HelperEvent::Finished {
                        result,
                        duration_ms,
                        ..
                    } => HelperEvent::Finished {
                        run_id: run_id.clone(),
                        result,
                        duration_ms,
                    },
                    HelperEvent::Error { code, message, .. } => HelperEvent::Error {
                        run_id: run_id.clone(),
                        code,
                        message,
                    },
                    HelperEvent::Cancelled { .. } => HelperEvent::Cancelled {
                        run_id: run_id.clone(),
                    },
                };
                if tx.send(rebound).await.is_err() {
                    break;
                }
            }
        });
        let cancel: Box<dyn Fn() + Send + Sync> = Box::new(|| {});
        Ok(RunHandle { cancel })
    }
}

#[tokio::test]
async fn fanout_count_2_spawns_children_no_grandchildren() {
    let runner = Arc::new(KindAwareRunner::new(
        vec![survey_script(), survey_script(), survey_script()],
        vec![
            plan_script("synthetic"),
            plan_script("synthetic"),
            plan_script("synthetic"),
        ],
        vec![
            construct_ok_script(),
            construct_ok_script(),
            construct_ok_script(),
        ],
    ));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (status, _) = app.auth_json(
        "PATCH",
        "/api/partition-settings",
        json!({
            "coordinator": {
                "model": "composer-2",
                "humanInTheLoop": {
                    "afterSurvey": false,
                    "afterPlanning": false,
                    "afterConstruct": false,
                    "afterIndivisible": false,
                },
                "maxIterations": { "kind": "count", "count": 2 }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let root_pid = body["id"].as_str().unwrap().to_string();

    let mut started_pids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut finished_pids: std::collections::HashSet<String> = std::collections::HashSet::new();
    started_pids.insert(root_pid);

    for _ in 0..200 {
        let ev = next_event(&mut rx).await;
        match &ev {
            SseEvent::Started { partition_id, .. } => {
                started_pids.insert(partition_id.clone());
            }
            SseEvent::Finished { partition_id, .. } => {
                finished_pids.insert(partition_id.clone());
            }
            _ => {}
        }
        if finished_pids.len() == 3 {
            break;
        }
    }
    assert_eq!(finished_pids.len(), 3, "expected exactly root + 2 children to Finish");
    assert_eq!(started_pids.len(), 3, "expected only root + 2 Started, got {:?}", started_pids);
}

#[tokio::test]
async fn fanout_auto_cascading_indivisible() {
    let runner = Arc::new(KindAwareRunner::new(
        vec![survey_script(), survey_script(), survey_script()],
        vec![
            plan_script("synthetic"),
            plan_indivisible_script("indivisible-a"),
            plan_indivisible_script("indivisible-b"),
        ],
        vec![construct_ok_script()],
    ));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (status, _) = app.auth_json(
        "PATCH",
        "/api/partition-settings",
        json!({
            "coordinator": {
                "model": "composer-2",
                "humanInTheLoop": {
                    "afterSurvey": false,
                    "afterPlanning": false,
                    "afterConstruct": false,
                    "afterIndivisible": false,
                },
                "maxIterations": { "kind": "auto" }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let root_pid = body["id"].as_str().unwrap().to_string();

    let mut started: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut finished: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cancelled: std::collections::HashSet<String> = std::collections::HashSet::new();
    started.insert(root_pid);

    for _ in 0..200 {
        let ev = next_event(&mut rx).await;
        match &ev {
            SseEvent::Started { partition_id, .. } => {
                started.insert(partition_id.clone());
            }
            SseEvent::Finished { partition_id, .. } => {
                finished.insert(partition_id.clone());
            }
            SseEvent::Cancelled { partition_id, .. } => {
                cancelled.insert(partition_id.clone());
            }
            _ => {}
        }
        if finished.len() == 1 && cancelled.len() == 2 {
            break;
        }
    }
    assert_eq!(finished.len(), 1, "expected root to Finish");
    assert_eq!(cancelled.len(), 2, "expected 2 children to be auto-Abandoned");
    assert_eq!(started.len(), 3, "expected root + 2 children Started");
}

async fn enable_transcripts(app: &TestApp) {
    let (status, _) = app.auth_json(
        "PATCH",
        "/api/partition-settings",
        json!({ "general": { "transcriptsEnabled": true } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

async fn count_runs_with_transcript(app: &TestApp) -> i64 {
    app.state
        .db
        .call(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM runs WHERE transcript_text IS NOT NULL AND transcript_text != ''",
                tokio_rusqlite::params![],
                |r| r.get(0),
            )?;
            Ok(n)
        })
        .await
        .unwrap()
}

fn survey_with_messages(messages: Vec<serde_json::Value>) -> Vec<HelperEvent> {
    let mut events: Vec<HelperEvent> = messages
        .into_iter()
        .map(|m| HelperEvent::SdkMessage {
            run_id: "0".to_string(),
            message: m,
        })
        .collect();
    events.push(HelperEvent::Finished {
        run_id: "0".to_string(),
        result: SURVEY_RESULT.to_string(),
        duration_ms: None,
    });
    events
}

#[tokio::test]
async fn transcripts_enabled_persists_prompt_and_transcript_text() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![survey_with_messages(vec![
        json!({"type": "assistant", "text": "thinking 1"}),
        json!({"type": "tool_use", "name": "ls"}),
    ])]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    enable_transcripts(&app).await;

    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let run_id = finished_run_id(&runs, "survey");

    let (status, transcript) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs/{run_id}/transcript"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "transcript body: {transcript}");
    let prompt = transcript["prompt"]
        .as_str()
        .expect("prompt must be populated when transcripts are on");
    assert!(!prompt.is_empty());
    let text = transcript["transcriptText"]
        .as_str()
        .expect("transcriptText must be populated");
    assert!(text.contains("thinking 1"), "text: {text}");
    assert!(text.contains("[tool: ls]"), "text: {text}");

    let _ = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn transcripts_disabled_still_persists_transcript_text_without_sse() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![survey_with_messages(vec![
        json!({"type": "assistant", "text": "thinking"}),
    ])]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;

    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();

    let mut saw_delta = false;
    loop {
        let ev = next_event(&mut rx).await;
        if let SseEvent::TranscriptDelta { .. } = &ev {
            saw_delta = true;
        }
        if matches!(
            ev,
            SseEvent::Phase {
                name: PhaseName::Survey,
                state: PhaseState::AwaitingReview,
                ..
            }
        ) {
            break;
        }
    }
    assert!(
        !saw_delta,
        "transcriptDelta must not be emitted when transcripts are disabled"
    );

    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let run_id = finished_run_id(&runs, "survey");

    let (status, transcript) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs/{run_id}/transcript"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let prompt = transcript["prompt"].as_str().unwrap();
    assert!(!prompt.is_empty());
    let text = transcript["transcriptText"]
        .as_str()
        .expect("transcriptText should be captured even when toggle is off");
    assert!(text.contains("thinking"), "text: {text}");

    let _ = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
}

#[tokio::test]
async fn transcripts_cleaned_up_on_accept_construct() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_with_messages(vec![json!({"text": "s"})]),
        vec![
            HelperEvent::SdkMessage {
                run_id: "0".to_string(),
                message: json!({"text": "p"}),
            },
            HelperEvent::Finished {
                run_id: "0".to_string(),
                result: plan_result("synthetic"),
                duration_ms: None,
            },
        ],
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    enable_transcripts(&app).await;

    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    drive_partition_to_construct_review(&app, &partition_id).await;

    assert!(
        count_runs_with_transcript(&app).await > 0,
        "expected captured transcript before accept"
    );

    let (status, body) = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/construct/accept"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    loop {
        let ev = next_event(&mut rx).await;
        if matches!(ev, SseEvent::Finished { .. }) {
            break;
        }
    }

    assert_eq!(
        count_runs_with_transcript(&app).await,
        0,
        "runs with transcript must be gone after accept_construct"
    );
}

#[tokio::test]
async fn transcripts_cleaned_up_on_abandon() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![survey_with_messages(vec![
        json!({"text": "s"}),
    ])]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    enable_transcripts(&app).await;

    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    assert!(
        count_runs_with_transcript(&app).await > 0,
        "expected captured transcript before abandon"
    );

    let (status, _) = app.auth_empty(
        "POST",
        &format!("/api/partitions/{partition_id}/abandon"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(
        count_runs_with_transcript(&app).await,
        0,
        "runs with transcript must be gone after abandon"
    );
}

fn partition_worktree_path(app: &TestApp, session_id: &str, partition_id: &str) -> std::path::PathBuf {
    app.data
        .path()
        .canonicalize()
        .unwrap()
        .join("worktrees")
        .join(session_id)
        .join(partition_id)
        .join("worktree")
}

#[tokio::test]
async fn construct_spawn_resets_dirty_worktree() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        plan_script("vertical"),
        construct_ok_script(),
        construct_ok_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner.clone()).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);

    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let worktree_root = partition_worktree_path(&app, &session_id, &partition_id);

    let _ = next_event(&mut rx).await;
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let survey_run_id = finished_run_id(&runs, "survey");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/survey/accept"),
        json!({ "runId": survey_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Plan, PhaseState::AwaitingReview).await;
    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let plan_run_id = finished_run_id(&runs, "plan");
    let _ = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/plan/accept"),
        json!({ "runId": plan_run_id }),
    )
    .await;
    let _ = wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::AwaitingReview).await;

    common::git::write(&worktree_root, "dirty-junk.txt", "junk\n");
    common::git::write(&worktree_root, "a.txt", "modified\n");
    let dirty_status = common::git::git(&worktree_root, &["status", "--porcelain"]);
    assert!(
        !dirty_status.is_empty(),
        "worktree should be dirty before re-run: {dirty_status:?}"
    );

    let (status, _) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "construct", "userFeedback": "revise slice" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let _ = wait_for_phase(&mut rx, PhaseName::Construct, PhaseState::Running).await;

    let clean_status = common::git::git(&worktree_root, &["status", "--porcelain"]);
    assert_eq!(
        clean_status, "",
        "construct spawn should reset worktree to parent baseline"
    );
    assert!(
        !worktree_root.join("dirty-junk.txt").exists(),
        "untracked junk should be removed by reset"
    );
    let a_contents = std::fs::read_to_string(worktree_root.join("a.txt")).unwrap();
    assert_eq!(a_contents, "a\n", "tracked files should match parent tree");
}

#[tokio::test]
async fn get_subagent_prompts_returns_embedded_bodies() {
    let app = TestApp::spawn_authenticated().await;
    let (status, body) = app.auth_empty("GET", "/api/subagent-prompts").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    for key in ["surveyor", "planner", "constructor"] {
        let text = body[key].as_str().unwrap();
        assert!(!text.is_empty(), "{key} prompt should be non-empty");
    }
}

#[tokio::test]
async fn node_session_lookup_returns_session_id() {
    let app = TestApp::spawn_authenticated().await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let (status, body) = app.auth_empty(
        "GET",
        &format!("/api/nodes/{target_node_id}/session"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["sessionId"].as_str().unwrap(), session_id);
}

#[tokio::test]
async fn node_session_lookup_unknown_returns_404() {
    let app = TestApp::spawn_authenticated().await;
    let (status, _) = app.auth_empty(
        "GET",
        "/api/nodes/00000000-0000-0000-0000-000000000000/session",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_prompt_override_returns_400() {
    let runner = Arc::new(FakeSubagentRunner::new(vec![survey_script()]));
    let app = TestApp::spawn_authenticated_with_runner(runner).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let (status, _) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "survey", "promptOverride": "hello {{NOPE}}" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn prompt_override_appears_in_transcript_prompt() {
    let custom = "You are a **Custom Surveyor**. Trees: {{BEFORE_TREE}} / {{TARGET_TREE}}. Feedback: {{USER_FEEDBACK}}.";
    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        survey_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let (status, run) = app.auth_json(
        "POST",
        &format!("/api/partitions/{partition_id}/runs"),
        json!({ "kind": "survey", "promptOverride": custom }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "run body: {run}");
    let run_id = run["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let (status, transcript) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs/{run_id}/transcript"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let prompt = transcript["prompt"].as_str().unwrap();
    assert!(
        prompt.contains("**Custom Surveyor**"),
        "expected custom prompt in transcript, got: {prompt}"
    );
}

#[tokio::test]
async fn subagent_run_cli_smoke() {
    use eunomio::cli::subagent_run::{run, SubagentRunArgs};
    use eunomio::types::RunKind;

    let runner = Arc::new(FakeSubagentRunner::new(vec![
        survey_script(),
        survey_script(),
    ]));
    let app = TestApp::spawn_authenticated_with_runner(runner).await;
    let (session_id, target_node_id) = create_session_and_pick_target(&app).await;
    let mut rx = app.state.coordinator.subscribe(&session_id);
    let (_, body) = app.auth_empty(
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/partition"),
    )
    .await;
    let partition_id = body["id"].as_str().unwrap().to_string();
    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = app.router.clone();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let partition_id_for_cli = partition_id.clone();
    run(SubagentRunArgs {
        base_url: format!("http://{addr}"),
        partition_id: partition_id_for_cli,
        kind: RunKind::Survey,
        prompt_file: None,
        session_cookie: Some(app.cookie().to_string()),
    })
    .await
    .unwrap();

    let _ = wait_for_phase(&mut rx, PhaseName::Survey, PhaseState::AwaitingReview).await;

    let (_, runs) = app.auth_empty(
        "GET",
        &format!("/api/partitions/{partition_id}/runs"),
    )
    .await;
    let latest = runs
        .as_array()
        .unwrap()
        .iter()
        .max_by_key(|r| r["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(latest["status"].as_str().unwrap(), "finished");
}

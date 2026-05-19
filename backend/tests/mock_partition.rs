use axum::http::StatusCode;
use eunomia::types::SseEvent;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;

mod common;
use common::{empty_request, json_request, TestApp};

async fn create_session_and_pick_node(app: &TestApp) -> (String, String) {
    let (status, body) = json_request(
        &app.router,
        "POST",
        "/api/sessions",
        json!({ "baseRef": "main", "sourceRef": "feature" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create session: {body}");
    let session_id = body["id"].as_str().unwrap().to_string();

    let (_, graph) = empty_request(
        &app.router,
        "GET",
        &format!("/api/sessions/{session_id}/graph"),
    )
    .await;
    let target_node_id = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| !n["parentNodeId"].is_null())
        .unwrap()["nodeId"]
        .as_str()
        .unwrap()
        .to_string();

    (session_id, target_node_id)
}

async fn next_event(
    rx: &mut tokio::sync::broadcast::Receiver<SseEvent>,
) -> SseEvent {
    timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timed out waiting for SSE event")
        .expect("SSE channel closed")
}

#[tokio::test]
async fn mock_partition_survey_gate_rerun_and_continue() {
    let app = TestApp::spawn().await;
    let (session_id, target_node_id) = create_session_and_pick_node(&app).await;

    let (status, _) = json_request(
        &app.router,
        "PATCH",
        &format!("/api/sessions/{session_id}/partition-settings"),
        json!({ "coordinator": { "humanInTheLoop": { "afterSurvey": true } } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut rx = app.state.mock_partitions.subscribe(&session_id);

    let (status, body) = json_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/mock-partition"),
        json!({ "strategy": "semantic", "userConcern": "test" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "start body: {body}");

    assert!(matches!(next_event(&mut rx).await, SseEvent::Started { .. }));
    let phase = next_event(&mut rx).await;
    matches!(
        phase,
        SseEvent::Phase { state: eunomia::types::PhaseState::Running, .. },
    );
    assert!(matches!(next_event(&mut rx).await, SseEvent::SdkMessage { .. }));
    matches!(
        next_event(&mut rx).await,
        SseEvent::Phase { state: eunomia::types::PhaseState::Done, .. },
    );
    matches!(
        next_event(&mut rx).await,
        SseEvent::Phase { state: eunomia::types::PhaseState::AwaitingReview, .. },
    );

    let (status, _) = json_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/mock-partition/rerun"),
        json!({ "userFeedback": "please add a fourth concern" }),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let phase = next_event(&mut rx).await;
    matches!(
        phase,
        SseEvent::Phase { state: eunomia::types::PhaseState::Running, .. },
    );
    assert!(matches!(next_event(&mut rx).await, SseEvent::SdkMessage { .. }));
    matches!(
        next_event(&mut rx).await,
        SseEvent::Phase { state: eunomia::types::PhaseState::Done, .. },
    );
    matches!(
        next_event(&mut rx).await,
        SseEvent::Phase { state: eunomia::types::PhaseState::AwaitingReview, .. },
    );

    let (status, _) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/mock-partition/continue"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let next = next_event(&mut rx).await;
    assert!(matches!(
        next,
        SseEvent::Phase {
            name: eunomia::types::PhaseName::Plan,
            state: eunomia::types::PhaseState::Running,
            ..
        }
    ));

    let (status, _) = empty_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/mock-partition/abandon"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn mock_partition_rejects_second_start() {
    let app = TestApp::spawn().await;
    let (session_id, target_node_id) = create_session_and_pick_node(&app).await;

    let (status, _) = json_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/mock-partition"),
        json!({ "strategy": "vertical" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = json_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/{target_node_id}/mock-partition"),
        json!({ "strategy": "vertical" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"].as_str().unwrap(), "partition_in_flight");
}

#[tokio::test]
async fn mock_partition_rejects_unknown_node() {
    let app = TestApp::spawn().await;
    let (session_id, _) = create_session_and_pick_node(&app).await;

    let (status, _) = json_request(
        &app.router,
        "POST",
        &format!("/api/sessions/{session_id}/edges/does-not-exist/mock-partition"),
        json!({ "strategy": "semantic" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

use crate::{
    branching, edges, embed,
    error::AppError,
    middleware::host_guard,
    repo, sessions, sse,
    state::AppState,
    types::*,
};
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware::from_fn_with_state,
    response::{sse::Event, IntoResponse, Sse},
    routing::{delete, get, patch, post},
    Json, Router,
};
use futures::stream::Stream;
use std::{convert::Infallible, net::SocketAddr};
use tower_http::trace::TraceLayer;

pub use crate::state::{build_state, build_state_with_runner};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/sessions", post(create_session).get(list_sessions))
        .route(
            "/api/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route("/api/sessions/:id/graph", get(get_graph))
        .route("/api/sessions/:id/edges/:target_node_id", get(get_edge))
        .route("/api/sessions/:id/diff", get(get_diff))
        .route("/api/sessions/:id/nodes/:node_id", patch(rename_node))
        .route(
            "/api/sessions/:id/nodes/:node_id/branch",
            post(branch_node),
        )
        .route(
            "/api/partition-settings",
            get(get_partition_settings).patch(patch_partition_settings),
        )
        .route(
            "/api/sessions/:id/edges/:target_node_id/partition",
            post(begin_partition),
        )
        .route("/api/sessions/:id/partitions", get(list_partitions))
        .route("/api/partitions/:partition_id", get(get_partition))
        .route(
            "/api/partitions/:partition_id/runs",
            get(list_runs).post(start_run),
        )
        .route(
            "/api/partitions/:partition_id/runs/:run_id/transcript",
            get(get_run_transcript),
        )
        .route(
            "/api/partitions/:partition_id/runs/:run_id",
            delete(cancel_run),
        )
        .route(
            "/api/partitions/:partition_id/survey/accept",
            post(accept_survey),
        )
        .route(
            "/api/partitions/:partition_id/plan/accept",
            post(accept_plan),
        )
        .route(
            "/api/partitions/:partition_id/construct/accept",
            post(accept_construct),
        )
        .route(
            "/api/partitions/:partition_id/abandon",
            post(abandon_partition),
        )
        .route("/api/sessions/:id/events", get(session_events))
        .route("/api/cursor-models", get(get_cursor_models))
        .route("/api/subagent-prompts", get(get_subagent_prompts))
        .route("/api/nodes/:node_id/session", get(get_node_session))
        .route("/api/repo", get(get_repo_hints))
        .route(
            "/api/tunnel",
            get(get_tunnel).post(start_tunnel).delete(stop_tunnel),
        )
        .route("/api/tunnel/events", get(tunnel_events))
        .fallback(embed::fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(state: AppState, port: u16) -> Result<()> {
    let app = router(state.clone()).layer(from_fn_with_state(state, host_guard));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("eunomia listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), AppError> {
    let (session, outcome) = sessions::create(&state, req).await?;
    let status = match outcome {
        sessions::CreateOutcome::Created => StatusCode::CREATED,
        sessions::CreateOutcome::Existed => StatusCode::OK,
    };
    Ok((status, Json(session)))
}

async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<Session>>, AppError> {
    Ok(Json(repo::session::list(&state).await?))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Session>, AppError> {
    repo::session::get(&state, &id).await?.map(Json).ok_or(AppError::NotFound)
}

async fn get_graph(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Graph>, AppError> {
    repo::session::ensure(&state, &id).await?;
    let nodes = repo::node::list_for_session(&state, &id).await?;
    if nodes.is_empty() {
        return Err(AppError::NotFound);
    }
    let edges: Vec<GraphEdge> = nodes
        .iter()
        .filter_map(|n| {
            n.parent_node_id.as_ref().map(|p| GraphEdge {
                from: p.clone(),
                to: n.node_id.clone(),
            })
        })
        .collect();
    Ok(Json(Graph { nodes, edges }))
}

async fn get_edge(
    State(state): State<AppState>,
    Path((session_id, target_node_id)): Path<(String, String)>,
) -> Result<Json<Edge>, AppError> {
    Ok(Json(
        edges::load_edge_for_target(&state, &session_id, target_node_id).await?,
    ))
}

async fn get_diff(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<DiffQuery>,
) -> Result<Json<Diff>, AppError> {
    let Some((base_tree, final_tree)) = repo::session::seed_trees(&state, &session_id).await? else {
        return Err(AppError::NotFound);
    };
    let mut trees_to_check: Vec<&str> = vec![&q.from_tree, &q.to_tree];
    if let Some(ref r) = q.before_ref {
        trees_to_check.push(r);
    }
    if let Some(ref r) = q.after_ref {
        trees_to_check.push(r);
    }
    if !repo::tree::trees_known_in_session(&state, &session_id, &trees_to_check).await? {
        return Err(AppError::NotFound);
    }
    let before_ref = q.before_ref.as_deref().unwrap_or(&base_tree);
    let after_ref = q.after_ref.as_deref().unwrap_or(&final_tree);
    let git_root = repo::session::git_root(&state, &session_id).await?;
    let (diff, files, synthesized) = edges::render_edge_diff(
        &git_root,
        &q.from_tree,
        &q.to_tree,
        before_ref,
        after_ref,
    )
    .await?;
    Ok(Json(Diff {
        from_tree: q.from_tree,
        to_tree: q.to_tree,
        diff,
        files,
        synthesized,
    }))
}

async fn rename_node(
    State(state): State<AppState>,
    Path((session_id, node_id)): Path<(String, String)>,
    Json(req): Json<RenameNodeRequest>,
) -> Result<Json<GraphNode>, AppError> {
    if req.title.trim().is_empty() {
        return Err(AppError::BadRequest("title must be non-empty".into()));
    }
    repo::session::ensure(&state, &session_id).await?;
    repo::node::update_title(&state, &session_id, &node_id, &req.title)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn branch_node(
    State(state): State<AppState>,
    Path((session_id, node_id)): Path<(String, String)>,
    Json(req): Json<BranchFromNodeRequest>,
) -> Result<Json<BranchFromNodeResponse>, AppError> {
    let tip =
        branching::branch_from_node(&state, &session_id, &node_id, &req.branch_name, req.force)
            .await?;
    Ok(Json(BranchFromNodeResponse {
        branch_name: req.branch_name,
        commit_sha: tip,
    }))
}

async fn get_partition_settings(
    State(state): State<AppState>,
) -> Result<Json<PartitionSettings>, AppError> {
    Ok(Json(state.partition_settings.snapshot().await))
}

async fn patch_partition_settings(
    State(state): State<AppState>,
    Json(patch): Json<PartitionSettingsPatch>,
) -> Result<Json<PartitionSettings>, AppError> {
    let merged = state.partition_settings.apply_patch(patch).await?;
    Ok(Json(merged))
}

async fn get_cursor_models(
    State(state): State<AppState>,
) -> Result<Json<CursorModels>, AppError> {
    if state.cursor_api_key.is_none() {
        return Err(AppError::Unrecoverable {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "cursor_sdk_unavailable".into(),
            message: "CURSOR_API_KEY not configured".into(),
        });
    }
    let models = state
        .cursor_models
        .get_or_try_init(|| crate::cursor_bridge::list_models(&state))
        .await?;
    Ok(Json(CursorModels {
        models: models.clone(),
    }))
}

async fn get_subagent_prompts(State(state): State<AppState>) -> Json<SubagentDefaultPrompts> {
    Json(state.coordinator.default_prompts())
}

async fn get_node_session(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<NodeSessionLookup>, AppError> {
    let session_id = repo::node::session_for_node_id(&state, &node_id).await?;
    Ok(Json(NodeSessionLookup { session_id }))
}

async fn get_repo_hints() -> Json<RepoHints> {
    Json(crate::repo_store::cwd_hints().await)
}

async fn get_tunnel(State(state): State<AppState>) -> Json<TunnelStatus> {
    Json(state.tunnel.status())
}

async fn start_tunnel(
    State(state): State<AppState>,
) -> Result<Json<TunnelStatus>, AppError> {
    let auth_router = router(state.clone());
    let dto = state.tunnel.start(auth_router).await?;
    Ok(Json(dto))
}

async fn stop_tunnel(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    state.tunnel.stop()?;
    Ok(StatusCode::NO_CONTENT)
}

async fn tunnel_events(
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if !state.tunnel.status().enabled {
        return Err(AppError::Unrecoverable {
            status: StatusCode::FORBIDDEN,
            code: "tunnel_disabled".into(),
            message: "tunnel sharing is not enabled".into(),
        });
    }
    let rx = state.tunnel.subscribe();
    let initial = Some(state.tunnel.status_redacted());
    Ok(sse::json_broadcast_stream(rx, initial))
}

async fn session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.coordinator.subscribe(&session_id);
    sse::json_broadcast_stream::<SseEvent>(rx, None)
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    sessions::delete(&state, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn begin_partition(
    State(state): State<AppState>,
    Path((session_id, target_node_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Partition>), AppError> {
    let partition = state
        .coordinator
        .begin_partition(&state, &session_id, &target_node_id)
        .await?;
    Ok((StatusCode::CREATED, Json(partition)))
}

async fn list_partitions(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<ListPartitionsQuery>,
) -> Result<Json<Vec<Partition>>, AppError> {
    let partitions = state
        .coordinator
        .list_partitions(&state, &session_id, q.target_node_id.as_deref())
        .await?;
    Ok(Json(partitions))
}

async fn get_partition(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
) -> Result<Json<Partition>, AppError> {
    Ok(Json(state.coordinator.get_partition(&state, partition_id).await?))
}

async fn start_run(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
    Json(req): Json<StartRunRequest>,
) -> Result<(StatusCode, Json<Run>), AppError> {
    let run = state.coordinator.start_run(&state, partition_id, req).await?;
    Ok((StatusCode::CREATED, Json(run)))
}

async fn list_runs(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
) -> Result<Json<Vec<Run>>, AppError> {
    Ok(Json(state.coordinator.list_runs(&state, partition_id).await?))
}

async fn cancel_run(
    State(state): State<AppState>,
    Path((partition_id, run_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    state
        .coordinator
        .cancel_run(&state, partition_id, run_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_run_transcript(
    State(state): State<AppState>,
    Path((partition_id, run_id)): Path<(i64, i64)>,
) -> Result<Json<Transcript>, AppError> {
    Ok(Json(
        state
            .coordinator
            .get_transcript(&state, partition_id, run_id)
            .await?,
    ))
}

async fn accept_survey(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
    Json(req): Json<AcceptSurveyRequest>,
) -> Result<Json<Partition>, AppError> {
    Ok(Json(
        state.coordinator.accept_survey(&state, partition_id, req).await?,
    ))
}

async fn accept_plan(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
    Json(req): Json<AcceptPlanRequest>,
) -> Result<Json<Partition>, AppError> {
    Ok(Json(
        state.coordinator.accept_plan(&state, partition_id, req).await?,
    ))
}

async fn accept_construct(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.coordinator.accept_construct(&state, partition_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn abandon_partition(
    State(state): State<AppState>,
    Path(partition_id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.coordinator.abandon_partition(&state, partition_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

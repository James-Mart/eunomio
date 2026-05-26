// SPDX-License-Identifier: Apache-2.0

use crate::{
    auth::{
        auth_routes, public_auth_routes, require_csrf_header, require_principal, CurrentPrincipal,
    },
    branching, edge_file_viewed, edges, embed,
    launch::public_launch_routes,
    middleware::host_guard,
    node_reviewed, partition_settings, sessions, sse,
    state::AppState,
    AppError, ServerError,
};
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware::{from_fn, from_fn_with_state},
    response::{sse::Event, IntoResponse, Sse},
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use eunomio_core::types::*;
use futures::stream::Stream;
use std::{convert::Infallible, net::SocketAddr};
use tower_http::trace::TraceLayer;

pub use crate::state::{build_state, BuildStateOptions};

fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", post(create_session).get(list_sessions))
        .route("/api/sessions/validate", post(validate_session))
        .route("/api/sessions/:id", get(get_session).delete(delete_session))
        .route("/api/sessions/:id/graph", get(get_graph))
        .route("/api/sessions/:id/edges/:target_node_id", get(get_edge))
        .route(
            "/api/sessions/:id/nodes/:node_id/shaving-track",
            get(get_shaving_track),
        )
        .route(
            "/api/sessions/:id/edges/:target_node_id/viewed",
            get(edge_file_viewed::get_edge_viewed),
        )
        .route(
            "/api/sessions/:id/edges/:target_node_id/viewed/:file_path",
            put(edge_file_viewed::put_edge_file_viewed),
        )
        .route("/api/sessions/:id/diff", get(get_diff))
        .route("/api/sessions/:id/nodes/:node_id", patch(rename_node))
        .route(
            "/api/sessions/:id/nodes/:node_id/reviewed",
            put(node_reviewed::put_node_reviewed),
        )
        .route("/api/sessions/:id/nodes/:node_id/branch", post(branch_node))
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
        .route(
            "/api/partitions/:partition_id/finish",
            post(finish_partition),
        )
        .route("/api/sessions/:id/events", get(session_events))
        .route("/api/cursor-models", get(get_cursor_models))
        .route("/api/subagent-prompts", get(get_subagent_prompts))
        .route("/api/nodes/:node_id/session", get(get_node_session))
        .route("/api/repo", get(get_repo_hints))
        .route("/api/repo/resolve-pull-request", post(resolve_pull_request))
        .route(
            "/api/tunnel",
            get(get_tunnel).post(start_tunnel).delete(stop_tunnel),
        )
        .route("/api/tunnel/events", get(tunnel_events))
        .merge(auth_routes())
}

pub fn router(state: AppState) -> Router {
    let protected = protected_routes().layer(from_fn_with_state(state.clone(), require_principal));

    Router::new()
        .merge(public_auth_routes())
        .merge(public_launch_routes())
        .merge(protected)
        .fallback(embed::fallback)
        .layer(TraceLayer::new_for_http())
        .layer(from_fn(require_csrf_header))
        .layer(from_fn_with_state(state.clone(), host_guard))
        .with_state(state)
}

pub async fn serve(state: AppState, port: u16) -> Result<()> {
    let app = router(state.clone());
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("eunomio listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_session(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), ServerError> {
    let (session, outcome) =
        sessions::create(&state, &principal.org_id, &principal.user_id, req).await?;
    let status = match outcome {
        sessions::CreateOutcome::Created => StatusCode::CREATED,
        sessions::CreateOutcome::Existed => StatusCode::OK,
    };
    Ok((status, Json(session)))
}

async fn validate_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<StatusCode, ServerError> {
    sessions::validate(&state, &req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_sessions(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
) -> Result<Json<Vec<Session>>, ServerError> {
    Ok(Json(
        state.datastore.sessions().list(&principal.org_id).await?,
    ))
}

async fn get_session(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(id): Path<String>,
) -> Result<Json<Session>, ServerError> {
    state
        .datastore
        .sessions()
        .get(&principal.org_id, &id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound.into())
}

async fn get_graph(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(id): Path<String>,
) -> Result<Json<Graph>, ServerError> {
    state
        .datastore
        .sessions()
        .ensure(&principal.org_id, &id)
        .await?;
    let mut nodes = state
        .datastore
        .nodes()
        .list_for_session(&principal.org_id, &id)
        .await?;
    if nodes.is_empty() {
        return Err(AppError::NotFound.into());
    }
    let reviewed_ids = state
        .datastore
        .node_reviewed()
        .list_node_ids(&principal.org_id, &principal.user_id, &id)
        .await?;
    let reviewed: std::collections::HashSet<_> = reviewed_ids.into_iter().collect();
    for node in &mut nodes {
        node.reviewed = reviewed.contains(&node.node_id);
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
    principal: CurrentPrincipal,
    Path((session_id, target_node_id)): Path<(String, String)>,
) -> Result<Json<Edge>, ServerError> {
    Ok(Json(
        edges::load_edge_for_target(&state, &principal.org_id, &session_id, target_node_id).await?,
    ))
}

async fn get_shaving_track(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((session_id, node_id)): Path<(String, String)>,
) -> Result<Json<ShavingTrackResponse>, ServerError> {
    state
        .datastore
        .sessions()
        .ensure(&principal.org_id, &session_id)
        .await?;
    let track = state
        .datastore
        .shaving_tracks()
        .get(&principal.org_id, &session_id, &node_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let git_root =
        crate::repo_store::session_git_root(&state, &principal.org_id, &session_id).await?;
    let mut step_diffs = Vec::with_capacity(track.steps.len() + 1);
    for (idx, step) in track.steps.iter().enumerate() {
        let from_tree = if idx == 0 {
            track.parent_tree_sha.as_str()
        } else {
            track.steps[idx - 1].tree_sha.as_str()
        };
        let (diff, files, synthesized) = edges::render_edge_diff(
            &git_root,
            from_tree,
            &step.tree_sha,
            &track.parent_tree_sha,
            &track.head_tree_sha,
        )
        .await?;
        step_diffs.push(Diff {
            from_tree: from_tree.to_string(),
            to_tree: step.tree_sha.clone(),
            diff,
            files,
            synthesized,
        });
    }
    let (diff, files, synthesized) = edges::render_edge_diff(
        &git_root,
        &track.parent_tree_sha,
        &track.head_tree_sha,
        &track.parent_tree_sha,
        &track.head_tree_sha,
    )
    .await?;
    step_diffs.push(Diff {
        from_tree: track.parent_tree_sha.clone(),
        to_tree: track.head_tree_sha.clone(),
        diff,
        files,
        synthesized,
    });

    Ok(Json(ShavingTrackResponse {
        target_node_id: track.target_node_id,
        parent_tree_sha: track.parent_tree_sha,
        head_tree_sha: track.head_tree_sha,
        steps: track.steps,
        step_diffs,
    }))
}

async fn get_diff(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(session_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<DiffQuery>,
) -> Result<Json<Diff>, ServerError> {
    let (base_tree, final_tree) = state
        .datastore
        .sessions()
        .seed_trees(&principal.org_id, &session_id)
        .await?;
    let before_ref = q.before_ref.as_deref().unwrap_or(&base_tree);
    let after_ref = q.after_ref.as_deref().unwrap_or(&final_tree);
    let trees_to_check: Vec<&str> = vec![&q.from_tree, &q.to_tree, before_ref, after_ref];
    if !state
        .datastore
        .diff_authorization()
        .trees_authorized_for_diff(&principal.org_id, &session_id, &trees_to_check)
        .await?
    {
        return Err(AppError::NotFound.into());
    }
    let git_root =
        crate::repo_store::session_git_root(&state, &principal.org_id, &session_id).await?;
    let (diff, files, synthesized) =
        edges::render_edge_diff(&git_root, &q.from_tree, &q.to_tree, before_ref, after_ref).await?;
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
    principal: CurrentPrincipal,
    Path((session_id, node_id)): Path<(String, String)>,
    Json(req): Json<RenameNodeRequest>,
) -> Result<Json<GraphNode>, ServerError> {
    if req.title.trim().is_empty() {
        return Err(AppError::BadRequest("title must be non-empty".into()).into());
    }
    state
        .datastore
        .sessions()
        .ensure(&principal.org_id, &session_id)
        .await?;
    let node = state
        .datastore
        .nodes()
        .update_title(&principal.org_id, &session_id, &node_id, &req.title)
        .await?;
    Ok(Json(node))
}

async fn branch_node(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((session_id, node_id)): Path<(String, String)>,
    Json(req): Json<BranchFromNodeRequest>,
) -> Result<Json<BranchFromNodeResponse>, ServerError> {
    let tip = branching::branch_from_node(
        &state,
        &principal.org_id,
        &session_id,
        &node_id,
        &req.branch_name,
        req.force,
    )
    .await?;
    Ok(Json(BranchFromNodeResponse {
        branch_name: req.branch_name,
        commit_sha: tip,
    }))
}

async fn get_partition_settings(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
) -> Result<Json<PartitionSettings>, ServerError> {
    Ok(Json(
        partition_settings::load_for_user(&state.data_dir, &principal.user_id).await?,
    ))
}

async fn patch_partition_settings(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Json(patch): Json<PartitionSettingsPatch>,
) -> Result<Json<PartitionSettings>, ServerError> {
    let mut settings =
        partition_settings::load_for_user(&state.data_dir, &principal.user_id).await?;
    settings.apply_patch(patch);
    partition_settings::save_for_user(&state.data_dir, &principal.user_id, &settings).await?;
    Ok(Json(settings))
}

async fn get_cursor_models(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
) -> Result<Json<CursorModels>, ServerError> {
    let api_key = state
        .keystore
        .get(&principal.user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("reading cursor api key: {e}")))?
        .ok_or_else(|| AppError::Unrecoverable {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "cursor_sdk_unavailable".into(),
            message: "Cursor API key not configured".into(),
        })?;
    let models = state.coordinator.list_models(&api_key).await?;
    Ok(Json(CursorModels { models }))
}

async fn get_subagent_prompts(State(state): State<AppState>) -> Json<SubagentDefaultPrompts> {
    Json(state.coordinator.default_prompts())
}

async fn get_node_session(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(node_id): Path<String>,
) -> Result<Json<NodeSessionLookup>, ServerError> {
    let session_id = state
        .datastore
        .nodes()
        .session_for_node_id(&principal.org_id, &node_id)
        .await?;
    Ok(Json(NodeSessionLookup { session_id }))
}

async fn get_repo_hints() -> Json<RepoHints> {
    Json(crate::repo_store::cwd_hints().await)
}

async fn resolve_pull_request(
    Json(req): Json<ResolvePullRequestRequest>,
) -> Result<Json<ResolvedPullRequest>, ServerError> {
    let resolved = crate::github::resolve_pull_request(&req.pull_request_url).await?;
    Ok(Json(resolved))
}

async fn get_tunnel(State(state): State<AppState>) -> Json<TunnelStatus> {
    Json(state.tunnel.status())
}

async fn start_tunnel(State(state): State<AppState>) -> Result<Json<TunnelStatus>, ServerError> {
    let auth_router = router(state.clone());
    let dto = state.tunnel.start(auth_router).await?;
    Ok(Json(dto))
}

async fn stop_tunnel(State(state): State<AppState>) -> Result<StatusCode, ServerError> {
    state.tunnel.stop()?;
    Ok(StatusCode::NO_CONTENT)
}

async fn tunnel_events(
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ServerError> {
    if !state.tunnel.status().enabled {
        return Err(AppError::Unrecoverable {
            status: StatusCode::FORBIDDEN,
            code: "tunnel_disabled".into(),
            message: "tunnel sharing is not enabled".into(),
        }
        .into());
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
    principal: CurrentPrincipal,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServerError> {
    sessions::delete(&state, &principal.org_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn begin_partition(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((session_id, target_node_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Partition>), ServerError> {
    let partition = state
        .coordinator
        .begin_partition(&state, &principal.org_id, &session_id, &target_node_id)
        .await?;
    Ok((StatusCode::CREATED, Json(partition)))
}

async fn list_partitions(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(session_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<ListPartitionsQuery>,
) -> Result<Json<Vec<Partition>>, ServerError> {
    let partitions = state
        .coordinator
        .list_partitions(
            &state,
            &principal.org_id,
            &session_id,
            q.target_node_id.as_deref(),
        )
        .await?;
    Ok(Json(partitions))
}

async fn get_partition(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
) -> Result<Json<Partition>, ServerError> {
    Ok(Json(
        state
            .coordinator
            .get_partition(&state, &principal.org_id, &partition_id)
            .await?,
    ))
}

async fn start_run(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
    Json(req): Json<StartRunRequest>,
) -> Result<(StatusCode, Json<Run>), ServerError> {
    let run = state
        .coordinator
        .start_run(&state, &principal.org_id, &partition_id, req)
        .await?;
    Ok((StatusCode::CREATED, Json(run)))
}

async fn list_runs(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
) -> Result<Json<Vec<Run>>, ServerError> {
    Ok(Json(
        state
            .coordinator
            .list_runs(&state, &principal.org_id, &partition_id)
            .await?,
    ))
}

async fn cancel_run(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((partition_id, run_id)): Path<(String, String)>,
) -> Result<StatusCode, ServerError> {
    state
        .coordinator
        .cancel_run(&state, &principal.org_id, &partition_id, &run_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_run_transcript(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path((partition_id, run_id)): Path<(String, String)>,
) -> Result<Json<Transcript>, ServerError> {
    Ok(Json(
        state
            .coordinator
            .get_transcript(&state, &principal.org_id, &partition_id, &run_id)
            .await?,
    ))
}

async fn accept_survey(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
    Json(req): Json<AcceptSurveyRequest>,
) -> Result<Json<Partition>, ServerError> {
    Ok(Json(
        state
            .coordinator
            .accept_survey(&state, &principal.org_id, &partition_id, req)
            .await?,
    ))
}

async fn accept_plan(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
    Json(req): Json<AcceptPlanRequest>,
) -> Result<Json<Partition>, ServerError> {
    Ok(Json(
        state
            .coordinator
            .accept_plan(&state, &principal.org_id, &partition_id, req)
            .await?,
    ))
}

async fn accept_construct(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
) -> Result<StatusCode, ServerError> {
    state
        .coordinator
        .accept_construct(&state, &principal.org_id, &partition_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn abandon_partition(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
) -> Result<StatusCode, ServerError> {
    state
        .coordinator
        .abandon_partition(&state, &principal.org_id, &partition_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn finish_partition(
    State(state): State<AppState>,
    principal: CurrentPrincipal,
    Path(partition_id): Path<String>,
) -> Result<StatusCode, ServerError> {
    state
        .coordinator
        .finish_partition(&state, &principal.org_id, &partition_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

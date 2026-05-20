use crate::{
    branching,
    coordinator::Coordinator,
    cursor_bridge::{CursorHelperRunner, SubagentRunner},
    db, embed,
    error::AppError,
    partition_settings::PartitionSettingsStore,
    sessions,
    subagents::load_subagents,
    tunnel::TunnelRegistry,
    types::*,
};
use anyhow::{Context, Result};
use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, patch, post},
    Json, Router,
};
use futures::stream::{Stream, StreamExt};
use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::OnceCell;
use tokio_rusqlite::Connection;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub repo_root: PathBuf,
    pub data_dir: PathBuf,
    pub db: Connection,
    pub cursor_api_key: Option<String>,
    pub cursor_models: OnceCell<Vec<CursorModel>>,
    pub partition_settings: PartitionSettingsStore,
    pub coordinator: Coordinator,
    pub tunnel: TunnelRegistry,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn build_state(
    repo_root: PathBuf,
    data_dir: PathBuf,
    cursor_api_key: Option<String>,
    dev_tunnel: bool,
) -> Result<AppState> {
    let runner: Arc<dyn SubagentRunner> = Arc::new(CursorHelperRunner::new(
        cursor_api_key.clone(),
        data_dir.clone(),
    ));
    build_state_with_runner(repo_root, data_dir, cursor_api_key, dev_tunnel, runner).await
}

pub async fn build_state_with_runner(
    repo_root: PathBuf,
    data_dir: PathBuf,
    cursor_api_key: Option<String>,
    dev_tunnel: bool,
    runner: Arc<dyn SubagentRunner>,
) -> Result<AppState> {
    tokio::fs::create_dir_all(&data_dir)
        .await
        .with_context(|| format!("create_dir_all {}", data_dir.display()))?;
    let db = db::open(&data_dir.join("eunomia.db")).await?;
    let settings_path = data_dir.join("settings.json");
    let partition_settings = PartitionSettingsStore::load(settings_path).await?;
    let tunnel = TunnelRegistry::new(data_dir.clone(), dev_tunnel);
    let subagents = load_subagents()?;
    let coordinator = Coordinator::new(subagents, runner);
    let state = AppState(Arc::new(AppStateInner {
        repo_root,
        data_dir,
        db,
        cursor_api_key,
        cursor_models: OnceCell::new(),
        partition_settings,
        coordinator: coordinator.clone(),
        tunnel,
    }));
    coordinator.process_startup_recovery(&state).await?;
    Ok(state)
}

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
        .route("/api/repo", get(get_repo_info))
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

/// Reject requests whose `Host` (or `Origin`, when present) names anything
/// other than loopback. Defends against CSRF from arbitrary sites the user
/// has open and against DNS-rebinding reads, since browsers will happily
/// connect to 127.0.0.1 under an attacker-controlled hostname.
///
/// In `--dev-tunnel` mode we additionally accept `Origin` headers that name a
/// `*.trycloudflare.com` subdomain. The browser loads the UI from the public
/// cloudflared URL and Vite proxies `/api/*` to the backend with
/// `changeOrigin: true` (which rewrites `Host` to loopback) but leaves the
/// original `Origin` intact. Without this exemption every mutating request
/// through the dev tunnel 403s; with it, the production CSRF/DNS-rebinding
/// defence still applies to every other deployment. The dev tunnel skips the
/// share-token gate by design, so allowing this origin does not weaken any
/// guarantee that wasn't already waived for dev.
async fn host_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let host_header = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host_header.map(is_loopback_host).unwrap_or(false) {
        return forbidden_host();
    }
    if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let dev_origin_ok = state.tunnel.dev_mode() && origin_is_dev_tunnel(origin);
        if !origin_is_loopback(origin) && !dev_origin_ok {
            return forbidden_host();
        }
    }
    next.run(req).await
}

fn forbidden_host() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": "forbidden host", "code": "forbidden_host" })),
    )
        .into_response()
}

fn is_loopback_host(value: &str) -> bool {
    let host = strip_host_port(value);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

fn origin_is_loopback(origin: &str) -> bool {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"));
    rest.map(is_loopback_host).unwrap_or(false)
}

/// Matches origins of the form `https://<sub>.trycloudflare.com`, where
/// `<sub>` is a single non-empty label of ASCII letters, digits, or hyphens
/// (the format cloudflared's Quick Tunnel issues, and the same shape matched
/// by the URL regex in `tunnel.rs`). Used only when `--dev-tunnel` is active.
fn origin_is_dev_tunnel(origin: &str) -> bool {
    let Some(rest) = origin.strip_prefix("https://") else {
        return false;
    };
    let host = strip_host_port(rest);
    let Some(sub) = host.strip_suffix(".trycloudflare.com") else {
        return false;
    };
    !sub.is_empty()
        && !sub.contains('.')
        && sub.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn strip_host_port(value: &str) -> &str {
    if value.starts_with('[') {
        if let Some(end) = value.find(']') {
            return &value[..=end];
        }
    }
    match value.rsplit_once(':') {
        Some((host, _)) if !host.is_empty() && !host.contains(':') => host,
        _ => value,
    }
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), AppError> {
    let base_ref = req.base_ref.clone();
    let source_ref = req.source_ref.clone();
    let (created, outcome) = sessions::create(&state, req).await?;
    let dto = Session {
        id: created.id,
        base_ref,
        source_ref,
        base_node_id: created.base_node_id,
        created_at: created.created_at,
    };
    let status = match outcome {
        sessions::CreateOutcome::Created => StatusCode::CREATED,
        sessions::CreateOutcome::Existed => StatusCode::OK,
    };
    Ok((status, Json(dto)))
}

async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<Session>>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let rows: Vec<Session> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_ref, source_ref, base_node_id, created_at FROM sessions \
                 WHERE repo_root = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![repo_root], |row| {
                    Ok(Session {
                        id: row.get(0)?,
                        base_ref: row.get(1)?,
                        source_ref: row.get(2)?,
                        base_node_id: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(rows))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Session>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let row: Option<Session> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_ref, source_ref, base_node_id, created_at FROM sessions \
                 WHERE id = ?1 AND repo_root = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![id, repo_root])?;
            if let Some(row) = rows.next()? {
                Ok(Some(Session {
                    id: row.get(0)?,
                    base_ref: row.get(1)?,
                    source_ref: row.get(2)?,
                    base_node_id: row.get(3)?,
                    created_at: row.get(4)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    row.map(Json).ok_or(AppError::NotFound)
}

async fn get_graph(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Graph>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let nodes: Vec<GraphNode> = state
        .db
        .call(move |conn| {
            let mut session_stmt =
                conn.prepare("SELECT 1 FROM sessions WHERE id = ?1 AND repo_root = ?2")?;
            let session_present = session_stmt
                .query(tokio_rusqlite::params![id, repo_root])?
                .next()?
                .is_some();
            if !session_present {
                return Ok(Vec::new());
            }
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description \
                 FROM nodes WHERE session_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![id], |row| {
                    Ok(GraphNode {
                        node_id: row.get(0)?,
                        parent_node_id: row.get(1)?,
                        tree_sha: row.get(2)?,
                        commit_sha: row.get(3)?,
                        title: row.get(4)?,
                        description: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

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
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let session_id_for_lookup = session_id.clone();
    let target_for_lookup = target_node_id.clone();
    let lookup: Option<(String, Option<String>, Option<String>, String)> = state
        .db
        .call(move |conn| {
            let mut session_stmt = conn.prepare(
                "SELECT final_tree FROM sessions WHERE id = ?1 AND repo_root = ?2",
            )?;
            let mut session_rows = session_stmt
                .query(tokio_rusqlite::params![session_id_for_lookup, repo_root])?;
            let Some(session_row) = session_rows.next()? else {
                return Ok(None);
            };
            let final_tree: String = session_row.get(0)?;
            let mut target_stmt = conn.prepare(
                "SELECT tree_sha, parent_node_id FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = target_stmt
                .query(tokio_rusqlite::params![session_id_for_lookup, target_for_lookup])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let target_tree: String = row.get(0)?;
            let parent_node_id: Option<String> = row.get(1)?;
            let parent_tree = match &parent_node_id {
                Some(pid) => {
                    let mut parent_stmt = conn.prepare(
                        "SELECT tree_sha FROM nodes WHERE session_id = ?1 AND node_id = ?2",
                    )?;
                    let mut prows =
                        parent_stmt.query(tokio_rusqlite::params![session_id_for_lookup, pid])?;
                    if let Some(prow) = prows.next()? {
                        Some(prow.get::<_, String>(0)?)
                    } else {
                        None
                    }
                }
                None => None,
            };
            Ok(Some((target_tree, parent_node_id, parent_tree, final_tree)))
        })
        .await?;

    let Some((target_tree, parent_node_id, parent_tree, final_tree)) = lookup else {
        return Err(AppError::NotFound);
    };

    let (diff, synthesized) = match (&parent_node_id, &parent_tree) {
        (Some(_), Some(parent_tree)) => {
            let diff =
                crate::git::diff_text(&state.repo_root, parent_tree, &target_tree).await?;
            let synthesized = crate::synthesized_content::compute(
                &state.repo_root,
                parent_tree,
                &target_tree,
                &final_tree,
            )
            .await?;
            (diff, synthesized)
        }
        _ => (String::new(), Default::default()),
    };

    Ok(Json(Edge {
        target_node_id,
        parent_node_id,
        diff,
        synthesized,
    }))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffQuery {
    from_tree: String,
    to_tree: String,
    #[serde(default)]
    reference_tree: Option<String>,
}

async fn get_diff(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<DiffQuery>,
) -> Result<Json<Diff>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let id_for_lookup = session_id.clone();
    let from_tree_lookup = q.from_tree.clone();
    let to_tree_lookup = q.to_tree.clone();
    let reference_tree_lookup = q.reference_tree.clone();
    let lookup: Option<String> = state
        .db
        .call(move |conn| {
            let mut session_stmt = conn.prepare(
                "SELECT final_tree FROM sessions WHERE id = ?1 AND repo_root = ?2",
            )?;
            let mut session_rows = session_stmt
                .query(tokio_rusqlite::params![id_for_lookup, repo_root])?;
            let Some(session_row) = session_rows.next()? else {
                return Ok(None);
            };
            let final_tree: String = session_row.get(0)?;
            let mut trees_to_check: Vec<String> =
                vec![from_tree_lookup.clone(), to_tree_lookup.clone()];
            if let Some(ref r) = reference_tree_lookup {
                trees_to_check.push(r.clone());
            }
            for tree in &trees_to_check {
                let mut found = false;
                let mut nodes_stmt = conn.prepare(
                    "SELECT 1 FROM nodes WHERE session_id = ?1 AND tree_sha = ?2 LIMIT 1",
                )?;
                if nodes_stmt
                    .query(tokio_rusqlite::params![id_for_lookup, tree])?
                    .next()?
                    .is_some()
                {
                    found = true;
                }
                if !found {
                    let mut p_stmt = conn.prepare(
                        "SELECT 1 FROM partitions WHERE session_id = ?1 AND candidate_slice_tree_sha = ?2 LIMIT 1",
                    )?;
                    if p_stmt
                        .query(tokio_rusqlite::params![id_for_lookup, tree])?
                        .next()?
                        .is_some()
                    {
                        found = true;
                    }
                }
                if !found {
                    return Ok(None);
                }
            }
            Ok(Some(final_tree))
        })
        .await?;
    let Some(final_tree) = lookup else {
        return Err(AppError::NotFound);
    };
    let diff = crate::git::diff_text(&state.repo_root, &q.from_tree, &q.to_tree).await?;
    let reference_tree = q.reference_tree.as_deref().unwrap_or(&final_tree);
    let synthesized = crate::synthesized_content::compute(
        &state.repo_root,
        &q.from_tree,
        &q.to_tree,
        reference_tree,
    )
    .await?;
    Ok(Json(Diff {
        from_tree: q.from_tree,
        to_tree: q.to_tree,
        diff,
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
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let updated: Option<GraphNode> = state
        .db
        .call(move |conn| {
            let mut session_stmt =
                conn.prepare("SELECT 1 FROM sessions WHERE id = ?1 AND repo_root = ?2")?;
            let session_present = session_stmt
                .query(tokio_rusqlite::params![session_id, repo_root])?
                .next()?
                .is_some();
            if !session_present {
                return Ok(None);
            }
            let updated = conn.execute(
                "UPDATE nodes SET title = ?1 WHERE session_id = ?2 AND node_id = ?3",
                tokio_rusqlite::params![req.title, session_id, node_id],
            )?;
            if updated == 0 {
                return Ok(None);
            }
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description \
                 FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, node_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(GraphNode {
                    node_id: row.get(0)?,
                    parent_node_id: row.get(1)?,
                    tree_sha: row.get(2)?,
                    commit_sha: row.get(3)?,
                    title: row.get(4)?,
                    description: row.get(5)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    updated.map(Json).ok_or(AppError::NotFound)
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

async fn get_repo_info(State(state): State<AppState>) -> Result<Json<RepoInfo>, AppError> {
    let current_branch = crate::git::current_branch(&state.repo_root).await?;
    Ok(Json(RepoInfo { current_branch }))
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
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tunnel.subscribe();
    let initial = state.tunnel.status_redacted();
    let initial = futures::stream::iter(
        serde_json::to_string(&initial)
            .ok()
            .map(|s| Ok(Event::default().data(s))),
    );
    let updates = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(dto) => match serde_json::to_string(&dto) {
                Ok(data) => Some(Ok(Event::default().data(data))),
                Err(e) => {
                    tracing::error!(error = %e, "failed to serialise tunnel SSE event");
                    None
                }
            },
            Err(_) => None,
        }
    });
    Sse::new(initial.chain(updates)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
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

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ListPartitionsQuery {
    #[serde(default)]
    target_node_id: Option<String>,
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

#[cfg(test)]
mod host_guard_tests {
    use super::*;

    #[test]
    fn allows_loopback_hosts() {
        assert!(is_loopback_host("127.0.0.1:3001"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost:5173"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("[::1]:3001"));
        assert!(is_loopback_host("[::1]"));
        assert!(is_loopback_host("::1"));
    }

    #[test]
    fn rejects_non_loopback_hosts() {
        assert!(!is_loopback_host("example.com"));
        assert!(!is_loopback_host("example.com:3001"));
        assert!(!is_loopback_host("evil.com"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("192.168.1.1"));
        assert!(!is_loopback_host(""));
    }

    #[test]
    fn origin_loopback_classification() {
        assert!(origin_is_loopback("http://127.0.0.1:3001"));
        assert!(origin_is_loopback("http://localhost:5173"));
        assert!(origin_is_loopback("https://[::1]:8080"));
        assert!(!origin_is_loopback("http://evil.com"));
        assert!(!origin_is_loopback("evil.com"));
        assert!(!origin_is_loopback("null"));
    }

    #[test]
    fn accepts_dev_tunnel_origins() {
        assert!(origin_is_dev_tunnel(
            "https://tee-left-stood-ping.trycloudflare.com"
        ));
        assert!(origin_is_dev_tunnel("https://abc123.trycloudflare.com"));
        assert!(origin_is_dev_tunnel(
            "https://a-b-c-1-2-3.trycloudflare.com"
        ));
    }

    #[test]
    fn rejects_non_dev_tunnel_origins() {
        // http:// is rejected — quick tunnels are always https.
        assert!(!origin_is_dev_tunnel(
            "http://tee-left-stood-ping.trycloudflare.com"
        ));
        // Bare apex is rejected (no subdomain).
        assert!(!origin_is_dev_tunnel("https://trycloudflare.com"));
        assert!(!origin_is_dev_tunnel("https://.trycloudflare.com"));
        // Multi-label subdomain is rejected — quick tunnels are single-label.
        assert!(!origin_is_dev_tunnel(
            "https://foo.bar.trycloudflare.com"
        ));
        // Suffix-spoofing must not match.
        assert!(!origin_is_dev_tunnel(
            "https://attacker-trycloudflare.com"
        ));
        assert!(!origin_is_dev_tunnel(
            "https://sub.trycloudflare.com.evil.com"
        ));
        // Disallowed characters in the label.
        assert!(!origin_is_dev_tunnel(
            "https://has_underscore.trycloudflare.com"
        ));
        assert!(!origin_is_dev_tunnel("https://has space.trycloudflare.com"));
        // Other schemes / shapes.
        assert!(!origin_is_dev_tunnel("ftp://sub.trycloudflare.com"));
        assert!(!origin_is_dev_tunnel("https://evil.com"));
        assert!(!origin_is_dev_tunnel("null"));
    }
}

async fn session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.coordinator.subscribe(&session_id);
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(event) => match serde_json::to_string(&event) {
                Ok(data) => Some(Ok(Event::default().data(data))),
                Err(e) => {
                    tracing::error!(error = %e, "failed to serialise SSE event");
                    None
                }
            },
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

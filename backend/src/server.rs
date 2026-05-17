use crate::{branching, db, embed, error::AppError, sessions, types::*};
use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
    Json, Router,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio_rusqlite::Connection;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub repo_root: PathBuf,
    pub data_dir: PathBuf,
    pub db: Connection,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn build_state(repo_root: PathBuf, data_dir: PathBuf) -> Result<AppState> {
    tokio::fs::create_dir_all(&data_dir)
        .await
        .with_context(|| format!("create_dir_all {}", data_dir.display()))?;
    let db = db::open(&data_dir.join("eunomia.db")).await?;
    Ok(AppState(Arc::new(AppStateInner {
        repo_root,
        data_dir,
        db,
    })))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/sessions", post(create_session).get(list_sessions))
        .route(
            "/api/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route("/api/sessions/:id/graph", get(get_graph))
        .route(
            "/api/sessions/:id/nodes/:node_id",
            patch(rename_node),
        )
        .route(
            "/api/sessions/:id/nodes/:node_id/branch",
            post(branch_node),
        )
        .fallback(embed::fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(state: AppState, port: u16) -> Result<()> {
    let app = router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("eunomia listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionDto>), AppError> {
    let base_ref = req.base_ref.clone();
    let source_ref = req.source_ref.clone();
    let created = sessions::create(&state, req).await?;
    let dto = SessionDto {
        id: created.id,
        base_ref,
        source_ref,
        base_node_id: created.base_node_id,
        created_at: created.created_at,
    };
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<SessionDto>>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let rows: Vec<SessionDto> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_ref, source_ref, base_node_id, created_at FROM sessions \
                 WHERE repo_root = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![repo_root], |row| {
                    Ok(SessionDto {
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
) -> Result<Json<SessionDto>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let row: Option<SessionDto> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, base_ref, source_ref, base_node_id, created_at FROM sessions \
                 WHERE id = ?1 AND repo_root = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![id, repo_root])?;
            if let Some(row) = rows.next()? {
                Ok(Some(SessionDto {
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
) -> Result<Json<GraphDto>, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let nodes: Vec<NodeDto> = state
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
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, is_favorite \
                 FROM nodes WHERE session_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![id], |row| {
                    Ok(NodeDto {
                        node_id: row.get(0)?,
                        parent_node_id: row.get(1)?,
                        tree_sha: row.get(2)?,
                        commit_sha: row.get(3)?,
                        title: row.get(4)?,
                        is_favorite: row.get::<_, i64>(5)? != 0,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    if nodes.is_empty() {
        return Err(AppError::NotFound);
    }

    let edges: Vec<EdgeDto> = nodes
        .iter()
        .filter_map(|n| {
            n.parent_node_id.as_ref().map(|p| EdgeDto {
                from: p.clone(),
                to: n.node_id.clone(),
            })
        })
        .collect();
    Ok(Json(GraphDto { nodes, edges }))
}

async fn rename_node(
    State(state): State<AppState>,
    Path((session_id, node_id)): Path<(String, String)>,
    Json(req): Json<RenameNodeRequest>,
) -> Result<Json<NodeDto>, AppError> {
    if req.title.trim().is_empty() {
        return Err(AppError::BadRequest("title must be non-empty".into()));
    }
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let updated: Option<NodeDto> = state
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
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, is_favorite \
                 FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, node_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(NodeDto {
                    node_id: row.get(0)?,
                    parent_node_id: row.get(1)?,
                    tree_sha: row.get(2)?,
                    commit_sha: row.get(3)?,
                    title: row.get(4)?,
                    is_favorite: row.get::<_, i64>(5)? != 0,
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

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let repo_root_str = state.repo_root.to_string_lossy().to_string();
    let id_for_lookup = id.clone();
    let worktree_path: Option<String> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT worktree_path FROM sessions WHERE id = ?1 AND repo_root = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![id_for_lookup, repo_root_str])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get::<_, String>(0)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    let Some(worktree_path) = worktree_path else {
        return Err(AppError::NotFound);
    };

    let wt = PathBuf::from(&worktree_path);
    if wt.exists() {
        if let Err(e) = crate::git::worktree_remove(&state.repo_root, &wt).await {
            tracing::warn!(error = %e, "git worktree remove failed; cleaning up rows anyway");
        }
    }

    let id_for_delete = id.clone();
    state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM nodes WHERE session_id = ?1",
                tokio_rusqlite::params![id_for_delete],
            )?;
            tx.execute(
                "DELETE FROM sessions WHERE id = ?1",
                tokio_rusqlite::params![id_for_delete],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

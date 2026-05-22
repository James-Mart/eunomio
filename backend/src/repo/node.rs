use crate::{error::AppError, state::AppState, types::*};
use super::{require_affected_sqlite, DbResultExt};

#[derive(Debug, Clone)]
pub struct WalkNode {
    pub tree_sha: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct NodeBasic {
    pub node_id: String,
    pub tree_sha: String,
    pub commit_sha: String,
}

pub async fn list_for_session(
    state: &AppState,
    org_id: &str,
    session_id: &str,
) -> Result<Vec<GraphNode>, AppError> {
    let org_id = org_id.to_string();
    let session_id = session_id.to_string();
    let rows: Vec<GraphNode> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description, strategy \
                 FROM nodes WHERE org_id = ?1 AND session_id = ?2 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(
                    tokio_rusqlite::params![org_id, session_id],
                    graph_node_mapper,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn get(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    node_id: &str,
) -> Result<Option<GraphNode>, AppError> {
    let org_id = org_id.to_string();
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let row: Option<GraphNode> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description, strategy \
                 FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![org_id, session_id, node_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(graph_node_mapper(row)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(row)
}

pub async fn target_and_parent(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    target_node_id: &str,
) -> Result<(NodeBasic, Option<NodeBasic>), AppError> {
    let org_id = org_id.to_string();
    let session_id = session_id.to_string();
    let target_node_id = target_node_id.to_string();
    let result: Option<(NodeBasic, Option<NodeBasic>)> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3",
            )?;
            let mut rows =
                stmt.query(tokio_rusqlite::params![org_id, session_id, target_node_id])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let target_node = NodeBasic {
                node_id: row.get(0)?,
                tree_sha: row.get(2)?,
                commit_sha: row.get(3)?,
            };
            let parent_id: Option<String> = row.get(1)?;
            let parent_node = if let Some(pid) = parent_id {
                let mut pstmt = conn.prepare(
                    "SELECT node_id, tree_sha, commit_sha FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3",
                )?;
                let mut prows =
                    pstmt.query(tokio_rusqlite::params![org_id, session_id, pid])?;
                if let Some(prow) = prows.next()? {
                    Some(NodeBasic {
                        node_id: prow.get(0)?,
                        tree_sha: prow.get(1)?,
                        commit_sha: prow.get(2)?,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            Ok(Some((target_node, parent_node)))
        })
        .await?;
    result.ok_or(AppError::NotFound)
}

pub async fn target_tree_and_parent(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    target_node_id: &str,
) -> Result<Option<(String, Option<String>, Option<String>)>, AppError> {
    let org_id = org_id.to_string();
    let session_id = session_id.to_string();
    let target_node_id = target_node_id.to_string();
    let row: Option<(String, Option<String>, Option<String>)> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT tree_sha, parent_node_id FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3",
            )?;
            let mut rows =
                stmt.query(tokio_rusqlite::params![org_id, session_id, target_node_id])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let target_tree: String = row.get(0)?;
            let parent_node_id: Option<String> = row.get(1)?;
            let parent_tree = match &parent_node_id {
                Some(pid) => {
                    let mut pstmt = conn.prepare(
                        "SELECT tree_sha FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3",
                    )?;
                    let mut prows =
                        pstmt.query(tokio_rusqlite::params![org_id, session_id, pid])?;
                    prows.next()?.map(|r| r.get::<_, String>(0)).transpose()?
                }
                None => None,
            };
            Ok(Some((target_tree, parent_node_id, parent_tree)))
        })
        .await?;
    Ok(row)
}

pub async fn update_title(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    node_id: &str,
    title: &str,
) -> Result<GraphNode, AppError> {
    let org_id = org_id.to_string();
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let title = title.to_string();
    state
        .db
        .call(move |conn| {
            let n = conn.execute(
                "UPDATE nodes SET title = ?1 WHERE org_id = ?2 AND session_id = ?3 AND node_id = ?4",
                tokio_rusqlite::params![title, org_id, session_id, node_id],
            )?;
            require_affected_sqlite(n)?;
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description, strategy \
                 FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![org_id, session_id, node_id])?;
            if let Some(row) = rows.next()? {
                Ok(graph_node_mapper(row)?)
            } else {
                Err(tokio_rusqlite::Error::Rusqlite(
                    rusqlite::Error::QueryReturnedNoRows,
                ))
            }
        })
        .await
        .map_not_found()
}

pub async fn walk_to_base(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    node_id: &str,
) -> Result<Vec<WalkNode>, AppError> {
    let org_id = org_id.to_string();
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let walk: Vec<WalkNode> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "WITH RECURSIVE walk(node_id, parent_node_id, tree_sha, title, depth) AS ( \
                   SELECT node_id, parent_node_id, tree_sha, title, 0 \
                   FROM nodes WHERE org_id = ?1 AND session_id = ?2 AND node_id = ?3 \
                   UNION ALL \
                   SELECT n.node_id, n.parent_node_id, n.tree_sha, n.title, walk.depth + 1 \
                   FROM nodes n JOIN walk ON n.node_id = walk.parent_node_id \
                   WHERE n.org_id = ?1 AND n.session_id = ?2 \
                 ) \
                 SELECT tree_sha, title FROM walk ORDER BY depth DESC",
            )?;
            let rows = stmt
                .query_map(
                    tokio_rusqlite::params![org_id, session_id, node_id],
                    |row| {
                        Ok(WalkNode {
                            tree_sha: row.get(0)?,
                            title: row.get(1)?,
                        })
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(walk)
}

pub async fn session_for_node_id(
    state: &AppState,
    org_id: &str,
    node_id: &str,
) -> Result<String, AppError> {
    let org_id = org_id.to_string();
    let node_id = node_id.to_string();
    let session_ids: Vec<String> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT session_id FROM nodes WHERE org_id = ?1 AND node_id = ?2 LIMIT 2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![org_id, node_id])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            Ok(out)
        })
        .await
        .map_err(AppError::from)?;

    match session_ids.len() {
        0 => Err(AppError::NotFound),
        1 => Ok(session_ids.into_iter().next().unwrap()),
        _ => Err(AppError::Conflict {
            code: "ambiguous_node_id".into(),
            message: "node id matches more than one session".into(),
        }),
    }
}

fn graph_node_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphNode> {
    Ok(GraphNode {
        node_id: row.get(0)?,
        parent_node_id: row.get(1)?,
        tree_sha: row.get(2)?,
        commit_sha: row.get(3)?,
        title: row.get(4)?,
        description: row.get(5)?,
        strategy: row
            .get::<_, Option<String>>(6)?
            .and_then(|s| PartitionStrategy::parse(&s)),
    })
}

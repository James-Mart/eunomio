use crate::{error::AppError, state::AppState, types::*};

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
    session_id: &str,
) -> Result<Vec<GraphNode>, AppError> {
    let session_id = session_id.to_string();
    let rows: Vec<GraphNode> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description, strategy \
                 FROM nodes WHERE session_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![session_id], graph_node_mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

pub async fn get(
    state: &AppState,
    session_id: &str,
    node_id: &str,
) -> Result<Option<GraphNode>, AppError> {
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let row: Option<GraphNode> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description, strategy \
                 FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, node_id])?;
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
    session_id: &str,
    target_node_id: &str,
) -> Result<(NodeBasic, Option<NodeBasic>), AppError> {
    let session_id = session_id.to_string();
    let target_node_id = target_node_id.to_string();
    let result: Option<(NodeBasic, Option<NodeBasic>)> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, target_node_id])?;
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
                    "SELECT node_id, tree_sha, commit_sha FROM nodes WHERE session_id = ?1 AND node_id = ?2",
                )?;
                let mut prows = pstmt.query(tokio_rusqlite::params![session_id, pid])?;
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
    session_id: &str,
    target_node_id: &str,
) -> Result<Option<(String, Option<String>, Option<String>)>, AppError> {
    let session_id = session_id.to_string();
    let target_node_id = target_node_id.to_string();
    let row: Option<(String, Option<String>, Option<String>)> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT tree_sha, parent_node_id FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, target_node_id])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let target_tree: String = row.get(0)?;
            let parent_node_id: Option<String> = row.get(1)?;
            let parent_tree = match &parent_node_id {
                Some(pid) => {
                    let mut pstmt = conn.prepare(
                        "SELECT tree_sha FROM nodes WHERE session_id = ?1 AND node_id = ?2",
                    )?;
                    let mut prows = pstmt.query(tokio_rusqlite::params![session_id, pid])?;
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
    session_id: &str,
    node_id: &str,
    title: &str,
) -> Result<Option<GraphNode>, AppError> {
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let title = title.to_string();
    let updated: Option<GraphNode> = state
        .db
        .call(move |conn| {
            let updated = conn.execute(
                "UPDATE nodes SET title = ?1 WHERE session_id = ?2 AND node_id = ?3",
                tokio_rusqlite::params![title, session_id, node_id],
            )?;
            if updated == 0 {
                return Ok(None);
            }
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha, title, description, strategy \
                 FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, node_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(graph_node_mapper(row)?))
            } else {
                Ok(None)
            }
        })
        .await?;
    Ok(updated)
}

pub async fn walk_to_base(
    state: &AppState,
    session_id: &str,
    node_id: &str,
) -> Result<Vec<WalkNode>, AppError> {
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let walk: Vec<WalkNode> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "WITH RECURSIVE walk(node_id, parent_node_id, tree_sha, title, depth) AS ( \
                   SELECT node_id, parent_node_id, tree_sha, title, 0 \
                   FROM nodes WHERE session_id = ?1 AND node_id = ?2 \
                   UNION ALL \
                   SELECT n.node_id, n.parent_node_id, n.tree_sha, n.title, walk.depth + 1 \
                   FROM nodes n JOIN walk ON n.node_id = walk.parent_node_id \
                   WHERE n.session_id = ?1 \
                 ) \
                 SELECT tree_sha, title FROM walk ORDER BY depth DESC",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![session_id, node_id], |row| {
                    Ok(WalkNode {
                        tree_sha: row.get(0)?,
                        title: row.get(1)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(walk)
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

use crate::{error::AppError, git, server::AppState};

struct WalkNode {
    tree_sha: String,
    title: String,
}

pub async fn branch_from_node(
    state: &AppState,
    session_id: &str,
    node_id: &str,
    branch_name: &str,
    force: bool,
) -> Result<String, AppError> {
    if branch_name.is_empty() {
        return Err(AppError::BadRequest("branchName is required".into()));
    }

    ensure_session_in_repo(state, session_id).await?;

    let walk = walk_to_base(state, session_id, node_id).await?;
    if walk.is_empty() {
        return Err(AppError::NotFound);
    }

    if !force && git::branch_exists(&state.repo_root, branch_name).await? {
        return Err(AppError::Conflict(format!(
            "branch {branch_name} already exists"
        )));
    }

    let mut parent: Option<String> = None;
    for node in &walk {
        let parents: Vec<&str> = match &parent {
            Some(p) => vec![p.as_str()],
            None => vec![],
        };
        let commit = git::commit_tree(&state.repo_root, &node.tree_sha, &parents, &node.title)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree replay: {e}")))?;
        parent = Some(commit);
    }

    let tip = parent.expect("walk is non-empty; tip is always set");
    git::branch_create(&state.repo_root, branch_name, &tip, force)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("branch create: {e}")))?;

    Ok(tip)
}

async fn ensure_session_in_repo(state: &AppState, session_id: &str) -> Result<(), AppError> {
    let session_id = session_id.to_string();
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let exists = state
        .db
        .call(move |conn| {
            let mut stmt =
                conn.prepare("SELECT 1 FROM sessions WHERE id = ?1 AND repo_root = ?2")?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_id, repo_root])?;
            Ok(rows.next()?.is_some())
        })
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn walk_to_base(
    state: &AppState,
    session_id: &str,
    node_id: &str,
) -> Result<Vec<WalkNode>, AppError> {
    let session_id = session_id.to_string();
    let node_id = node_id.to_string();
    let walk: Vec<(String, String)> = state
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
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    Ok(walk
        .into_iter()
        .map(|(tree_sha, title)| WalkNode { tree_sha, title })
        .collect())
}

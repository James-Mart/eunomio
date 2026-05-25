// SPDX-License-Identifier: Apache-2.0

use crate::{git, state::AppState, AppError};

pub async fn branch_from_node(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    node_id: &str,
    branch_name: &str,
    force: bool,
) -> Result<String, AppError> {
    if branch_name.is_empty() {
        return Err(AppError::BadRequest("branchName is required".into()));
    }

    state
        .datastore
        .sessions()
        .ensure(org_id, session_id)
        .await?;
    let fields = state
        .datastore
        .sessions()
        .repo_fields(org_id, session_id)
        .await?;
    if !fields.is_local {
        return Err(AppError::BadRequest(
            "branch creation is only supported for local repository sessions".into(),
        ));
    }
    let git_root = crate::repo_store::session_git_root(state, org_id, session_id).await?;

    let walk = state
        .datastore
        .nodes()
        .walk_to_base(org_id, session_id, node_id)
        .await?;
    if walk.is_empty() {
        return Err(AppError::NotFound);
    }

    if !force && git::branch_exists(&git_root, branch_name).await? {
        return Err(AppError::Conflict {
            code: "branch_exists".into(),
            message: format!("branch {branch_name} already exists"),
        });
    }

    let mut parent: Option<String> = None;
    for node in &walk {
        let parents: Vec<&str> = match &parent {
            Some(p) => vec![p.as_str()],
            None => vec![],
        };
        let commit = git::commit_tree(&git_root, &node.tree_sha, &parents, &node.title)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("commit-tree replay: {e}")))?;
        parent = Some(commit);
    }

    let tip = parent.expect("walk is non-empty; tip is always set");
    git::branch_create(&git_root, branch_name, &tip, force)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("branch create: {e}")))?;

    Ok(tip)
}

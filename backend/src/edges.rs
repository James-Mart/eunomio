use crate::{error::AppError, git, repo, state::AppState, synthesized_content, types::*};
use std::path::Path;

pub async fn render_edge_diff(
    repo_root: &Path,
    parent_tree: &str,
    child_tree: &str,
    before_ref: &str,
    after_ref: &str,
) -> Result<(String, Vec<git::FileBlob>, synthesized_content::SynthesizedRanges), AppError> {
    let diff = git::diff_text(repo_root, parent_tree, child_tree).await?;
    let files = git::changed_files(repo_root, parent_tree, child_tree).await?;
    let synthesized = synthesized_content::compute(
        repo_root,
        parent_tree,
        child_tree,
        before_ref,
        after_ref,
    )
    .await?;
    Ok((diff, files, synthesized))
}

pub async fn load_edge_for_target(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    target_node_id: String,
) -> Result<Edge, AppError> {
    let Some((base_tree, final_tree)) =
        repo::session::seed_trees(state, org_id, session_id).await?
    else {
        return Err(AppError::NotFound);
    };
    let lookup =
        repo::node::target_tree_and_parent(state, org_id, session_id, &target_node_id).await?;
    let Some((target_tree, parent_node_id, parent_tree)) = lookup else {
        return Err(AppError::NotFound);
    };
    let (diff, files, synthesized) = match (&parent_node_id, &parent_tree) {
        (Some(_), Some(parent_tree)) => {
            let git_root = repo::session::git_root(state, org_id, session_id).await?;
            render_edge_diff(
                &git_root,
                parent_tree,
                &target_tree,
                &base_tree,
                &final_tree,
            )
            .await?
        }
        _ => (String::new(), Vec::new(), Default::default()),
    };
    Ok(Edge {
        target_node_id,
        parent_node_id,
        diff,
        files,
        synthesized,
    })
}

use crate::{error::AppError, git, repo, state::AppState, synthesized_content, types::*};
use std::path::Path;

/// Returns `(diff_text, synthesized_ranges)` for the edge between two
/// trees, using `reference_tree` as the destination tree the synthesized
/// computation rebases against. Shared between `get_edge` and `get_diff`.
pub async fn render_edge_diff(
    repo_root: &Path,
    parent_tree: &str,
    child_tree: &str,
    reference_tree: &str,
) -> Result<(String, synthesized_content::SynthesizedRanges), AppError> {
    let diff = git::diff_text(repo_root, parent_tree, child_tree).await?;
    let synthesized =
        synthesized_content::compute(repo_root, parent_tree, child_tree, reference_tree).await?;
    Ok((diff, synthesized))
}

/// Loads the edge for `target_node_id` within `session_id`: looks up the
/// node and its parent, renders the edge diff against the session's final
/// tree, and returns the assembled DTO. Used by the `GET /edges/:id` route.
pub async fn load_edge_for_target(
    state: &AppState,
    session_id: &str,
    target_node_id: String,
) -> Result<Edge, AppError> {
    let final_tree = repo::session::final_tree(state, session_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let lookup = repo::node::target_tree_and_parent(state, session_id, &target_node_id).await?;
    let Some((target_tree, parent_node_id, parent_tree)) = lookup else {
        return Err(AppError::NotFound);
    };
    let (diff, synthesized) = match (&parent_node_id, &parent_tree) {
        (Some(_), Some(parent_tree)) => {
            render_edge_diff(&state.repo_root, parent_tree, &target_tree, &final_tree).await?
        }
        _ => (String::new(), Default::default()),
    };
    Ok(Edge {
        target_node_id,
        parent_node_id,
        diff,
        synthesized,
    })
}

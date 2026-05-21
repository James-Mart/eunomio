use crate::{error::AppError, state::AppState};

pub async fn trees_known_in_session(
    state: &AppState,
    session_id: &str,
    trees: &[&str],
) -> Result<bool, AppError> {
    let session_id = session_id.to_string();
    let trees: Vec<String> = trees.iter().map(|s| s.to_string()).collect();
    let known: bool = state
        .db
        .call(move |conn| {
            for tree in &trees {
                let mut found = false;
                let mut nodes_stmt = conn.prepare(
                    "SELECT 1 FROM nodes WHERE session_id = ?1 AND tree_sha = ?2 LIMIT 1",
                )?;
                if nodes_stmt
                    .query(tokio_rusqlite::params![session_id, tree])?
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
                        .query(tokio_rusqlite::params![session_id, tree])?
                        .next()?
                        .is_some()
                    {
                        found = true;
                    }
                }
                if !found {
                    return Ok(false);
                }
            }
            Ok(true)
        })
        .await?;
    Ok(known)
}

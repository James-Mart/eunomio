// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use eunomio_core::{traits::DiffAuthorizationRepo, types::ShavingStep, AppError};
use std::sync::Arc;
use tokio_rusqlite::Connection;

pub struct SqliteDiffAuthorizationRepo {
    conn: Arc<Connection>,
}

impl SqliteDiffAuthorizationRepo {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl DiffAuthorizationRepo for SqliteDiffAuthorizationRepo {
    async fn trees_authorized_for_diff(
        &self,
        org_id: &str,
        session_id: &str,
        trees: &[&str],
    ) -> Result<bool, AppError> {
        let org_id = org_id.to_string();
        let session_id = session_id.to_string();
        let trees: Vec<String> = trees.iter().map(|s| s.to_string()).collect();
        self.conn
            .call(move |conn| {
                for tree in &trees {
                    let mut found = false;
                    let mut nodes_stmt = conn.prepare(
                        "SELECT 1 FROM nodes \
                         WHERE org_id = ?1 AND session_id = ?2 AND tree_sha = ?3 LIMIT 1",
                    )?;
                    if nodes_stmt
                        .query(rusqlite::params![org_id, session_id, tree])?
                        .next()?
                        .is_some()
                    {
                        found = true;
                    }
                    if !found {
                        let mut partitions_stmt = conn.prepare(
                            "SELECT 1 FROM partitions \
                             WHERE org_id = ?1 AND session_id = ?2 AND candidate_slice_tree_sha = ?3 LIMIT 1",
                        )?;
                        if partitions_stmt
                            .query(rusqlite::params![org_id, session_id, tree])?
                            .next()?
                            .is_some()
                        {
                            found = true;
                        }
                    }
                    if !found {
                        let mut shaving_stmt = conn.prepare(
                            "SELECT parent_tree_sha, head_tree_sha, steps_json \
                             FROM shaving_tracks WHERE org_id = ?1 AND session_id = ?2",
                        )?;
                        let mut rows =
                            shaving_stmt.query(rusqlite::params![org_id, session_id])?;
                        while let Some(row) = rows.next()? {
                            let parent_tree_sha: String = row.get(0)?;
                            let head_tree_sha: String = row.get(1)?;
                            if &parent_tree_sha == tree || &head_tree_sha == tree {
                                found = true;
                                break;
                            }
                            let steps_json: String = row.get(2)?;
                            let steps: Vec<ShavingStep> =
                                serde_json::from_str(&steps_json).map_err(|e| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        2,
                                        rusqlite::types::Type::Text,
                                        Box::new(e),
                                    )
                                })?;
                            if steps.iter().any(|step| &step.tree_sha == tree) {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        return Ok(false);
                    }
                }
                Ok(true)
            })
            .await
            .map_err(crate::repo::map_sqlite_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eunomio_core::traits::DiffAuthorizationRepo;

    #[tokio::test]
    async fn shaving_step_trees_are_session_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Arc::new(crate::db::open(&dir.path().join("test.db")).await.unwrap());
        conn.call(|conn| {
            conn.execute(
                "INSERT INTO orgs (id, display_name, created_at) VALUES ('org', 'Org', 1)",
                [],
            )?;
            conn.execute(
                "INSERT INTO users (id, username, created_at) VALUES ('user', 'user', 1)",
                [],
            )?;
            for session_id in ["session-a", "session-b"] {
                conn.execute(
                    "INSERT INTO sessions (
                      id, org_id, user_id, normalized_remote, literal_remote,
                      base_ref, source_ref, resolved_base_commit, resolved_source_commit,
                      base_tree, final_tree, base_node_id, created_at
                    ) VALUES (?1, 'org', 'user', ?2, ?2, 'main', 'feature', ?5, ?6, ?3, ?4, 'base', 1)",
                    rusqlite::params![
                        session_id,
                        format!("remote-{session_id}"),
                        format!("base-{session_id}"),
                        format!("head-{session_id}"),
                        format!("commit-base-{session_id}"),
                        format!("commit-head-{session_id}"),
                    ],
                )?;
                conn.execute(
                    "INSERT INTO nodes (
                      session_id, node_id, org_id, parent_node_id, tree_sha, commit_sha,
                      title, description, strategy, created_at
                    ) VALUES (?1, 'slice', 'org', 'base', ?2, 'commit', 'Slice', '', NULL, 1)",
                    rusqlite::params![session_id, format!("head-{session_id}")],
                )?;
            }
            conn.execute(
                "INSERT INTO shaving_tracks (
                  session_id, target_node_id, org_id, parent_tree_sha, head_tree_sha,
                  steps_json, ref_name, created_at
                ) VALUES ('session-a', 'slice', 'org', 'base-session-a', 'head-session-a', ?1, 'ref-a', 1)",
                [serde_json::to_string(&vec![ShavingStep {
                    tree_sha: "step-session-a".to_string(),
                    commit_sha: "commit-a".to_string(),
                    label: None,
                }])
                .unwrap()],
            )?;
            conn.execute(
                "INSERT INTO shaving_tracks (
                  session_id, target_node_id, org_id, parent_tree_sha, head_tree_sha,
                  steps_json, ref_name, created_at
                ) VALUES ('session-b', 'slice', 'org', 'base-session-b', 'head-session-b', ?1, 'ref-b', 1)",
                [serde_json::to_string(&vec![ShavingStep {
                    tree_sha: "step-session-b".to_string(),
                    commit_sha: "commit-b".to_string(),
                    label: None,
                }])
                .unwrap()],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        let repo = SqliteDiffAuthorizationRepo::new(conn);
        assert!(repo
            .trees_authorized_for_diff("org", "session-a", &["step-session-a"])
            .await
            .unwrap());
        assert!(!repo
            .trees_authorized_for_diff("org", "session-a", &["step-session-b"])
            .await
            .unwrap());
    }
}

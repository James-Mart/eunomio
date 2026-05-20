use anyhow::{Context, Result};
use std::path::Path;
use tokio_rusqlite::Connection;

const MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  repo_root TEXT NOT NULL,
  base_ref TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  base_tree TEXT NOT NULL,
  final_tree TEXT NOT NULL,
  base_node_id TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_by_repo ON sessions (repo_root);
CREATE UNIQUE INDEX IF NOT EXISTS sessions_unique_pair
  ON sessions (repo_root, base_ref, source_ref);

CREATE TABLE IF NOT EXISTS nodes (
  session_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  parent_node_id TEXT,
  tree_sha TEXT NOT NULL,
  commit_sha TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, node_id)
);

CREATE TABLE IF NOT EXISTS partitions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  target_node_id TEXT NOT NULL,
  strategy TEXT,
  change_survey_json TEXT,
  plan_json TEXT,
  candidate_slice_tree_sha TEXT,
  candidate_slice_commit_sha TEXT,
  phase TEXT NOT NULL,
  phase_state TEXT NOT NULL,
  worktree_path TEXT NOT NULL,
  remaining_depth INTEGER,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS partitions_by_edge ON partitions (session_id, target_node_id);

CREATE TABLE IF NOT EXISTS runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  partition_id INTEGER NOT NULL,
  session_id TEXT NOT NULL,
  target_node_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  parent_run_id INTEGER,
  status TEXT NOT NULL,
  result_json TEXT,
  result_text TEXT,
  error_message TEXT,
  started_at INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE INDEX IF NOT EXISTS runs_by_edge ON runs (session_id, target_node_id);
CREATE INDEX IF NOT EXISTS runs_by_partition ON runs (partition_id);
"#;

pub async fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)
        .await
        .with_context(|| format!("opening sqlite db at {}", db_path.display()))?;
    conn.call(|conn| {
        conn.execute_batch(MIGRATION)?;
        Ok(())
    })
    .await
    .context("running embedded migration")?;
    Ok(conn)
}

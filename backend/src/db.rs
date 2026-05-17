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
  worktree_path TEXT NOT NULL,
  base_node_id TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_by_repo ON sessions (repo_root);
CREATE TABLE IF NOT EXISTS nodes (
  session_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  parent_node_id TEXT,
  tree_sha TEXT NOT NULL,
  commit_sha TEXT NOT NULL,
  title TEXT NOT NULL,
  is_favorite INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, node_id)
);
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

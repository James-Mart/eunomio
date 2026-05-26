// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_rusqlite::Connection;

const MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS orgs (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS org_memberships (
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  role TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (org_id, user_id)
);

CREATE TABLE IF NOT EXISTS auth_sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  org_id TEXT NOT NULL REFERENCES orgs(id),
  created_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  ip TEXT NOT NULL,
  user_agent TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS auth_sessions_by_user ON auth_sessions (user_id);

CREATE TABLE IF NOT EXISTS auth_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  org_id TEXT,
  user_id TEXT,
  event_type TEXT NOT NULL,
  ip TEXT NOT NULL,
  user_agent TEXT NOT NULL,
  details_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS auth_events_by_org ON auth_events (org_id, created_at);

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  normalized_remote TEXT NOT NULL,
  literal_remote TEXT NOT NULL,
  is_local INTEGER NOT NULL,
  base_ref TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  base_tree TEXT NOT NULL,
  final_tree TEXT NOT NULL,
  base_node_id TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_by_remote ON sessions (normalized_remote);
CREATE INDEX IF NOT EXISTS sessions_by_org ON sessions (org_id);
CREATE INDEX IF NOT EXISTS sessions_by_user ON sessions (user_id);
CREATE UNIQUE INDEX IF NOT EXISTS sessions_unique_pair
  ON sessions (normalized_remote, base_ref, source_ref);

CREATE TABLE IF NOT EXISTS nodes (
  session_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  org_id TEXT NOT NULL REFERENCES orgs(id),
  parent_node_id TEXT,
  tree_sha TEXT NOT NULL,
  commit_sha TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  strategy TEXT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, node_id)
);
CREATE INDEX IF NOT EXISTS nodes_by_node_id ON nodes (node_id);
CREATE INDEX IF NOT EXISTS nodes_by_org ON nodes (org_id);

CREATE TABLE IF NOT EXISTS partitions (
  id TEXT PRIMARY KEY,
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
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
CREATE INDEX IF NOT EXISTS partitions_by_org ON partitions (org_id);

CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  partition_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  target_node_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  parent_run_id TEXT,
  status TEXT NOT NULL,
  result_json TEXT,
  result_text TEXT,
  error_message TEXT,
  prompt_text TEXT,
  transcript_text TEXT,
  started_at INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE INDEX IF NOT EXISTS runs_by_edge ON runs (session_id, target_node_id);
CREATE INDEX IF NOT EXISTS runs_by_partition ON runs (partition_id);
CREATE INDEX IF NOT EXISTS runs_by_org ON runs (org_id);

CREATE TABLE IF NOT EXISTS shaver_runs (
  id TEXT PRIMARY KEY,
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  session_id TEXT NOT NULL,
  target_node_id TEXT NOT NULL,
  worktree_path TEXT NOT NULL,
  status TEXT NOT NULL,
  result_json TEXT,
  result_text TEXT,
  error_message TEXT,
  prompt_text TEXT,
  transcript_text TEXT,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  FOREIGN KEY (session_id, target_node_id) REFERENCES nodes(session_id, node_id)
);
CREATE INDEX IF NOT EXISTS shaver_runs_by_edge ON shaver_runs (session_id, target_node_id);
CREATE INDEX IF NOT EXISTS shaver_runs_by_org ON shaver_runs (org_id);

CREATE TABLE IF NOT EXISTS edge_file_viewed (
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  session_id TEXT NOT NULL,
  target_node_id TEXT NOT NULL,
  file_path TEXT NOT NULL,
  viewed_at INTEGER NOT NULL,
  PRIMARY KEY (org_id, user_id, session_id, target_node_id, file_path),
  FOREIGN KEY (session_id, target_node_id) REFERENCES nodes(session_id, node_id)
);
CREATE INDEX IF NOT EXISTS edge_file_viewed_by_session
  ON edge_file_viewed (session_id, org_id);

CREATE TABLE IF NOT EXISTS node_reviewed (
  org_id TEXT NOT NULL REFERENCES orgs(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  session_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  reviewed_at INTEGER NOT NULL,
  PRIMARY KEY (org_id, user_id, session_id, node_id),
  FOREIGN KEY (session_id, node_id) REFERENCES nodes(session_id, node_id)
);
CREATE INDEX IF NOT EXISTS node_reviewed_by_session
  ON node_reviewed (session_id, org_id);

CREATE TABLE IF NOT EXISTS shaving_tracks (
  session_id TEXT NOT NULL,
  target_node_id TEXT NOT NULL,
  org_id TEXT NOT NULL REFERENCES orgs(id),
  parent_tree_sha TEXT NOT NULL,
  head_tree_sha TEXT NOT NULL,
  steps_json TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, target_node_id),
  FOREIGN KEY (session_id, target_node_id) REFERENCES nodes(session_id, node_id)
);
CREATE INDEX IF NOT EXISTS shaving_tracks_by_org_session
  ON shaving_tracks (org_id, session_id);
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

pub fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

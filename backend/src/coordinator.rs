use crate::{
    cursor_bridge::{HelperEvent, RunHandle, RunRequest, SubagentRunner},
    error::AppError,
    git,
    partition_settings::resolve_model,
    server::AppState,
    subagents::{
        self,
        constructor::ConstructOutput,
        planner::{PlanOutput, PlanStrategy, PriorAttempt},
        surveyor::SurveyOutput,
        Subagents,
    },
    types::*,
};
use anyhow::anyhow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;

const BROADCAST_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct Coordinator {
    inner: Arc<Inner>,
}

struct Inner {
    channels: StdMutex<HashMap<String, broadcast::Sender<SseEvent>>>,
    handles: Mutex<HashMap<i64, RunHandle>>,
    abandoning: StdMutex<std::collections::HashSet<i64>>,
    subagents: Subagents,
    runner: Arc<dyn SubagentRunner>,
}

impl Coordinator {
    pub fn new(subagents: Subagents, runner: Arc<dyn SubagentRunner>) -> Self {
        Self {
            inner: Arc::new(Inner {
                channels: StdMutex::new(HashMap::new()),
                handles: Mutex::new(HashMap::new()),
                abandoning: StdMutex::new(std::collections::HashSet::new()),
                subagents,
                runner,
            }),
        }
    }

    pub fn subscribe(&self, session_id: &str) -> broadcast::Receiver<SseEvent> {
        let mut channels = self.inner.channels.lock().unwrap();
        channels
            .entry(session_id.to_string())
            .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
            .subscribe()
    }

    fn emit(&self, session_id: &str, event: SseEvent) {
        let tx = {
            let mut channels = self.inner.channels.lock().unwrap();
            channels
                .entry(session_id.to_string())
                .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
                .clone()
        };
        let _ = tx.send(event);
    }

    pub async fn process_startup_recovery(&self, state: &AppState) -> Result<(), AppError> {
        let stuck_runs: Vec<i64> = state
            .db
            .call(|conn| {
                let mut stmt = conn.prepare("SELECT id FROM runs WHERE status = 'running'")?;
                let rows = stmt
                    .query_map([], |row| row.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        if !stuck_runs.is_empty() {
            let now = unix_seconds();
            state
                .db
                .call(move |conn| {
                    let tx = conn.transaction()?;
                    for id in &stuck_runs {
                        tx.execute(
                            "UPDATE runs SET status = 'error', error_message = 'process_restart', finished_at = ?1 WHERE id = ?2",
                            tokio_rusqlite::params![now, id],
                        )?;
                    }
                    tx.commit()?;
                    Ok(())
                })
                .await?;
        }

        let partition_rows: Vec<(i64, String, String)> = state
            .db
            .call(|conn| {
                let mut stmt =
                    conn.prepare("SELECT id, session_id, worktree_path FROM partitions")?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        let mut alive_partition_ids: Vec<i64> = Vec::new();
        for (id, session_id, worktree_path) in partition_rows {
            let exists = std::path::Path::new(&worktree_path).exists();
            if !exists {
                tracing::warn!(partition_id = id, worktree = %worktree_path, "partition worktree missing on disk; deleting row");
                state
                    .db
                    .call(move |conn| {
                        let tx = conn.transaction()?;
                        tx.execute(
                            "DELETE FROM runs WHERE partition_id = ?1",
                            tokio_rusqlite::params![id],
                        )?;
                        tx.execute(
                            "DELETE FROM partitions WHERE id = ?1",
                            tokio_rusqlite::params![id],
                        )?;
                        tx.commit()?;
                        Ok(())
                    })
                    .await?;
                continue;
            }
            alive_partition_ids.push(id);
            let _ = session_id;
        }

        let worktrees_root = state.data_dir.join("worktrees");
        if worktrees_root.exists() {
            let alive_ids = alive_partition_ids.clone();
            let mut sessions_iter = match tokio::fs::read_dir(&worktrees_root).await {
                Ok(it) => it,
                Err(e) => {
                    tracing::warn!(error = %e, "reading worktrees root failed");
                    return Ok(());
                }
            };
            while let Ok(Some(session_entry)) = sessions_iter.next_entry().await {
                let session_path = session_entry.path();
                if !session_path.is_dir() {
                    continue;
                }
                let mut parts_iter = match tokio::fs::read_dir(&session_path).await {
                    Ok(it) => it,
                    Err(_) => continue,
                };
                while let Ok(Some(part_entry)) = parts_iter.next_entry().await {
                    let part_path = part_entry.path();
                    if !part_path.is_dir() {
                        continue;
                    }
                    let name = part_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    let pid_opt = name.parse::<i64>().ok();
                    let synthesis = part_path.join("synthesis");
                    if !synthesis.exists() {
                        continue;
                    }
                    let alive = pid_opt.map(|p| alive_ids.contains(&p)).unwrap_or(false);
                    if alive {
                        continue;
                    }
                    tracing::info!(path = %synthesis.display(), "removing orphan partition worktree");
                    if let Err(e) = git::worktree_remove(&state.repo_root, &synthesis).await {
                        tracing::warn!(error = %e, "removing orphan worktree failed");
                    }
                    let _ = tokio::fs::remove_dir_all(&part_path).await;
                }
            }
        }

        Ok(())
    }

    pub async fn begin_partition(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<Partition, AppError> {
        let settings = state.partition_settings.snapshot().await;
        let remaining_depth = match settings.coordinator.max_iterations {
            IterationLimit::Count { count } => Some(count as i64),
            IterationLimit::Auto => None,
        };
        self.begin_partition_internal(state, session_id, target_node_id, remaining_depth)
            .await
    }

    async fn begin_child_partition(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: &str,
        parent_remaining_depth: Option<i64>,
    ) -> Result<Partition, AppError> {
        let remaining_depth = parent_remaining_depth.map(|n| (n - 1).max(0));
        self.begin_partition_internal(state, session_id, target_node_id, remaining_depth)
            .await
    }

    async fn begin_partition_internal(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: &str,
        remaining_depth: Option<i64>,
    ) -> Result<Partition, AppError> {
        let (_, parent_node) = fetch_target_and_parent(state, session_id, target_node_id).await?;
        let parent = parent_node
            .ok_or_else(|| AppError::BadRequest("base node has no incoming edge to partition".into()))?;

        let worktree_root = state.data_dir.join("worktrees").join(session_id);
        tokio::fs::create_dir_all(&worktree_root)
            .await
            .map_err(|e| AppError::Internal(anyhow!("creating worktree root: {e}")))?;

        let session_id_owned = session_id.to_string();
        let target_owned = target_node_id.to_string();
        let now = unix_seconds();
        let depth_for_insert = remaining_depth;
        let inserted_id: i64 = state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO partitions (session_id, target_node_id, phase, phase_state, worktree_path, remaining_depth, created_at) \
                     VALUES (?1, ?2, 'survey', 'running', '', ?3, ?4)",
                    tokio_rusqlite::params![session_id_owned, target_owned, depth_for_insert, now],
                )?;
                let id = tx.last_insert_rowid();
                tx.commit()?;
                Ok(id)
            })
            .await?;

        let worktree_path = worktree_root
            .join(inserted_id.to_string())
            .join("synthesis");
        if let Some(parent_dir) = worktree_path.parent() {
            tokio::fs::create_dir_all(parent_dir)
                .await
                .map_err(|e| AppError::Internal(anyhow!("create worktree parent: {e}")))?;
        }

        if let Err(e) = git::worktree_add(&state.repo_root, &worktree_path, &parent.commit_sha).await {
            let _ = state
                .db
                .call(move |conn| {
                    conn.execute(
                        "DELETE FROM partitions WHERE id = ?1",
                        tokio_rusqlite::params![inserted_id],
                    )?;
                    Ok(())
                })
                .await;
            return Err(AppError::Internal(anyhow!("worktree add: {e}")));
        }

        let wt_path_str = worktree_path.to_string_lossy().to_string();
        let wt_path_for_update = wt_path_str.clone();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE partitions SET worktree_path = ?1 WHERE id = ?2",
                    tokio_rusqlite::params![wt_path_for_update, inserted_id],
                )?;
                Ok(())
            })
            .await?;

        self.emit(
            session_id,
            SseEvent::Started {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id: inserted_id,
            },
        );

        let row = load_partition_row(state, inserted_id).await?;
        let partition: Partition = row.into();

        self.spawn_run_boxed(state.clone(), inserted_id, RunKind::Survey, None, None, None)
            .await?;

        Ok(partition)
    }

    pub async fn start_run(
        &self,
        state: &AppState,
        partition_id: i64,
        req: StartRunRequest,
    ) -> Result<Run, AppError> {
        if self.inner.handles.lock().await.contains_key(&partition_id) {
            return Err(AppError::Conflict {
                code: "partition_run_in_flight".into(),
                message: "this partition already has a run in flight".into(),
            });
        }
        let row = load_partition_row(state, partition_id).await?;
        if !matches!(row.phase_state, PhaseState::AwaitingReview | PhaseState::Error) {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not currently at a review gate or in error state".into(),
            });
        }
        validate_run_kind_transition(row.phase, req.kind)?;
        let kind = req.kind;
        if kind == RunKind::Plan && row.phase == PhaseName::Construct {
            self.reset_for_construct_to_plan_back_edge(state, partition_id, &row).await?;
        }
        self.spawn_run_boxed(
            state.clone(),
            partition_id,
            kind,
            req.parent_run_id,
            req.user_feedback,
            req.strategy_override,
        )
        .await
    }

    async fn reset_for_construct_to_plan_back_edge(
        &self,
        state: &AppState,
        partition_id: i64,
        row: &PartitionRow,
    ) -> Result<(), AppError> {
        let (_, parent_node) =
            fetch_target_and_parent(state, &row.session_id, &row.target_node_id).await?;
        let parent = parent_node
            .ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        if let Err(e) = git::run_in(&worktree_path, &["reset", "--hard", &parent.commit_sha]).await
        {
            tracing::warn!(error = %e, "reset on back-edge failed");
        }
        if let Err(e) = git::run_in(&worktree_path, &["clean", "-fdx"]).await {
            tracing::warn!(error = %e, "clean on back-edge failed");
        }
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE partitions SET plan_json = NULL, strategy = NULL, candidate_slice_tree_sha = NULL, candidate_slice_commit_sha = NULL WHERE id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn cancel_run(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
    ) -> Result<(), AppError> {
        let run = load_run(state, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::NotFound);
        }
        if !matches!(run.status, RunStatus::Running) {
            return Err(AppError::Conflict {
                code: "run_not_running".into(),
                message: "run is not in running state".into(),
            });
        }
        let row = load_partition_row(state, partition_id).await?;
        let handle = self.inner.handles.lock().await.remove(&partition_id);
        if let Some(handle) = handle {
            (handle.cancel)();
        }
        let now = unix_seconds();
        state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
                    tokio_rusqlite::params![now, run_id],
                )?;
                tx.execute(
                    "UPDATE partitions SET phase_state = 'error' WHERE id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        self.emit(
            &row.session_id,
            SseEvent::Cancelled {
                session_id: row.session_id.clone(),
                target_node_id: row.target_node_id.clone(),
                partition_id,
            },
        );
        Ok(())
    }

    fn spawn_run_boxed(
        &self,
        state: AppState,
        partition_id: i64,
        kind: RunKind,
        parent_run_id: Option<i64>,
        user_feedback: Option<String>,
        strategy_override: Option<PartitionStrategy>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Run, AppError>> + Send + '_>> {
        Box::pin(self.spawn_run(state, partition_id, kind, parent_run_id, user_feedback, strategy_override))
    }

    pub async fn list_runs(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<Vec<Run>, AppError> {
        let rows = load_runs_for_partition(state, partition_id).await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn accept_survey(
        &self,
        state: &AppState,
        partition_id: i64,
        req: AcceptSurveyRequest,
    ) -> Result<Partition, AppError> {
        let row = load_partition_row(state, partition_id).await?;
        if !(matches!(row.phase, PhaseName::Survey)
            && matches!(row.phase_state, PhaseState::AwaitingReview))
        {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not at the survey review gate".into(),
            });
        }
        self.do_accept_survey(state, partition_id, req.run_id).await
    }

    async fn do_accept_survey(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
    ) -> Result<Partition, AppError> {
        let run = load_run(state, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::BadRequest("runId does not belong to this partition".into()));
        }
        let result_json = run
            .result_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("survey run has no parsed result".into()))?;
        let _: SurveyOutput = serde_json::from_str(result_json)
            .map_err(|e| AppError::BadRequest(format!("invalid survey result: {e}")))?;
        let result_json_owned = result_json.to_string();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE partitions SET change_survey_json = ?1, phase = 'plan', phase_state = 'running' WHERE id = ?2",
                    tokio_rusqlite::params![result_json_owned, partition_id],
                )?;
                Ok(())
            })
            .await?;
        let new_row = load_partition_row(state, partition_id).await?;
        self.spawn_run_boxed(state.clone(), partition_id, RunKind::Plan, Some(run_id), None, None)
            .await?;
        Ok(new_row.into())
    }

    pub async fn accept_plan(
        &self,
        state: &AppState,
        partition_id: i64,
        req: AcceptPlanRequest,
    ) -> Result<Partition, AppError> {
        let row = load_partition_row(state, partition_id).await?;
        if !(matches!(row.phase, PhaseName::Plan)
            && matches!(row.phase_state, PhaseState::AwaitingReview))
        {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not at the plan review gate".into(),
            });
        }
        self.do_accept_plan(state, partition_id, req.run_id).await
    }

    async fn do_accept_plan(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
    ) -> Result<Partition, AppError> {
        let run = load_run(state, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::BadRequest("runId does not belong to this partition".into()));
        }
        let result_json = run
            .result_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("plan run has no parsed result".into()))?;
        let plan: PlanOutput = serde_json::from_str(result_json)
            .map_err(|e| AppError::BadRequest(format!("invalid plan result: {e}")))?;
        let strategy = match &plan {
            PlanOutput::Split { strategy, .. } => *strategy,
            PlanOutput::Indivisible { .. } => {
                return Err(AppError::BadRequest(
                    "plan is indivisible; cannot accept".into(),
                ))
            }
        };
        let strategy_str = match strategy {
            PlanStrategy::Semantic => "semantic",
            PlanStrategy::Vertical => "vertical",
            PlanStrategy::Horizontal => "horizontal",
        };
        let result_json_owned = result_json.to_string();
        let strategy_owned = strategy_str.to_string();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE partitions SET plan_json = ?1, strategy = ?2, phase = 'construct', phase_state = 'running' WHERE id = ?3",
                    tokio_rusqlite::params![result_json_owned, strategy_owned, partition_id],
                )?;
                Ok(())
            })
            .await?;
        let new_row = load_partition_row(state, partition_id).await?;
        self.spawn_run_boxed(state.clone(), partition_id, RunKind::Construct, Some(run_id), None, None)
            .await?;
        Ok(new_row.into())
    }

    pub async fn accept_construct(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<(), AppError> {
        let row = load_partition_row(state, partition_id).await?;
        if !matches!(row.phase, PhaseName::Construct) {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not at the construct review gate".into(),
            });
        }
        self.do_accept_construct(state, partition_id).await
    }

    async fn do_accept_construct(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<(), AppError> {
        let row = load_partition_row(state, partition_id).await?;
        let candidate_tree = row
            .candidate_slice_tree_sha
            .clone()
            .ok_or_else(|| AppError::BadRequest("no candidate slice to accept".into()))?;
        let candidate_commit = row
            .candidate_slice_commit_sha
            .clone()
            .ok_or_else(|| AppError::BadRequest("no candidate slice to accept".into()))?;
        let plan_json = row
            .plan_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("no plan accepted".into()))?;
        let plan: PlanOutput = serde_json::from_str(plan_json)
            .map_err(|e| AppError::BadRequest(format!("invalid plan result: {e}")))?;
        let edges = match &plan {
            PlanOutput::Split { edges, .. } => edges.clone(),
            PlanOutput::Indivisible { .. } => {
                return Err(AppError::BadRequest(
                    "plan is indivisible; cannot accept".into(),
                ))
            }
        };

        let session_id = row.session_id.clone();
        let target_node_id = row.target_node_id.clone();
        let (_target_node, parent_node) =
            fetch_target_and_parent(state, &session_id, &target_node_id).await?;
        let parent = parent_node.ok_or_else(|| {
            AppError::BadRequest("target has no parent; cannot insert slice".into())
        })?;

        let siblings = load_sibling_partitions(state, &session_id, &target_node_id, partition_id)
            .await?;

        {
            let mut a = self.inner.abandoning.lock().unwrap();
            for s in &siblings {
                a.insert(s.id);
            }
        }
        for s in &siblings {
            let h = self.inner.handles.lock().await.remove(&s.id);
            if let Some(handle) = h {
                (handle.cancel)();
            }
        }

        let slice_node_id = Uuid::new_v4().to_string();
        let now = unix_seconds();
        let slice_title = edges[0].title.clone();
        let leftover_title = edges[1].title.clone();
        let remaining_depth = row.remaining_depth;

        let session_id_db = session_id.clone();
        let target_id_db = target_node_id.clone();
        let parent_node_id_db = parent.node_id.clone();
        let slice_id_db = slice_node_id.clone();
        let slice_title_db = slice_title.clone();
        let leftover_title_db = leftover_title.clone();
        let candidate_tree_db = candidate_tree.clone();
        let candidate_commit_db = candidate_commit.clone();
        let sibling_ids: Vec<i64> = siblings.iter().map(|s| s.id).collect();
        state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO nodes (session_id, node_id, parent_node_id, tree_sha, commit_sha, title, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    tokio_rusqlite::params![
                        session_id_db,
                        slice_id_db,
                        parent_node_id_db,
                        candidate_tree_db,
                        candidate_commit_db,
                        slice_title_db,
                        now
                    ],
                )?;
                tx.execute(
                    "UPDATE nodes SET parent_node_id = ?1, title = ?2 WHERE session_id = ?3 AND node_id = ?4",
                    tokio_rusqlite::params![slice_id_db, leftover_title_db, session_id_db, target_id_db],
                )?;
                let mut all_ids = sibling_ids.clone();
                all_ids.push(partition_id);
                for id in &all_ids {
                    tx.execute(
                        "DELETE FROM runs WHERE partition_id = ?1",
                        tokio_rusqlite::params![id],
                    )?;
                    tx.execute(
                        "DELETE FROM partitions WHERE id = ?1",
                        tokio_rusqlite::params![id],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await?;

        let worktree_path = PathBuf::from(&row.worktree_path);
        if worktree_path.exists() {
            if let Err(e) = git::worktree_remove(&state.repo_root, &worktree_path).await {
                tracing::warn!(error = %e, "removing partition worktree after accept failed");
            }
        }
        if let Some(parent_dir) = worktree_path.parent() {
            let _ = tokio::fs::remove_dir_all(parent_dir).await;
        }
        for sib in &siblings {
            let sib_path = PathBuf::from(&sib.worktree_path);
            if sib_path.exists() {
                if let Err(e) = git::worktree_remove(&state.repo_root, &sib_path).await {
                    tracing::warn!(error = %e, sibling_partition_id = sib.id, "removing sibling worktree failed");
                }
            }
            if let Some(parent_dir) = sib_path.parent() {
                let _ = tokio::fs::remove_dir_all(parent_dir).await;
            }
        }

        self.emit(
            &session_id,
            SseEvent::Finished {
                session_id: session_id.clone(),
                target_node_id: target_node_id.clone(),
                partition_id,
            },
        );
        for sib in &siblings {
            self.emit(
                &session_id,
                SseEvent::Cancelled {
                    session_id: session_id.clone(),
                    target_node_id: sib.target_node_id.clone(),
                    partition_id: sib.id,
                },
            );
        }
        {
            let mut a = self.inner.abandoning.lock().unwrap();
            for s in &siblings {
                a.remove(&s.id);
            }
        }

        let should_fan_out = match remaining_depth {
            None => true,
            Some(n) => n > 1,
        };
        if should_fan_out {
            let renamed_target_id = target_node_id.clone();
            let new_slice_id = slice_node_id.clone();
            let session_for_children = session_id.clone();
            let coord = self.clone();
            let state_for_children = state.clone();
            tokio::spawn(async move {
                let on_slice = coord
                    .begin_child_partition(
                        &state_for_children,
                        &session_for_children,
                        &new_slice_id,
                        remaining_depth,
                    )
                    .await;
                if let Err(e) = on_slice {
                    tracing::warn!(error = %e, "fan-out child on slice failed");
                }
                let on_target = coord
                    .begin_child_partition(
                        &state_for_children,
                        &session_for_children,
                        &renamed_target_id,
                        remaining_depth,
                    )
                    .await;
                if let Err(e) = on_target {
                    tracing::warn!(error = %e, "fan-out child on renamed target failed");
                }
            });
        }
        Ok(())
    }

    pub async fn abandon_partition(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<(), AppError> {
        let row = load_partition_row(state, partition_id).await?;
        {
            let mut a = self.inner.abandoning.lock().unwrap();
            a.insert(partition_id);
        }
        let handle = self.inner.handles.lock().await.remove(&partition_id);
        if let Some(handle) = handle {
            (handle.cancel)();
        }
        let now = unix_seconds();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE partition_id = ?2 AND status = 'running'",
                    tokio_rusqlite::params![now, partition_id],
                )?;
                Ok(())
            })
            .await?;
        self.emit(
            &row.session_id,
            SseEvent::Cancelled {
                session_id: row.session_id.clone(),
                target_node_id: row.target_node_id.clone(),
                partition_id,
            },
        );
        let worktree_path = PathBuf::from(&row.worktree_path);
        if worktree_path.exists() {
            if let Err(e) = git::worktree_remove(&state.repo_root, &worktree_path).await {
                tracing::warn!(error = %e, "removing partition worktree on abandon failed");
            }
        }
        if let Some(parent_dir) = worktree_path.parent() {
            let _ = tokio::fs::remove_dir_all(parent_dir).await;
        }
        state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM runs WHERE partition_id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                tx.execute(
                    "DELETE FROM partitions WHERE id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        {
            let mut a = self.inner.abandoning.lock().unwrap();
            a.remove(&partition_id);
        }
        Ok(())
    }

    pub async fn list_partitions(
        &self,
        state: &AppState,
        session_id: &str,
        target_node_id: Option<&str>,
    ) -> Result<Vec<Partition>, AppError> {
        let session_owned = session_id.to_string();
        let target_owned = target_node_id.map(|s| s.to_string());
        let rows: Vec<PartitionRow> = state
            .db
            .call(move |conn| {
                let (sql, has_filter) = match &target_owned {
                    Some(_) => (
                        "SELECT id, session_id, target_node_id, strategy, change_survey_json, plan_json, candidate_slice_tree_sha, candidate_slice_commit_sha, phase, phase_state, worktree_path, remaining_depth, created_at \
                         FROM partitions WHERE session_id = ?1 AND target_node_id = ?2 ORDER BY created_at",
                        true,
                    ),
                    None => (
                        "SELECT id, session_id, target_node_id, strategy, change_survey_json, plan_json, candidate_slice_tree_sha, candidate_slice_commit_sha, phase, phase_state, worktree_path, remaining_depth, created_at \
                         FROM partitions WHERE session_id = ?1 ORDER BY created_at",
                        false,
                    ),
                };
                let mut stmt = conn.prepare(sql)?;
                let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<PartitionRow> {
                    Ok(PartitionRow {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        target_node_id: row.get(2)?,
                        strategy: row
                            .get::<_, Option<String>>(3)?
                            .and_then(|s| parse_strategy(&s)),
                        change_survey_json: row.get(4)?,
                        plan_json: row.get(5)?,
                        candidate_slice_tree_sha: row.get(6)?,
                        candidate_slice_commit_sha: row.get(7)?,
                        phase: parse_phase(&row.get::<_, String>(8)?).unwrap_or(PhaseName::Survey),
                        phase_state: parse_phase_state(&row.get::<_, String>(9)?)
                            .unwrap_or(PhaseState::Error),
                        worktree_path: row.get(10)?,
                        remaining_depth: row.get(11)?,
                        created_at: row.get(12)?,
                    })
                };
                let rows = if has_filter {
                    let target = target_owned.unwrap();
                    stmt.query_map(tokio_rusqlite::params![session_owned, target], mapper)?
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    stmt.query_map(tokio_rusqlite::params![session_owned], mapper)?
                        .collect::<Result<Vec<_>, _>>()?
                };
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn get_partition(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<Partition, AppError> {
        let row = load_partition_row(state, partition_id).await?;
        Ok(row.into())
    }

    async fn spawn_run(
        &self,
        state: AppState,
        partition_id: i64,
        kind: RunKind,
        parent_run_id: Option<i64>,
        user_feedback: Option<String>,
        strategy_override: Option<PartitionStrategy>,
    ) -> Result<Run, AppError> {
        let partition = load_partition_row(&state, partition_id).await?;
        let now = unix_seconds();
        let session_id_db = partition.session_id.clone();
        let target_id_db = partition.target_node_id.clone();
        let kind_str = kind.as_str().to_string();
        let run_id: i64 = state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO runs (partition_id, session_id, target_node_id, kind, parent_run_id, status, started_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6)",
                    tokio_rusqlite::params![partition_id, session_id_db, target_id_db, kind_str, parent_run_id, now],
                )?;
                let id = tx.last_insert_rowid();
                tx.commit()?;
                Ok(id)
            })
            .await?;

        let session_id_phase = partition.session_id.clone();
        let target_id_phase = partition.target_node_id.clone();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE partitions SET phase = ?1, phase_state = 'running' WHERE id = ?2",
                    tokio_rusqlite::params![kind.phase().as_str(), partition_id],
                )?;
                Ok(())
            })
            .await?;
        self.emit(
            &session_id_phase,
            SseEvent::Phase {
                session_id: session_id_phase.clone(),
                target_node_id: target_id_phase.clone(),
                partition_id,
                name: kind.phase(),
                state: PhaseState::Running,
                payload: None,
            },
        );

        let prompt = self.build_prompt(&state, &partition, kind, user_feedback.as_deref(), strategy_override).await?;
        let settings = state.partition_settings.snapshot().await;
        let model = resolve_model(&settings, kind.phase());

        let (tx_helper, rx_helper) = mpsc::channel::<HelperEvent>(64);
        let request = RunRequest {
            model,
            cwd: PathBuf::from(&partition.worktree_path),
            prompt,
            run_id,
        };

        let handle = self.inner.runner.run(request, tx_helper).await?;
        self.inner.handles.lock().await.insert(partition_id, handle);

        let coord = self.clone();
        let state_for_task = state.clone();
        tokio::spawn(async move {
            coord
                .drive_run(state_for_task, partition_id, run_id, kind, rx_helper)
                .await;
        });

        Ok(Run {
            id: run_id,
            partition_id,
            kind,
            status: RunStatus::Running,
            result: None,
            error_message: None,
            started_at: now,
            finished_at: None,
        })
    }

    async fn build_prompt(
        &self,
        state: &AppState,
        partition: &PartitionRow,
        kind: RunKind,
        user_feedback: Option<&str>,
        strategy_override: Option<PartitionStrategy>,
    ) -> Result<String, AppError> {
        let (target_node, parent_node) =
            fetch_target_and_parent(state, &partition.session_id, &partition.target_node_id).await?;
        let parent = parent_node.ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
        let before_tree = parent.tree_sha.clone();
        let target_tree = target_node.tree_sha.clone();

        let prompt = match kind {
            RunKind::Survey => {
                let ctx = subagents::surveyor::SurveyContext {
                    before_tree,
                    target_tree,
                    user_feedback: user_feedback.unwrap_or("").to_string(),
                };
                subagents::surveyor::render_prompt(&ctx, &self.inner.subagents)
            }
            RunKind::Plan => {
                let survey_json = partition
                    .change_survey_json
                    .clone()
                    .unwrap_or_else(|| "{}".to_string());
                let strategy_override_str = match strategy_override {
                    Some(s) => s.as_str().to_string(),
                    None => "auto".to_string(),
                };
                let prior_attempt = self.lookup_prior_attempt(state, partition.id).await?;
                let ctx = subagents::planner::PlanContext {
                    before_tree,
                    target_tree,
                    change_survey_json: survey_json,
                    strategy_override: strategy_override_str,
                    user_feedback: user_feedback.unwrap_or("").to_string(),
                    prior_attempt,
                };
                subagents::planner::render_prompt(&ctx, &self.inner.subagents)
            }
            RunKind::Construct => {
                let plan_json = partition
                    .plan_json
                    .as_deref()
                    .ok_or_else(|| AppError::BadRequest("no plan accepted".into()))?;
                let plan: PlanOutput = serde_json::from_str(plan_json)
                    .map_err(|e| AppError::BadRequest(format!("invalid plan: {e}")))?;
                let edges = match &plan {
                    PlanOutput::Split { edges, .. } => edges,
                    PlanOutput::Indivisible { .. } => {
                        return Err(AppError::BadRequest(
                            "cannot run constructor: plan is indivisible".into(),
                        ))
                    }
                };
                let strategy = partition
                    .strategy
                    .ok_or_else(|| AppError::BadRequest("no strategy on partition".into()))?;
                let ctx = subagents::constructor::ConstructContext {
                    before_tree: before_tree.clone(),
                    target_tree,
                    worktree_head_tree: before_tree,
                    strategy: strategy.as_str().to_string(),
                    slice_title: edges[0].title.clone(),
                    slice_description: edges[0].description.clone(),
                    user_feedback: user_feedback.unwrap_or("").to_string(),
                };
                subagents::constructor::render_prompt(&ctx, &self.inner.subagents)
            }
        };
        Ok(prompt)
    }

    async fn lookup_prior_attempt(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<Option<PriorAttempt>, AppError> {
        let runs = load_runs_for_partition(state, partition_id).await?;
        let last_construct = runs
            .iter()
            .find(|r| r.kind == RunKind::Construct && matches!(r.status, RunStatus::Finished | RunStatus::Error));
        let Some(construct_run) = last_construct else {
            return Ok(None);
        };
        if let Some(json) = construct_run.result_json.as_deref() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                let outcome = value.get("outcome").and_then(|v| v.as_str()).unwrap_or("");
                if outcome == "blocked" {
                    let reason = value
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return Ok(Some(PriorAttempt::Blocked { reason }));
                }
                if outcome == "ok" {
                    let last_plan = runs
                        .iter()
                        .find(|r| r.kind == RunKind::Plan && r.status == RunStatus::Finished);
                    if let Some(plan_run) = last_plan {
                        if let Some(plan_json) = plan_run.result_json.as_deref() {
                            if let Ok(PlanOutput::Split { edges, .. }) =
                                serde_json::from_str::<PlanOutput>(plan_json)
                            {
                                if !edges.is_empty() {
                                    return Ok(Some(PriorAttempt::Candidate {
                                        slice_title: edges[0].title.clone(),
                                        slice_description: edges[0].description.clone(),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    async fn drive_run(
        &self,
        state: AppState,
        partition_id: i64,
        run_id: i64,
        kind: RunKind,
        mut rx: mpsc::Receiver<HelperEvent>,
    ) {
        let partition_row = match load_partition_row(&state, partition_id).await {
            Ok(r) => r,
            Err(_) => return,
        };
        let session_id = partition_row.session_id.clone();
        let target_node_id = partition_row.target_node_id.clone();

        let mut final_result: Option<String> = None;
        let mut error: Option<(String, String)> = None;
        let mut cancelled = false;

        while let Some(ev) = rx.recv().await {
            if self.inner.abandoning.lock().unwrap().contains(&partition_id) {
                continue;
            }
            match ev {
                HelperEvent::Started { .. } => {}
                HelperEvent::SdkMessage { message, .. } => {
                    self.emit(
                        &session_id,
                        SseEvent::SdkMessage {
                            session_id: session_id.clone(),
                            target_node_id: target_node_id.clone(),
                            partition_id,
                            message,
                        },
                    );
                }
                HelperEvent::Finished { result, .. } => {
                    final_result = Some(result);
                }
                HelperEvent::Error { code, message, .. } => {
                    error = Some((code, message));
                }
                HelperEvent::Cancelled { .. } => {
                    cancelled = true;
                }
            }
        }

        self.inner.handles.lock().await.remove(&partition_id);

        if self.inner.abandoning.lock().unwrap().contains(&partition_id) {
            return;
        }

        if cancelled {
            return;
        }

        if let Some((code, message)) = error {
            self.finalize_error(&state, partition_id, run_id, &session_id, &target_node_id, &code, &message)
                .await;
            return;
        }

        let raw = match final_result {
            Some(r) => r,
            None => {
                self.finalize_error(
                    &state,
                    partition_id,
                    run_id,
                    &session_id,
                    &target_node_id,
                    "helper_exited",
                    "no terminal event from helper",
                )
                .await;
                return;
            }
        };

        if let Err(e) = self
            .finalize_run_result(&state, partition_id, run_id, kind, &session_id, &target_node_id, raw)
            .await
        {
            tracing::error!(error = %e, "finalizing run failed");
            self.finalize_error(
                &state,
                partition_id,
                run_id,
                &session_id,
                &target_node_id,
                "internal",
                &format!("{e}"),
            )
            .await;
        }
    }

    async fn finalize_error(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
        session_id: &str,
        target_node_id: &str,
        code: &str,
        message: &str,
    ) {
        let now = unix_seconds();
        let msg_db = message.to_string();
        let _ = state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "UPDATE runs SET status = 'error', error_message = ?1, finished_at = ?2 WHERE id = ?3",
                    tokio_rusqlite::params![msg_db, now, run_id],
                )?;
                tx.execute(
                    "UPDATE partitions SET phase_state = 'error' WHERE id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await;
        self.emit(
            session_id,
            SseEvent::Error {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id,
                code: code.to_string(),
                message: message.to_string(),
            },
        );
    }

    async fn finalize_run_result(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
        kind: RunKind,
        session_id: &str,
        target_node_id: &str,
        raw: String,
    ) -> Result<(), AppError> {
        match kind {
            RunKind::Survey => {
                let parsed = subagents::surveyor::parse_output(&raw).map_err(|e| {
                    AppError::Internal(anyhow!("parsing survey output: {e}"))
                });
                match parsed {
                    Ok(out) => {
                        let json = serde_json::to_string(&out)
                            .map_err(|e| AppError::Internal(anyhow!("survey json: {e}")))?;
                        let now = unix_seconds();
                        let json_db = json.clone();
                        let raw_db = raw.clone();
                        state
                            .db
                            .call(move |conn| {
                                conn.execute(
                                    "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                                    tokio_rusqlite::params![json_db, raw_db, now, run_id],
                                )?;
                                Ok(())
                            })
                            .await?;
                        self.handle_phase_terminal(state, partition_id, RunKind::Survey, run_id, session_id, target_node_id, serde_json::from_str(&json).ok())
                            .await?;
                    }
                    Err(e) => {
                        self.finalize_parse_error(state, partition_id, run_id, session_id, target_node_id, &raw, &format!("{e}"))
                            .await;
                    }
                }
            }
            RunKind::Plan => {
                let parsed = subagents::planner::parse_output(&raw).map_err(|e| {
                    AppError::Internal(anyhow!("parsing plan output: {e}"))
                });
                match parsed {
                    Ok(out) => {
                        let json = serde_json::to_string(&out)
                            .map_err(|e| AppError::Internal(anyhow!("plan json: {e}")))?;
                        let now = unix_seconds();
                        let json_db = json.clone();
                        let raw_db = raw.clone();
                        state
                            .db
                            .call(move |conn| {
                                conn.execute(
                                    "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                                    tokio_rusqlite::params![json_db, raw_db, now, run_id],
                                )?;
                                Ok(())
                            })
                            .await?;
                        self.handle_phase_terminal(state, partition_id, RunKind::Plan, run_id, session_id, target_node_id, serde_json::from_str(&json).ok())
                            .await?;
                    }
                    Err(e) => {
                        self.finalize_parse_error(state, partition_id, run_id, session_id, target_node_id, &raw, &format!("{e}"))
                            .await;
                    }
                }
            }
            RunKind::Construct => {
                let parsed = subagents::constructor::parse_output(&raw);
                match parsed {
                    Ok(ConstructOutput::Ok) => {
                        self.constructor_capture_ok(state, partition_id, run_id, &raw, session_id, target_node_id)
                            .await?;
                    }
                    Ok(ConstructOutput::Blocked { reason }) => {
                        self.constructor_capture_blocked(state, partition_id, run_id, &raw, &reason, session_id, target_node_id)
                            .await?;
                    }
                    Err(e) => {
                        self.finalize_parse_error(state, partition_id, run_id, session_id, target_node_id, &raw, &format!("{e}"))
                            .await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn finalize_parse_error(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
        session_id: &str,
        target_node_id: &str,
        raw: &str,
        msg: &str,
    ) {
        let now = unix_seconds();
        let raw_db = raw.to_string();
        let msg_db = msg.to_string();
        let _ = state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "UPDATE runs SET status = 'error', error_message = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                    tokio_rusqlite::params![msg_db, raw_db, now, run_id],
                )?;
                tx.execute(
                    "UPDATE partitions SET phase_state = 'error' WHERE id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await;
        self.emit(
            session_id,
            SseEvent::Error {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id,
                code: "parse_error".to_string(),
                message: msg.to_string(),
            },
        );
    }

    async fn handle_phase_terminal(
        &self,
        state: &AppState,
        partition_id: i64,
        kind: RunKind,
        run_id: i64,
        session_id: &str,
        target_node_id: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let settings = state.partition_settings.snapshot().await;
        let hitl = settings.coordinator.human_in_the_loop;

        if kind == RunKind::Plan {
            let outcome = payload
                .as_ref()
                .and_then(|p| p.get("outcome"))
                .and_then(|v| v.as_str())
                .unwrap_or("split");
            if outcome == "indivisible" {
                if hitl.after_indivisible {
                    state
                        .db
                        .call(move |conn| {
                            conn.execute(
                                "UPDATE partitions SET phase_state = 'awaiting_review' WHERE id = ?1",
                                tokio_rusqlite::params![partition_id],
                            )?;
                            Ok(())
                        })
                        .await?;
                    self.emit(
                        session_id,
                        SseEvent::Phase {
                            session_id: session_id.to_string(),
                            target_node_id: target_node_id.to_string(),
                            partition_id,
                            name: PhaseName::Plan,
                            state: PhaseState::AwaitingReview,
                            payload,
                        },
                    );
                } else {
                    let coord = self.clone();
                    let state_owned = state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = coord.abandon_partition(&state_owned, partition_id).await {
                            tracing::error!(error = %e, partition_id, "auto-abandon on indivisible failed");
                        }
                    });
                }
                return Ok(());
            }
        }

        let gate = match kind {
            RunKind::Survey => hitl.after_survey,
            RunKind::Plan => hitl.after_planning,
            RunKind::Construct => hitl.after_construct,
        };
        if gate {
            state
                .db
                .call(move |conn| {
                    conn.execute(
                        "UPDATE partitions SET phase_state = 'awaiting_review' WHERE id = ?1",
                        tokio_rusqlite::params![partition_id],
                    )?;
                    Ok(())
                })
                .await?;
            self.emit(
                session_id,
                SseEvent::Phase {
                    session_id: session_id.to_string(),
                    target_node_id: target_node_id.to_string(),
                    partition_id,
                    name: kind.phase(),
                    state: PhaseState::AwaitingReview,
                    payload,
                },
            );
        } else {
            let coord = self.clone();
            let state_owned = state.clone();
            let fut: std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), AppError>> + Send>,
            > = match kind {
                RunKind::Survey => Box::pin(async move {
                    coord
                        .do_accept_survey(&state_owned, partition_id, run_id)
                        .await
                        .map(|_| ())
                }),
                RunKind::Plan => Box::pin(async move {
                    coord
                        .do_accept_plan(&state_owned, partition_id, run_id)
                        .await
                        .map(|_| ())
                }),
                RunKind::Construct => Box::pin(async move {
                    coord.do_accept_construct(&state_owned, partition_id).await
                }),
            };
            tokio::spawn(async move {
                if let Err(e) = fut.await {
                    tracing::error!(error = %e, partition_id, "auto-accept failed");
                }
            });
        }
        Ok(())
    }

    async fn constructor_capture_ok(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
        raw: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<(), AppError> {
        let row = load_partition_row(state, partition_id).await?;
        let (_t, parent) =
            fetch_target_and_parent(state, &row.session_id, &row.target_node_id).await?;
        let parent = parent.ok_or_else(|| AppError::BadRequest("no parent".into()))?;
        let plan_json = row.plan_json.as_deref().ok_or_else(|| AppError::BadRequest("no plan".into()))?;
        let plan: PlanOutput = serde_json::from_str(plan_json)
            .map_err(|e| AppError::BadRequest(format!("invalid plan: {e}")))?;
        let slice_title = match &plan {
            PlanOutput::Split { edges, .. } => edges[0].title.clone(),
            PlanOutput::Indivisible { .. } => {
                return Err(AppError::BadRequest(
                    "constructor produced OK for an indivisible plan".into(),
                ))
            }
        };

        let worktree_path = PathBuf::from(&row.worktree_path);
        git::run_in(&worktree_path, &["add", "-A"])
            .await
            .map_err(|e| AppError::Internal(anyhow!("git add -A: {e}")))?;
        let tree_sha = git::write_tree(&worktree_path)
            .await
            .map_err(|e| AppError::Internal(anyhow!("write-tree: {e}")))?;
        let commit_sha = git::commit_tree(&state.repo_root, &tree_sha, &[&parent.commit_sha], &slice_title)
            .await
            .map_err(|e| AppError::Internal(anyhow!("commit-tree: {e}")))?;
        git::run_in(&worktree_path, &["reset", "--hard", &parent.commit_sha])
            .await
            .map_err(|e| AppError::Internal(anyhow!("reset --hard: {e}")))?;
        git::run_in(&worktree_path, &["clean", "-fdx"])
            .await
            .map_err(|e| AppError::Internal(anyhow!("clean -fdx: {e}")))?;

        let now = unix_seconds();
        let result_json = serde_json::json!({
            "outcome": "ok",
            "candidateTreeSha": tree_sha,
            "candidateCommitSha": commit_sha,
        });
        let result_json_str = result_json.to_string();
        let raw_db = raw.to_string();
        let tree_db = tree_sha.clone();
        let commit_db = commit_sha.clone();
        state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "UPDATE partitions SET candidate_slice_tree_sha = ?1, candidate_slice_commit_sha = ?2, phase = 'construct', phase_state = 'running' WHERE id = ?3",
                    tokio_rusqlite::params![tree_db, commit_db, partition_id],
                )?;
                tx.execute(
                    "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                    tokio_rusqlite::params![result_json_str, raw_db, now, run_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;

        let payload = Some(serde_json::json!({
            "outcome": "ok",
            "candidateTreeSha": tree_sha,
            "candidateCommitSha": commit_sha,
        }));
        self.handle_phase_terminal(state, partition_id, RunKind::Construct, run_id, session_id, target_node_id, payload)
            .await?;
        Ok(())
    }

    async fn constructor_capture_blocked(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
        raw: &str,
        reason: &str,
        session_id: &str,
        target_node_id: &str,
    ) -> Result<(), AppError> {
        let row = load_partition_row(state, partition_id).await?;
        let (_t, parent) =
            fetch_target_and_parent(state, &row.session_id, &row.target_node_id).await?;
        let parent = parent.ok_or_else(|| AppError::BadRequest("no parent".into()))?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        if let Err(e) =
            git::run_in(&worktree_path, &["reset", "--hard", &parent.commit_sha]).await
        {
            tracing::warn!(error = %e, "reset on blocked failed");
        }
        if let Err(e) = git::run_in(&worktree_path, &["clean", "-fdx"]).await {
            tracing::warn!(error = %e, "clean on blocked failed");
        }
        let now = unix_seconds();
        let result_json = serde_json::json!({
            "outcome": "blocked",
            "reason": reason,
        });
        let result_json_str = result_json.to_string();
        let raw_db = raw.to_string();
        state
            .db
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "UPDATE partitions SET phase = 'construct', phase_state = 'awaiting_review' WHERE id = ?1",
                    tokio_rusqlite::params![partition_id],
                )?;
                tx.execute(
                    "UPDATE runs SET status = 'finished', result_json = ?1, result_text = ?2, finished_at = ?3 WHERE id = ?4",
                    tokio_rusqlite::params![result_json_str, raw_db, now, run_id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        self.emit(
            session_id,
            SseEvent::Phase {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
                partition_id,
                name: PhaseName::Construct,
                state: PhaseState::AwaitingReview,
                payload: Some(serde_json::json!({"outcome": "blocked", "reason": reason})),
            },
        );
        Ok(())
    }
}

fn validate_run_kind_transition(phase: PhaseName, kind: RunKind) -> Result<(), AppError> {
    let ok = matches!(
        (phase, kind),
        (PhaseName::Survey, RunKind::Survey)
            | (PhaseName::Plan, RunKind::Plan)
            | (PhaseName::Construct, RunKind::Construct)
            | (PhaseName::Construct, RunKind::Plan)
    );
    if ok {
        Ok(())
    } else {
        Err(AppError::Conflict {
            code: "invalid_run_kind".into(),
            message: format!(
                "cannot start run kind {} from phase {}",
                kind.as_str(),
                phase.as_str()
            ),
        })
    }
}

fn parse_phase(s: &str) -> Option<PhaseName> {
    match s {
        "survey" => Some(PhaseName::Survey),
        "plan" => Some(PhaseName::Plan),
        "construct" => Some(PhaseName::Construct),
        _ => None,
    }
}

fn parse_phase_state(s: &str) -> Option<PhaseState> {
    match s {
        "running" => Some(PhaseState::Running),
        "awaiting_review" => Some(PhaseState::AwaitingReview),
        "error" => Some(PhaseState::Error),
        _ => None,
    }
}

fn parse_strategy(s: &str) -> Option<PartitionStrategy> {
    match s {
        "semantic" => Some(PartitionStrategy::Semantic),
        "vertical" => Some(PartitionStrategy::Vertical),
        "horizontal" => Some(PartitionStrategy::Horizontal),
        _ => None,
    }
}

fn parse_run_kind(s: &str) -> Option<RunKind> {
    match s {
        "survey" => Some(RunKind::Survey),
        "plan" => Some(RunKind::Plan),
        "construct" => Some(RunKind::Construct),
        _ => None,
    }
}

fn parse_run_status(s: &str) -> Option<RunStatus> {
    match s {
        "running" => Some(RunStatus::Running),
        "finished" => Some(RunStatus::Finished),
        "error" => Some(RunStatus::Error),
        "cancelled" => Some(RunStatus::Cancelled),
        _ => None,
    }
}

async fn load_partition_row(state: &AppState, partition_id: i64) -> Result<PartitionRow, AppError> {
    let repo_root = state.repo_root.to_string_lossy().to_string();
    let row: Option<PartitionRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.session_id, p.target_node_id, p.strategy, p.change_survey_json, p.plan_json, p.candidate_slice_tree_sha, p.candidate_slice_commit_sha, p.phase, p.phase_state, p.worktree_path, p.remaining_depth, p.created_at \
                 FROM partitions p JOIN sessions s ON s.id = p.session_id \
                 WHERE p.id = ?1 AND s.repo_root = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![partition_id, repo_root])?;
            if let Some(r) = rows.next()? {
                Ok(Some(PartitionRow {
                    id: r.get(0)?,
                    session_id: r.get(1)?,
                    target_node_id: r.get(2)?,
                    strategy: r.get::<_, Option<String>>(3)?.and_then(|s| parse_strategy(&s)),
                    change_survey_json: r.get(4)?,
                    plan_json: r.get(5)?,
                    candidate_slice_tree_sha: r.get(6)?,
                    candidate_slice_commit_sha: r.get(7)?,
                    phase: parse_phase(&r.get::<_, String>(8)?).unwrap_or(PhaseName::Survey),
                    phase_state: parse_phase_state(&r.get::<_, String>(9)?)
                        .unwrap_or(PhaseState::Error),
                    worktree_path: r.get(10)?,
                    remaining_depth: r.get(11)?,
                    created_at: r.get(12)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    row.ok_or(AppError::NotFound)
}

async fn load_run(state: &AppState, run_id: i64) -> Result<RunRow, AppError> {
    let row: Option<RunRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, partition_id, session_id, target_node_id, kind, parent_run_id, status, result_json, result_text, error_message, started_at, finished_at \
                 FROM runs WHERE id = ?1",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![run_id])?;
            if let Some(r) = rows.next()? {
                Ok(Some(RunRow {
                    id: r.get(0)?,
                    partition_id: r.get(1)?,
                    session_id: r.get(2)?,
                    target_node_id: r.get(3)?,
                    kind: parse_run_kind(&r.get::<_, String>(4)?).unwrap_or(RunKind::Survey),
                    parent_run_id: r.get(5)?,
                    status: parse_run_status(&r.get::<_, String>(6)?).unwrap_or(RunStatus::Error),
                    result_json: r.get(7)?,
                    result_text: r.get(8)?,
                    error_message: r.get(9)?,
                    started_at: r.get(10)?,
                    finished_at: r.get(11)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?;
    row.ok_or(AppError::NotFound)
}

#[derive(Debug, Clone)]
struct SiblingInfo {
    id: i64,
    target_node_id: String,
    worktree_path: String,
}

async fn load_sibling_partitions(
    state: &AppState,
    session_id: &str,
    target_node_id: &str,
    accepted_partition_id: i64,
) -> Result<Vec<SiblingInfo>, AppError> {
    let session_owned = session_id.to_string();
    let target_owned = target_node_id.to_string();
    let rows: Vec<SiblingInfo> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, target_node_id, worktree_path FROM partitions \
                 WHERE session_id = ?1 AND target_node_id = ?2 AND id != ?3",
            )?;
            let rows = stmt
                .query_map(
                    tokio_rusqlite::params![session_owned, target_owned, accepted_partition_id],
                    |r| {
                        Ok(SiblingInfo {
                            id: r.get(0)?,
                            target_node_id: r.get(1)?,
                            worktree_path: r.get(2)?,
                        })
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

async fn load_runs_for_partition(
    state: &AppState,
    partition_id: i64,
) -> Result<Vec<RunRow>, AppError> {
    let rows: Vec<RunRow> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, partition_id, session_id, target_node_id, kind, parent_run_id, status, result_json, result_text, error_message, started_at, finished_at \
                 FROM runs WHERE partition_id = ?1 ORDER BY started_at DESC, id DESC",
            )?;
            let rows = stmt
                .query_map(tokio_rusqlite::params![partition_id], |r| {
                    Ok(RunRow {
                        id: r.get(0)?,
                        partition_id: r.get(1)?,
                        session_id: r.get(2)?,
                        target_node_id: r.get(3)?,
                        kind: parse_run_kind(&r.get::<_, String>(4)?).unwrap_or(RunKind::Survey),
                        parent_run_id: r.get(5)?,
                        status: parse_run_status(&r.get::<_, String>(6)?).unwrap_or(RunStatus::Error),
                        result_json: r.get(7)?,
                        result_text: r.get(8)?,
                        error_message: r.get(9)?,
                        started_at: r.get(10)?,
                        finished_at: r.get(11)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(rows)
}

#[derive(Debug, Clone)]
struct NodeBasic {
    node_id: String,
    tree_sha: String,
    commit_sha: String,
}

async fn fetch_target_and_parent(
    state: &AppState,
    session_id: &str,
    target_node_id: &str,
) -> Result<(NodeBasic, Option<NodeBasic>), AppError> {
    let session_owned = session_id.to_string();
    let target_owned = target_node_id.to_string();
    let result: Option<(NodeBasic, Option<NodeBasic>)> = state
        .db
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT node_id, parent_node_id, tree_sha, commit_sha FROM nodes WHERE session_id = ?1 AND node_id = ?2",
            )?;
            let mut rows = stmt.query(tokio_rusqlite::params![session_owned, target_owned])?;
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
                let mut prows = pstmt.query(tokio_rusqlite::params![session_owned, pid])?;
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

fn unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

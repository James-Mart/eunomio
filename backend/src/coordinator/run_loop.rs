use crate::{
    cursor_bridge::{HelperEvent, RunRequest},
    db,
    error::AppError,
    partition_settings::resolve_model,
    repo,
    state::AppState,
    subagents::{self, constructor::ConstructOutput},
    types::*,
    worktree,
};
use anyhow::anyhow;
use serde::Serialize;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::sync::mpsc;

use super::Coordinator;

impl Coordinator {
    pub async fn start_run(
        &self,
        state: &AppState,
        partition_id: i64,
        req: StartRunRequest,
    ) -> Result<Run, AppError> {
        if self.inner.runs.has_in_flight(partition_id).await {
            return Err(AppError::Conflict {
                code: "partition_run_in_flight".into(),
                message: "this partition already has a run in flight".into(),
            });
        }
        let row = repo::partition::get(state, partition_id).await?;
        if !matches!(row.phase_state, PhaseState::AwaitingReview | PhaseState::Error) {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not currently at a review gate or in error state".into(),
            });
        }
        validate_run_kind_transition(row.phase, req.kind)?;
        let kind = req.kind;
        if kind == RunKind::Plan && row.phase == PhaseName::Construct {
            self.reset_for_construct_to_plan_back_edge(state, partition_id, &row)
                .await?;
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
            repo::node::target_and_parent(state, &row.session_id, &row.target_node_id).await?;
        let parent =
            parent_node.ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        worktree::reset_to_parent(&worktree_path, &parent.commit_sha, false).await?;
        repo::partition::clear_plan_and_slice(state, partition_id).await?;
        Ok(())
    }

    pub async fn cancel_run(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
    ) -> Result<(), AppError> {
        let run = repo::run::get(state, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::NotFound);
        }
        if !matches!(run.status, RunStatus::Running) {
            return Err(AppError::Conflict {
                code: "run_not_running".into(),
                message: "run is not in running state".into(),
            });
        }
        let row = repo::partition::get(state, partition_id).await?;
        self.inner.runs.take_and_cancel(partition_id).await;
        repo::partition::cancel_run(state, partition_id, run_id).await?;
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

    pub(super) fn spawn_run_boxed(
        &self,
        state: AppState,
        partition_id: i64,
        kind: RunKind,
        parent_run_id: Option<i64>,
        user_feedback: Option<String>,
        strategy_override: Option<PartitionStrategy>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Run, AppError>> + Send + '_>> {
        Box::pin(self.spawn_run(
            state,
            partition_id,
            kind,
            parent_run_id,
            user_feedback,
            strategy_override,
        ))
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
        let partition = repo::partition::get(&state, partition_id).await?;
        let now = db::unix_seconds();
        let run_id = repo::run::start(
            &state,
            partition_id,
            partition.session_id.clone(),
            partition.target_node_id.clone(),
            kind,
            parent_run_id,
            now,
        )
        .await?;

        let session_id = partition.session_id.clone();
        let target_node_id = partition.target_node_id.clone();
        repo::partition::set_phase_running(&state, partition_id, kind.phase()).await?;
        self.emit(
            &session_id,
            SseEvent::Phase {
                session_id: session_id.clone(),
                target_node_id: target_node_id.clone(),
                partition_id,
                name: kind.phase(),
                state: PhaseState::Running,
                payload: None,
            },
        );

        let prompt = self
            .build_prompt(
                &state,
                &partition,
                kind,
                user_feedback.as_deref(),
                strategy_override,
            )
            .await?;
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
        self.inner.runs.insert(partition_id, handle).await;

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

    async fn drive_run(
        &self,
        state: AppState,
        partition_id: i64,
        run_id: i64,
        kind: RunKind,
        mut rx: mpsc::Receiver<HelperEvent>,
    ) {
        let Ok(partition_row) = repo::partition::get(&state, partition_id).await else {
            return;
        };
        let session_id = partition_row.session_id.clone();
        let target_node_id = partition_row.target_node_id.clone();

        let mut final_result: Option<String> = None;
        let mut error: Option<(String, String)> = None;
        let mut cancelled = false;

        while let Some(ev) = rx.recv().await {
            if self.inner.runs.is_abandoning(partition_id) {
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
                HelperEvent::Finished { result, .. } => final_result = Some(result),
                HelperEvent::Error { code, message, .. } => error = Some((code, message)),
                HelperEvent::Cancelled { .. } => cancelled = true,
            }
        }

        self.inner.runs.forget(partition_id).await;

        if self.inner.runs.is_abandoning(partition_id) || cancelled {
            return;
        }

        if let Some((code, message)) = error {
            self.finalize_error(
                &state,
                partition_id,
                run_id,
                &session_id,
                &target_node_id,
                &code,
                &message,
            )
            .await;
            return;
        }

        let Some(raw) = final_result else {
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
        };

        if let Err(e) = self
            .finalize_run_result(
                &state,
                partition_id,
                run_id,
                kind,
                &session_id,
                &target_node_id,
                raw,
            )
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
        let _ =
            repo::partition::fail_run(state, partition_id, run_id, message.to_string(), None).await;
        self.emit_run_error(session_id, target_node_id, partition_id, code, message);
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
        let _ = repo::partition::fail_run(
            state,
            partition_id,
            run_id,
            msg.to_string(),
            Some(raw.to_string()),
        )
        .await;
        self.emit_run_error(session_id, target_node_id, partition_id, "parse_error", msg);
    }

    fn emit_run_error(
        &self,
        session_id: &str,
        target_node_id: &str,
        partition_id: i64,
        code: &str,
        message: &str,
    ) {
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
                self.finalize_parsed_json_run(
                    state,
                    partition_id,
                    run_id,
                    kind,
                    session_id,
                    target_node_id,
                    &raw,
                    |raw| {
                        subagents::surveyor::parse_output(raw)
                            .map_err(|e| anyhow!("parsing survey output: {e}"))
                    },
                    "survey json",
                )
                .await
            }
            RunKind::Plan => {
                self.finalize_parsed_json_run(
                    state,
                    partition_id,
                    run_id,
                    kind,
                    session_id,
                    target_node_id,
                    &raw,
                    |raw| {
                        subagents::planner::parse_output(raw)
                            .map_err(|e| anyhow!("parsing plan output: {e}"))
                    },
                    "plan json",
                )
                .await
            }
            RunKind::Construct => {
                match subagents::constructor::parse_output(&raw) {
                    Ok(ConstructOutput::Ok) => {
                        self.constructor_capture_ok(
                            state,
                            partition_id,
                            run_id,
                            &raw,
                            session_id,
                            target_node_id,
                        )
                        .await?;
                    }
                    Ok(ConstructOutput::Blocked { reason }) => {
                        self.constructor_capture_blocked(
                            state,
                            partition_id,
                            run_id,
                            &raw,
                            &reason,
                            session_id,
                            target_node_id,
                        )
                        .await?;
                    }
                    Err(e) => {
                        self.finalize_parse_error(
                            state,
                            partition_id,
                            run_id,
                            session_id,
                            target_node_id,
                            &raw,
                            &format!("{e}"),
                        )
                        .await;
                    }
                }
                Ok(())
            }
        }
    }

    /// Survey and Plan share an identical "parse, JSON-encode, persist,
    /// dispatch to the gate" tail; this consolidates it so adding a new
    /// terminal event variant only takes one change.
    async fn finalize_parsed_json_run<T: Serialize>(
        &self,
        state: &AppState,
        partition_id: i64,
        run_id: i64,
        kind: RunKind,
        session_id: &str,
        target_node_id: &str,
        raw: &str,
        parse: impl FnOnce(&str) -> Result<T, anyhow::Error>,
        json_label: &'static str,
    ) -> Result<(), AppError> {
        match parse(raw) {
            Ok(out) => {
                let json = serde_json::to_string(&out)
                    .map_err(|e| AppError::Internal(anyhow!("{json_label}: {e}")))?;
                repo::run::finish_success(state, run_id, json.clone(), Some(raw.to_string()))
                    .await?;
                self.handle_phase_terminal(
                    state,
                    partition_id,
                    kind,
                    run_id,
                    session_id,
                    target_node_id,
                    serde_json::from_str(&json).ok(),
                )
                .await?;
                Ok(())
            }
            Err(e) => {
                self.finalize_parse_error(
                    state,
                    partition_id,
                    run_id,
                    session_id,
                    target_node_id,
                    raw,
                    &format!("{e}"),
                )
                .await;
                Ok(())
            }
        }
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

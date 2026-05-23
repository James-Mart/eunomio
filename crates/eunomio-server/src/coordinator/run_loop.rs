// SPDX-License-Identifier: Apache-2.0

use eunomio_core::types::*;
// Run lifecycle: spawn helper, stream transcript, finalize. Partition and run
// rows carry `org_id` from the database; pass it through to repo calls that
// scope by tenant. Settings and Cursor keys resolve via partition → session →
// user_id.
use crate::{
    cursor_bridge::{HelperEvent, RunRequest},
    AppError,
    partition_settings::{load_for_partition, resolve_model},
    state::AppState,
    subagents::{self, constructor::ConstructOutput},
     
    worktree,
};
use anyhow::anyhow;
use serde::Serialize;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::sync::mpsc;

use super::{scope::ActiveRun, Coordinator};

struct DriveRunContext {
    state: AppState,
    org_id: String,
    partition_id: String,
    run_id: String,
    kind: RunKind,
    transcripts_enabled: bool,
    rx: mpsc::Receiver<HelperEvent>,
}

impl Coordinator {
    pub async fn start_run(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        req: StartRunRequest,
    ) -> Result<Run, AppError> {
        if self.inner.runs.has_in_flight(partition_id).await {
            return Err(AppError::Conflict {
                code: "partition_run_in_flight".into(),
                message: "this partition already has a run in flight".into(),
            });
        }
        let row = state.datastore.partitions().get(org_id, partition_id).await?;
        if !matches!(row.phase_state, PhaseState::AwaitingReview | PhaseState::Error) {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not currently at a review gate or in error state".into(),
            });
        }
        validate_run_kind_transition(row.phase, req.kind)?;
        let kind = req.kind;
        if kind == RunKind::Plan && row.phase == PhaseName::Construct {
            self.reset_for_construct_to_plan_back_edge(state, org_id, partition_id, &row)
                .await?;
        }
        if let Some(ref override_text) = req.prompt_override {
            self.validate_prompt_override(kind, override_text)?;
        }
        self.spawn_run_boxed(
            state.clone(),
            org_id.to_string(),
            partition_id.to_string(),
            req,
        )
        .await
    }

    fn validate_prompt_override(&self, kind: RunKind, text: &str) -> Result<(), AppError> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let _ = self.resolve_template(kind, Some(text))?;
        Ok(())
    }

    async fn reset_for_construct_to_plan_back_edge(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        row: &PartitionRow,
    ) -> Result<(), AppError> {
        let (_, parent_node) = state.datastore.nodes().target_and_parent(org_id, &row.session_id, &row.target_node_id,
        ).await?;
        let parent =
            parent_node.ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
        let worktree_path = PathBuf::from(&row.worktree_path);
        worktree::reset_to_parent(&worktree_path, &parent.commit_sha, false).await?;
        state.datastore.partitions().clear_plan_and_slice(org_id, partition_id).await?;
        Ok(())
    }

    pub async fn cancel_run(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
    ) -> Result<(), AppError> {
        let run = state.datastore.runs().get(org_id, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::NotFound);
        }
        if !matches!(run.status, RunStatus::Running) {
            return Err(AppError::Conflict {
                code: "run_not_running".into(),
                message: "run is not in running state".into(),
            });
        }
        let row = state.datastore.partitions().get(org_id, partition_id).await?;
        self.inner.runs.take_and_cancel(partition_id).await;
        state.datastore.partitions().cancel_run(org_id, partition_id, run_id).await?;
        self.emit(
            &row.session_id,
            SseEvent::Cancelled {
                session_id: row.session_id.clone(),
                target_node_id: row.target_node_id.clone(),
                partition_id: partition_id.to_string(),
            },
        );
        Ok(())
    }

    pub(super) fn spawn_run_boxed(
        &self,
        state: AppState,
        org_id: String,
        partition_id: String,
        req: StartRunRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Run, AppError>> + Send + '_>> {
        Box::pin(self.spawn_run(state, org_id, partition_id, req))
    }

    async fn spawn_run(
        &self,
        state: AppState,
        org_id: String,
        partition_id: String,
        req: StartRunRequest,
    ) -> Result<Run, AppError> {
        let kind = req.kind;
        self.inner.quota.check_can_start_run(&org_id).await?;
        let partition = state.datastore.partitions().get(&org_id, &partition_id).await?;
        if kind == RunKind::Construct {
            let (_, parent_node) = state.datastore.nodes().target_and_parent(&org_id, &partition.session_id, &partition.target_node_id,
            ).await?;
            let parent =
                parent_node.ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
            let worktree_path = PathBuf::from(&partition.worktree_path);
            worktree::reset_to_parent(&worktree_path, &parent.commit_sha, true).await?;
        }
        let prompt = self
            .build_prompt(
                &state,
                &partition,
                kind,
                req.user_feedback.as_deref(),
                req.strategy_override,
                req.prompt_override.as_deref(),
            )
            .await?;
        let settings =
            load_for_partition(&state, &partition.org_id, &partition.session_id).await?;
        let model = resolve_model(&settings, kind.phase());
        let transcripts_enabled = settings.general.transcripts_enabled;
        let prompt_for_helper = prompt.clone();

        let cursor_api_key = state
            .keystore
            .get(&partition.user_id)
            .await
            .map_err(|e| AppError::Internal(anyhow!("reading cursor api key: {e}")))?
            .ok_or_else(|| {
                AppError::Unrecoverable {
                    status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    code: "cursor_sdk_unavailable".into(),
                    message: "Cursor API key not configured".into(),
                }
            })?;

        let now = eunomio_core::unix_seconds();
        let run_id = state.datastore.runs().start(NewRunInsert {
                org_id: org_id.clone(),
                user_id: partition.user_id.clone(),
                partition_id: partition_id.clone(),
                session_id: partition.session_id.clone(),
                target_node_id: partition.target_node_id.clone(),
                kind,
                parent_run_id: req.parent_run_id,
                prompt_text: prompt,
                started_at: now,
            },
        )
        .await?;

        let session_id = partition.session_id.clone();
        let target_node_id = partition.target_node_id.clone();
        state.datastore.partitions().set_phase_running(&org_id, &partition_id, kind.phase()).await?;
        self.emit(
            &session_id,
            SseEvent::Phase {
                session_id: session_id.clone(),
                target_node_id: target_node_id.clone(),
                partition_id: partition_id.clone(),
                name: kind.phase(),
                state: PhaseState::Running,
                payload: None,
            },
        );

        let (tx_helper, rx_helper) = mpsc::channel::<HelperEvent>(64);
        let run_id_return = run_id.clone();
        let request = RunRequest {
            model,
            cwd: PathBuf::from(&partition.worktree_path),
            prompt: prompt_for_helper,
            run_id: run_id.clone(),
            cursor_api_key: Some(cursor_api_key),
        };

        let handle = self.inner.runner.run(request, tx_helper).await?;
        self.inner.runs.insert(&partition_id, handle).await;

        let coord = self.clone();
        let state_for_task = state.clone();
        let partition_id_for_task = partition_id.clone();
        let org_id_for_task = org_id.clone();
        tokio::spawn(async move {
            coord
                .drive_run(DriveRunContext {
                    state: state_for_task,
                    org_id: org_id_for_task,
                    partition_id: partition_id_for_task,
                    run_id,
                    kind,
                    transcripts_enabled,
                    rx: rx_helper,
                }).await;
        });

        Ok(Run {
            id: run_id_return,
            partition_id,
            kind,
            status: RunStatus::Running,
            result: None,
            error_message: None,
            started_at: now,
            finished_at: None,
        })
    }

    async fn drive_run(&self, ctx: DriveRunContext) {
        let DriveRunContext {
            state,
            org_id,
            partition_id,
            run_id,
            kind,
            transcripts_enabled,
            mut rx,
        } = ctx;
        let Ok(partition_row) = state.datastore.partitions().get(&org_id, &partition_id).await else {
            return;
        };
        let active = ActiveRun::new(org_id.clone(), &partition_row, run_id.clone(), kind);
        let session_id = active.scope.session_id.clone();
        let target_node_id = active.scope.target_node_id.clone();

        let mut final_result: Option<String> = None;
        let mut error: Option<(String, String)> = None;
        let mut cancelled = false;

        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let state_for_writer = state.clone();
        let coord = self.clone();
        let session_id_writer = session_id.clone();
        let target_node_id_writer = target_node_id.clone();
        let run_id_writer = run_id.clone();
        let partition_id_writer = partition_id.clone();
        let org_id_writer = org_id.clone();
        let transcript_writer = tokio::spawn(async move {
            while let Some(chunk) = chunk_rx.recv().await {
                if let Err(e) = state_for_writer.datastore.runs().append_transcript_text(&org_id_writer, &run_id_writer, &chunk,
                ).await
                {
                    tracing::warn!(error = %e, run_id = %run_id_writer, "persisting transcript_text failed");
                }
                if transcripts_enabled {
                    coord.emit(
                        &session_id_writer,
                        SseEvent::TranscriptDelta {
                            session_id: session_id_writer.clone(),
                            target_node_id: target_node_id_writer.clone(),
                            partition_id: partition_id_writer.clone(),
                            run_id: run_id_writer.clone(),
                            text: chunk,
                        },
                    );
                }
            }
        });

        while let Some(ev) = rx.recv().await {
            if self.inner.runs.is_abandoning(&partition_id) {
                continue;
            }
            match ev {
                HelperEvent::Started { .. } => {}
                HelperEvent::SdkMessage { message, .. } => {
                    if let Some(chunk) = crate::cursor_bridge::fold_sdk_event(&message) {
                        let _ = chunk_tx.send(chunk);
                    }
                }
                HelperEvent::UsageReported { usage, .. } => {
                    if let Err(e) = self.inner.quota.record_usage(&org_id, usage).await {
                        tracing::warn!(org_id = %org_id, error = %e, "quota record_usage failed");
                    }
                }
                HelperEvent::Finished { result, .. } => final_result = Some(result),
                HelperEvent::Error { code, message, .. } => error = Some((code, message)),
                HelperEvent::Cancelled { .. } => cancelled = true,
            }
        }

        drop(chunk_tx);
        let _ = transcript_writer.await;

        self.inner.runs.forget(&partition_id).await;

        if self.inner.runs.is_abandoning(&partition_id) || cancelled {
            return;
        }

        if let Some((code, message)) = error {
            self.finalize_error(&state, &active, &code, &message).await;
            return;
        }

        let Some(raw) = final_result else {
            self.finalize_error(&state, &active, "helper_exited", "no terminal event from helper")
                .await;
            return;
        };

        if let Err(e) = self.finalize_run_result(&state, &active, raw).await {
            tracing::error!(error = %e, "finalizing run failed");
            self.finalize_error(&state, &active, "internal", &format!("{e}"))
                .await;
        }
    }

    async fn finalize_error(
        &self,
        state: &AppState,
        active: &ActiveRun,
        code: &str,
        message: &str,
    ) {
        let _ = state.datastore.partitions().fail_run(&active.scope.org_id, &active.scope.partition_id, &active.run_id, message.to_string(), None,
        ).await;
        self.emit_run_error(active, code, message);
    }

    async fn finalize_parse_error(
        &self,
        state: &AppState,
        active: &ActiveRun,
        raw: &str,
        msg: &str,
    ) {
        let _ = state.datastore.partitions().fail_run(
            &active.scope.org_id,
            &active.scope.partition_id,
            &active.run_id,
            msg.to_string(),
            Some(raw.to_string()),
        ).await;
        self.emit_run_error(active, "parse_error", msg);
    }

    fn emit_run_error(&self, active: &ActiveRun, code: &str, message: &str) {
        self.emit(
            &active.scope.session_id,
            SseEvent::Error {
                session_id: active.scope.session_id.clone(),
                target_node_id: active.scope.target_node_id.clone(),
                partition_id: active.scope.partition_id.clone(),
                code: code.to_string(),
                message: message.to_string(),
            },
        );
    }

    async fn finalize_run_result(
        &self,
        state: &AppState,
        active: &ActiveRun,
        raw: String,
    ) -> Result<(), AppError> {
        match active.kind {
            RunKind::Survey => {
                self.finalize_parsed_json_run(
                    state,
                    active,
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
                    active,
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
                            &active.scope.org_id,
                            &active.scope.partition_id,
                            &active.run_id,
                            &raw,
                        )
                        .await?;
                    }
                    Ok(ConstructOutput::Blocked { reason }) => {
                        self.constructor_capture_blocked(
                            state,
                            &active.scope.org_id,
                            &active.scope.partition_id,
                            &active.run_id,
                            &raw,
                            &reason,
                        )
                        .await?;
                    }
                    Err(e) => {
                        self.finalize_parse_error(state, active, &raw, &format!("{e}"))
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
        active: &ActiveRun,
        raw: &str,
        parse: impl FnOnce(&str) -> Result<T, anyhow::Error>,
        json_label: &'static str,
    ) -> Result<(), AppError> {
        match parse(raw) {
            Ok(out) => {
                let json = serde_json::to_string(&out)
                    .map_err(|e| AppError::Internal(anyhow!("{json_label}: {e}")))?;
                state.datastore.runs().finish_success(
                    &active.scope.org_id,
                    &active.run_id,
                    json.clone(),
                    Some(raw.to_string()),
                ).await?;
                self.handle_phase_terminal(
                    state,
                    &active.scope,
                    active.kind,
                    &active.run_id,
                    serde_json::from_str(&json).ok(),
                )
                .await?;
                Ok(())
            }
            Err(e) => {
                self.finalize_parse_error(state, active, raw, &format!("{e}"))
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

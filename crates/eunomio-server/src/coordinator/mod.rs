// SPDX-License-Identifier: Apache-2.0

use eunomio_core::types::*;
use crate::{
    cursor_bridge::{RunHandle, SubagentRunner},
    AppError,
    state::AppState,
    subagents::{
        planner::{PlanOutput, PlanStrategy},
        Subagents,
    },
     
};
use eunomio_core::traits::QuotaEnforcer;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{broadcast, Mutex};

mod accept;
mod begin;
mod constructor_outcome;
mod gates;
mod prompt;
mod recovery;
mod run_loop;
mod scope;

const BROADCAST_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct Coordinator {
    inner: Arc<Inner>,
}

struct Inner {
    events: CoordinatorEvents,
    runs: RunHandleRegistry,
    subagents: Subagents,
    runner: Arc<dyn SubagentRunner>,
    quota: Arc<dyn QuotaEnforcer>,
}

/// Per-session SSE broadcast registry. Methods own the `to_string()` /
/// clone of identifiers so call sites at the coordinator can pass `&str`
/// and stay terse.
struct CoordinatorEvents {
    channels: StdMutex<HashMap<String, broadcast::Sender<SseEvent>>>,
}

impl CoordinatorEvents {
    fn new() -> Self {
        Self {
            channels: StdMutex::new(HashMap::new()),
        }
    }

    fn subscribe(&self, session_id: &str) -> broadcast::Receiver<SseEvent> {
        let mut channels = self.channels.lock().unwrap();
        channels
            .entry(session_id.to_string())
            .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
            .subscribe()
    }

    fn emit(&self, session_id: &str, event: SseEvent) {
        let tx = {
            let mut channels = self.channels.lock().unwrap();
            channels
                .entry(session_id.to_string())
                .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
                .clone()
        };
        let _ = tx.send(event);
    }
}

/// Bookkeeping for in-flight per-partition runs and the
/// "this partition is being abandoned, ignore late helper events"
/// flag set used by `drive_run`. Wraps the historical pair of
/// `Mutex<HashMap<_, _>>` + `StdMutex<HashSet<_>>` so call sites
/// don't open both locks individually.
struct RunHandleRegistry {
    handles: Mutex<HashMap<String, RunHandle>>,
    abandoning: StdMutex<HashSet<String>>,
}

impl RunHandleRegistry {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            abandoning: StdMutex::new(HashSet::new()),
        }
    }

    async fn has_in_flight(&self, partition_id: &str) -> bool {
        self.handles.lock().await.contains_key(partition_id)
    }

    async fn insert(&self, partition_id: &str, handle: RunHandle) {
        self.handles.lock().await.insert(partition_id.to_string(), handle);
    }

    async fn take_and_cancel(&self, partition_id: &str) {
        let h = self.handles.lock().await.remove(partition_id);
        if let Some(handle) = h {
            (handle.cancel)();
        }
    }

    async fn forget(&self, partition_id: &str) {
        self.handles.lock().await.remove(partition_id);
    }

    fn mark_abandoning(&self, partition_id: &str) {
        self.abandoning.lock().unwrap().insert(partition_id.to_string());
    }

    fn unmark_abandoning(&self, partition_id: &str) {
        self.abandoning.lock().unwrap().remove(partition_id);
    }

    fn is_abandoning(&self, partition_id: &str) -> bool {
        self.abandoning.lock().unwrap().contains(partition_id)
    }

    fn mark_abandoning_many(&self, ids: &[String]) {
        let mut a = self.abandoning.lock().unwrap();
        for id in ids {
            a.insert(id.clone());
        }
    }

    fn unmark_abandoning_many(&self, ids: &[String]) {
        let mut a = self.abandoning.lock().unwrap();
        for id in ids {
            a.remove(id);
        }
    }
}

impl Coordinator {
    pub fn new(
        subagents: Subagents,
        runner: Arc<dyn SubagentRunner>,
        quota: Arc<dyn QuotaEnforcer>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                events: CoordinatorEvents::new(),
                runs: RunHandleRegistry::new(),
                subagents,
                runner,
                quota,
            }),
        }
    }

    pub fn subscribe(&self, session_id: &str) -> broadcast::Receiver<SseEvent> {
        self.inner.events.subscribe(session_id)
    }

    fn emit(&self, session_id: &str, event: SseEvent) {
        self.inner.events.emit(session_id, event);
    }

    pub async fn list_runs(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
    ) -> Result<Vec<Run>, AppError> {
        let rows = state.datastore.runs().list_for_partition(org_id, partition_id).await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn list_partitions(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        target_node_id: Option<&str>,
    ) -> Result<Vec<Partition>, AppError> {
        let rows = state.datastore.partitions().list(org_id, session_id, target_node_id).await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_partition(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
    ) -> Result<Partition, AppError> {
        let row = state.datastore.partitions().get(org_id, partition_id).await?;
        Ok(row.into())
    }

    pub async fn get_transcript(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
    ) -> Result<Transcript, AppError> {
        state.datastore.partitions().get(org_id, partition_id).await?;
        let run = state.datastore.runs().get(org_id, run_id).await?;
        if run.partition_id != partition_id {
            return Err(AppError::NotFound);
        }
        let prompt = state.datastore.runs().get_prompt(org_id, run_id).await?;
        let parsed_result = run
            .result_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        Ok(Transcript {
            run_id: run.id,
            kind: run.kind,
            prompt,
            transcript_text: run.transcript_text,
            raw_result: run.result_text,
            parsed_result,
            error_message: run.error_message,
        })
    }

    pub fn default_prompts(&self) -> SubagentDefaultPrompts {
        let defs = &self.inner.subagents;
        SubagentDefaultPrompts {
            surveyor: defs.surveyor.template.body().to_string(),
            planner: defs.planner.template.body().to_string(),
            constructor: defs.constructor.template.body().to_string(),
        }
    }

    pub async fn list_models(&self, cursor_api_key: &str) -> Result<Vec<CursorModel>, AppError> {
        self.inner.runner.list_models(cursor_api_key).await
    }
}

/// Reject a request that arrived at the wrong gate. Centralises the
/// `not_at_gate` Conflict response shape used by every accept/start
/// handler so the message stays consistent.
pub(super) fn ensure_at_gate(
    row: &PartitionRow,
    expected_phase: PhaseName,
    label: &'static str,
) -> Result<(), AppError> {
    if matches!(row.phase, p if p == expected_phase)
        && matches!(row.phase_state, PhaseState::AwaitingReview)
    {
        Ok(())
    } else {
        Err(AppError::Conflict {
            code: "not_at_gate".into(),
            message: format!("partition is not at the {label} review gate"),
        })
    }
}

/// Outputs of `parse_split_plan`. Mirrors the only branch the
/// accept/construct paths actually use; the indivisible branch is
/// rejected with `AppError::BadRequest` before this is constructed.
pub(super) struct SplitPlan {
    pub strategy: PartitionStrategy,
    pub edges: Vec<crate::subagents::planner::PlanEdge>,
}

/// Parse a plan run's `result_json`, rejecting `Indivisible` with the
/// same error shape used at every consumer (`do_accept_plan`,
/// `do_accept_construct`, `build_prompt`, `constructor_capture_ok`).
pub(super) fn parse_split_plan(plan_json: &str) -> Result<SplitPlan, AppError> {
    let plan: PlanOutput = serde_json::from_str(plan_json)
        .map_err(|e| AppError::BadRequest(format!("invalid plan: {e}")))?;
    match plan {
        PlanOutput::Split { strategy, edges, .. } => Ok(SplitPlan {
            strategy: match strategy {
                PlanStrategy::Synthetic => PartitionStrategy::Synthetic,
                PlanStrategy::Vertical => PartitionStrategy::Vertical,
                PlanStrategy::Horizontal => PartitionStrategy::Horizontal,
            },
            edges,
        }),
        PlanOutput::Indivisible { .. } => Err(AppError::BadRequest(
            "plan is indivisible; cannot accept".into(),
        )),
    }
}

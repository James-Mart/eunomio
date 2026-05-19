use crate::{error::AppError, types::*};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;

const BROADCAST_CAPACITY: usize = 64;

/// Per-process registry of mock partitions and their session-scoped SSE fan-outs.
#[derive(Clone, Default)]
pub struct MockRegistry {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    runs: Mutex<HashMap<String, MockRun>>,
    channels: Mutex<HashMap<String, broadcast::Sender<SseEvent>>>,
}

struct MockRun {
    target_node_id: String,
    gate: Arc<GateSignal>,
    task: JoinHandle<()>,
}

#[derive(Default)]
struct GateSignal {
    notify: Notify,
    decision: Mutex<Option<GateDecision>>,
}

#[derive(Clone, Copy)]
enum GateDecision {
    Continue,
    Rerun,
}

impl MockRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, session_id: &str) -> broadcast::Receiver<SseEvent> {
        let mut channels = self.inner.channels.lock().unwrap();
        channels
            .entry(session_id.to_string())
            .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
            .subscribe()
    }

    pub fn start(
        &self,
        session_id: String,
        target_node_id: String,
        strategy: PartitionStrategy,
        user_concern: Option<String>,
        hitl: HumanInTheLoopDto,
        started_at: i64,
    ) -> Result<MockPartitionDto, AppError> {
        let mut runs = self.inner.runs.lock().unwrap();
        if runs.contains_key(&session_id) {
            return Err(AppError::Conflict {
                code: "partition_in_flight".into(),
                message: "another partition is already in flight for this session".into(),
            });
        }
        let tx = {
            let mut channels = self.inner.channels.lock().unwrap();
            channels
                .entry(session_id.clone())
                .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0)
                .clone()
        };
        let gate = Arc::new(GateSignal::default());
        let task = tokio::spawn(script(
            session_id.clone(),
            target_node_id.clone(),
            strategy,
            user_concern.clone(),
            hitl,
            tx,
            gate.clone(),
        ));
        runs.insert(
            session_id.clone(),
            MockRun {
                target_node_id: target_node_id.clone(),
                gate,
                task,
            },
        );
        Ok(MockPartitionDto {
            session_id,
            target_node_id,
            strategy,
            user_concern,
            started_at,
        })
    }

    pub fn continue_(&self, session_id: &str, target_node_id: &str) -> Result<(), AppError> {
        let runs = self.inner.runs.lock().unwrap();
        let run = runs.get(session_id).ok_or_else(|| AppError::Conflict {
            code: "no_partition".into(),
            message: "no partition in flight for this session".into(),
        })?;
        if run.target_node_id != target_node_id {
            return Err(AppError::NotFound);
        }
        run.gate.signal(GateDecision::Continue);
        Ok(())
    }

    pub fn rerun(
        &self,
        session_id: &str,
        target_node_id: &str,
        _user_feedback: Option<String>,
    ) -> Result<(), AppError> {
        let runs = self.inner.runs.lock().unwrap();
        let run = runs.get(session_id).ok_or_else(|| AppError::Conflict {
            code: "no_partition".into(),
            message: "no partition in flight for this session".into(),
        })?;
        if run.target_node_id != target_node_id {
            return Err(AppError::NotFound);
        }
        if !run.gate.is_parked() {
            return Err(AppError::Conflict {
                code: "not_at_gate".into(),
                message: "partition is not currently parked at a review gate".into(),
            });
        }
        run.gate.signal(GateDecision::Rerun);
        Ok(())
    }

    pub fn abandon(&self, session_id: &str, target_node_id: &str) -> Result<(), AppError> {
        let run = {
            let mut runs = self.inner.runs.lock().unwrap();
            let Some(run) = runs.get(session_id) else {
                return Err(AppError::NotFound);
            };
            if run.target_node_id != target_node_id {
                return Err(AppError::NotFound);
            }
            runs.remove(session_id).unwrap()
        };
        run.task.abort();
        let tx = self
            .inner
            .channels
            .lock()
            .unwrap()
            .get(session_id)
            .cloned();
        if let Some(tx) = tx {
            let _ = tx.send(SseEvent::Cancelled {
                session_id: session_id.to_string(),
                target_node_id: target_node_id.to_string(),
            });
        }
        Ok(())
    }
}

impl GateSignal {
    fn is_parked(&self) -> bool {
        self.decision.lock().unwrap().is_none()
    }

    fn signal(&self, d: GateDecision) {
        let mut slot = self.decision.lock().unwrap();
        if slot.is_none() {
            *slot = Some(d);
            self.notify.notify_one();
        }
    }

    async fn wait(&self) -> GateDecision {
        loop {
            if let Some(d) = self.decision.lock().unwrap().take() {
                return d;
            }
            self.notify.notified().await;
        }
    }
}

async fn script(
    session_id: String,
    target_node_id: String,
    strategy: PartitionStrategy,
    user_concern: Option<String>,
    hitl: HumanInTheLoopDto,
    tx: broadcast::Sender<SseEvent>,
    gate: Arc<GateSignal>,
) {
    let _ = tx.send(SseEvent::Started {
        session_id: session_id.clone(),
        target_node_id: target_node_id.clone(),
        strategy,
        user_concern: user_concern.clone(),
    });

    loop {
        if !run_phase(
            &tx,
            &session_id,
            &target_node_id,
            PhaseName::Survey,
            Some(canned_survey()),
        )
        .await
        {
            return;
        }
        if hitl.after_survey {
            let _ = tx.send(SseEvent::Phase {
                session_id: session_id.clone(),
                target_node_id: target_node_id.clone(),
                name: PhaseName::Survey,
                state: PhaseState::AwaitingReview,
                payload: Some(canned_survey()),
            });
            match gate.wait().await {
                GateDecision::Continue => {}
                GateDecision::Rerun => continue,
            }
        }
        break;
    }

    loop {
        if !run_phase(
            &tx,
            &session_id,
            &target_node_id,
            PhaseName::Plan,
            Some(canned_plan()),
        )
        .await
        {
            return;
        }
        if hitl.after_planning {
            let _ = tx.send(SseEvent::Phase {
                session_id: session_id.clone(),
                target_node_id: target_node_id.clone(),
                name: PhaseName::Plan,
                state: PhaseState::AwaitingReview,
                payload: Some(canned_plan()),
            });
            match gate.wait().await {
                GateDecision::Continue => {}
                GateDecision::Rerun => continue,
            }
        }
        break;
    }

    let _ = tx.send(SseEvent::Phase {
        session_id: session_id.clone(),
        target_node_id: target_node_id.clone(),
        name: PhaseName::Construct,
        state: PhaseState::Running,
        payload: None,
    });
    for item_id in ["item-1", "item-2", "item-3"] {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let _ = tx.send(SseEvent::LoopProgress {
            session_id: session_id.clone(),
            target_node_id: target_node_id.clone(),
            item_id: item_id.into(),
            status: "ok".into(),
        });
    }
    let _ = tx.send(SseEvent::Phase {
        session_id: session_id.clone(),
        target_node_id: target_node_id.clone(),
        name: PhaseName::Construct,
        state: PhaseState::Done,
        payload: None,
    });
    let _ = tx.send(SseEvent::Finished {
        session_id,
        target_node_id,
    });
}

async fn run_phase(
    tx: &broadcast::Sender<SseEvent>,
    session_id: &str,
    target_node_id: &str,
    name: PhaseName,
    payload: Option<serde_json::Value>,
) -> bool {
    if tx
        .send(SseEvent::Phase {
            session_id: session_id.to_string(),
            target_node_id: target_node_id.to_string(),
            name,
            state: PhaseState::Running,
            payload: None,
        })
        .is_err()
    {
        return false;
    }
    tokio::time::sleep(Duration::from_millis(600)).await;
    let _ = tx.send(SseEvent::SdkMessage {
        session_id: session_id.to_string(),
        target_node_id: target_node_id.to_string(),
        message: json!({
            "kind": "assistant",
            "text": format!("working on {name:?}…"),
        }),
    });
    tokio::time::sleep(Duration::from_millis(800)).await;
    let _ = tx.send(SseEvent::Phase {
        session_id: session_id.to_string(),
        target_node_id: target_node_id.to_string(),
        name,
        state: PhaseState::Done,
        payload,
    });
    true
}

fn canned_survey() -> serde_json::Value {
    json!({
        "summary": "Three independent concerns are tangled in this diff: a config-loader \
                    refactor, a CLI-flag addition, and a new error variant for SDK failures.",
        "concerns": [
            {
                "id": "config-loader",
                "title": "Extract config loader into its own module",
                "description": "The serialization/deserialization of partition settings moved from `lib.rs` into a dedicated module.",
                "paths": ["backend/src/config.rs", "backend/src/lib.rs"],
            },
            {
                "id": "cli-flag",
                "title": "Add --cursor-api-key flag",
                "description": "A new optional flag exposes the Cursor SDK auth key without requiring an env var.",
                "paths": ["backend/src/main.rs"],
            },
            {
                "id": "sdk-error",
                "title": "AppError variant for unrecoverable SDK failures",
                "description": "503 responses now carry a `code` so the frontend can show a sticky banner.",
                "paths": ["backend/src/error.rs"],
            }
        ],
    })
}

fn canned_plan() -> serde_json::Value {
    json!({
        "items": [
            { "id": "item-1", "title": "Extract config loader",         "concernIds": ["config-loader"] },
            { "id": "item-2", "title": "Add --cursor-api-key CLI flag", "concernIds": ["cli-flag"] },
            { "id": "item-3", "title": "Add Unrecoverable error variant","concernIds": ["sdk-error"] },
        ],
    })
}

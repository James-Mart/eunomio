use crate::{error::AppError, repo, state::AppState, types::*};

use super::Coordinator;

impl Coordinator {
    /// Decide what happens when a run finishes successfully: park at a
    /// review gate (HITL on) or auto-advance to the next phase (HITL off).
    /// `Plan + indivisible` short-circuits both branches with its own
    /// review-or-abandon split.
    pub(super) async fn handle_phase_terminal(
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

        if kind == RunKind::Plan && is_indivisible(payload.as_ref()) {
            return self
                .on_indivisible_plan(state, partition_id, session_id, target_node_id, payload, hitl)
                .await;
        }

        let gate = gate_for(kind, hitl);
        if gate {
            self.park_at_gate(state, partition_id, kind, session_id, target_node_id, payload)
                .await
        } else {
            self.auto_advance(state, partition_id, run_id, kind);
            Ok(())
        }
    }

    async fn on_indivisible_plan(
        &self,
        state: &AppState,
        partition_id: i64,
        session_id: &str,
        target_node_id: &str,
        payload: Option<serde_json::Value>,
        hitl: HumanInTheLoop,
    ) -> Result<(), AppError> {
        if hitl.after_indivisible {
            self.park_at_gate(
                state,
                partition_id,
                RunKind::Plan,
                session_id,
                target_node_id,
                payload,
            )
            .await
        } else {
            let coord = self.clone();
            let state_owned = state.clone();
            tokio::spawn(async move {
                if let Err(e) = coord.abandon_partition(&state_owned, partition_id).await {
                    tracing::error!(error = %e, partition_id, "auto-abandon on indivisible failed");
                }
            });
            Ok(())
        }
    }

    async fn park_at_gate(
        &self,
        state: &AppState,
        partition_id: i64,
        kind: RunKind,
        session_id: &str,
        target_node_id: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        repo::partition::set_phase_state(state, partition_id, PhaseState::AwaitingReview).await?;
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
        Ok(())
    }

    fn auto_advance(&self, state: &AppState, partition_id: i64, run_id: i64, kind: RunKind) {
        let coord = self.clone();
        let state_owned = state.clone();
        tokio::spawn(async move {
            let res = match kind {
                RunKind::Survey => coord
                    .do_accept_survey(&state_owned, partition_id, run_id)
                    .await
                    .map(|_| ()),
                RunKind::Plan => coord
                    .do_accept_plan(&state_owned, partition_id, run_id)
                    .await
                    .map(|_| ()),
                RunKind::Construct => coord.do_accept_construct(&state_owned, partition_id).await,
            };
            if let Err(e) = res {
                tracing::error!(error = %e, partition_id, "auto-accept failed");
            }
        });
    }
}

fn gate_for(kind: RunKind, hitl: HumanInTheLoop) -> bool {
    match kind {
        RunKind::Survey => hitl.after_survey,
        RunKind::Plan => hitl.after_planning,
        RunKind::Construct => hitl.after_construct,
    }
}

fn is_indivisible(payload: Option<&serde_json::Value>) -> bool {
    payload
        .and_then(|p| p.get("outcome"))
        .and_then(|v| v.as_str())
        == Some("indivisible")
}

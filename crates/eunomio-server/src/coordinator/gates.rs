// SPDX-License-Identifier: Apache-2.0

use crate::{partition_settings::load_for_partition, state::AppState, AppError};
use eunomio_core::types::*;

use super::{scope::PhaseScope, Coordinator};

impl Coordinator {
    /// Decide what happens when a run finishes successfully: park at a
    /// review gate (HITL on) or auto-advance to the next phase (HITL off).
    /// `Plan + indivisible` short-circuits both branches with its own
    /// review-or-abandon split.
    pub(super) async fn handle_phase_terminal(
        &self,
        state: &AppState,
        scope: &PhaseScope,
        kind: RunKind,
        run_id: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let settings = load_for_partition(state, &scope.org_id, &scope.session_id).await?;
        let hitl = settings.coordinator.human_in_the_loop;

        if kind == RunKind::Plan && is_indivisible(payload.as_ref()) {
            return self.on_indivisible_plan(state, scope, payload, hitl).await;
        }

        let gate = gate_for(kind, hitl);
        if gate {
            self.park_at_gate(state, scope, kind, payload).await
        } else {
            self.auto_advance(state, &scope.org_id, &scope.partition_id, run_id, kind);
            Ok(())
        }
    }

    async fn on_indivisible_plan(
        &self,
        state: &AppState,
        scope: &PhaseScope,
        payload: Option<serde_json::Value>,
        hitl: HumanInTheLoop,
    ) -> Result<(), AppError> {
        if hitl.after_indivisible {
            self.park_at_gate(state, scope, RunKind::Plan, payload)
                .await
        } else {
            state
                .datastore
                .partitions()
                .set_phase_state(
                    &scope.org_id,
                    &scope.partition_id,
                    PhaseState::AwaitingReview,
                )
                .await?;
            let coord = self.clone();
            let state_owned = state.clone();
            let org_id = scope.org_id.clone();
            let partition_id = scope.partition_id.clone();
            tokio::spawn(async move {
                if let Err(e) = coord
                    .finish_partition(&state_owned, &org_id, &partition_id)
                    .await
                {
                    tracing::error!(error = %e, partition_id = %partition_id, "auto-finish on indivisible failed");
                }
            });
            Ok(())
        }
    }

    async fn park_at_gate(
        &self,
        state: &AppState,
        scope: &PhaseScope,
        kind: RunKind,
        payload: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        state
            .datastore
            .partitions()
            .set_phase_state(
                &scope.org_id,
                &scope.partition_id,
                PhaseState::AwaitingReview,
            )
            .await?;
        self.emit(
            &scope.session_id,
            SseEvent::Phase {
                session_id: scope.session_id.clone(),
                target_node_id: scope.target_node_id.clone(),
                partition_id: scope.partition_id.clone(),
                name: kind.phase(),
                state: PhaseState::AwaitingReview,
                payload,
            },
        );
        Ok(())
    }

    fn auto_advance(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
        run_id: &str,
        kind: RunKind,
    ) {
        let coord = self.clone();
        let state_owned = state.clone();
        let org_id = org_id.to_string();
        let partition_id = partition_id.to_string();
        let run_id = run_id.to_string();
        tokio::spawn(async move {
            let res = match kind {
                RunKind::Survey => coord
                    .do_accept_survey(&state_owned, &org_id, &partition_id, &run_id)
                    .await
                    .map(|_| ()),
                RunKind::Plan => coord
                    .do_accept_plan(&state_owned, &org_id, &partition_id, &run_id)
                    .await
                    .map(|_| ()),
                RunKind::Construct => {
                    coord
                        .do_accept_construct(&state_owned, &org_id, &partition_id)
                        .await
                }
            };
            if let Err(e) = res {
                tracing::error!(error = %e, partition_id = %partition_id, "auto-accept failed");
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

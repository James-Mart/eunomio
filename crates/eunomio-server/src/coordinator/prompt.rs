// SPDX-License-Identifier: Apache-2.0

use eunomio_core::types::*;
use crate::{
    AppError,
    state::AppState,
    subagents::{
        self,
        loader::{
            constructor_placeholders, planner_placeholders, resolve_prompt_template,
            surveyor_placeholders,
        },
        planner::PriorAttempt,
        PromptTemplate,
    },
     
};
use serde::Deserialize;

use super::{parse_split_plan, Coordinator};

#[derive(Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
enum ConstructResultJson {
    Ok,
    Blocked {
        #[serde(default)]
        reason: String,
    },
}

impl Coordinator {
    pub(super) async fn build_prompt(
        &self,
        state: &AppState,
        partition: &PartitionRow,
        kind: RunKind,
        user_feedback: Option<&str>,
        strategy_override: Option<PartitionStrategy>,
        prompt_override: Option<&str>,
    ) -> Result<String, AppError> {
        let (target_node, parent_node) = state.datastore.nodes().target_and_parent(&partition.org_id, &partition.session_id, &partition.target_node_id,
        ).await?;
        let parent =
            parent_node.ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
        let before_tree = parent.tree_sha.clone();
        let target_tree = target_node.tree_sha.clone();

        let prompt = match kind {
            RunKind::Survey => {
                let template = self.resolve_template(RunKind::Survey, prompt_override)?;
                self.survey_prompt(before_tree, target_tree, user_feedback, &template)
            }
            RunKind::Plan => {
                let template = self.resolve_template(RunKind::Plan, prompt_override)?;
                self.plan_prompt(
                    state,
                    partition,
                    parent.commit_sha.clone(),
                    before_tree,
                    target_tree,
                    user_feedback,
                    strategy_override,
                    &template,
                )
                .await?
            }
            RunKind::Construct => {
                let template = self.resolve_template(RunKind::Construct, prompt_override)?;
                self.construct_prompt(
                    partition,
                    parent.commit_sha.clone(),
                    before_tree,
                    target_tree,
                    user_feedback,
                    &template,
                )?
            }
        };
        Ok(prompt)
    }

    pub(super) fn resolve_template(
        &self,
        kind: RunKind,
        prompt_override: Option<&str>,
    ) -> Result<PromptTemplate, AppError> {
        let defs = &self.inner.subagents;
        let (default, allowed) = match kind {
            RunKind::Survey => (&defs.surveyor.template, surveyor_placeholders()),
            RunKind::Plan => (&defs.planner.template, planner_placeholders()),
            RunKind::Construct => (&defs.constructor.template, constructor_placeholders()),
        };
        resolve_prompt_template(default, allowed, prompt_override)
            .map_err(|e| AppError::BadRequest(e.to_string()))
    }

    fn survey_prompt(
        &self,
        before_tree: String,
        target_tree: String,
        user_feedback: Option<&str>,
        template: &PromptTemplate,
    ) -> String {
        let ctx = subagents::surveyor::SurveyContext {
            before_tree,
            target_tree,
            user_feedback: user_feedback.unwrap_or("").to_string(),
        };
        subagents::surveyor::render_prompt(&ctx, template)
    }

    #[allow(clippy::too_many_arguments)]
    async fn plan_prompt(
        &self,
        state: &AppState,
        partition: &PartitionRow,
        parent_commit: String,
        before_tree: String,
        target_tree: String,
        user_feedback: Option<&str>,
        strategy_override: Option<PartitionStrategy>,
        template: &PromptTemplate,
    ) -> Result<String, AppError> {
        let survey_json = partition
            .change_survey_json
            .clone()
            .unwrap_or_else(|| "{}".to_string());
        let strategy_override_str = strategy_override
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "auto".to_string());
        let prior_attempt = self
            .lookup_prior_attempt(state, &partition.org_id, partition.id.as_str())
            .await?;
        let ctx = subagents::planner::PlanContext {
            parent_commit,
            before_tree,
            target_tree,
            change_survey_json: survey_json,
            strategy_override: strategy_override_str,
            user_feedback: user_feedback.unwrap_or("").to_string(),
            prior_attempt,
        };
        Ok(subagents::planner::render_prompt(&ctx, template))
    }

    fn construct_prompt(
        &self,
        partition: &PartitionRow,
        parent_commit: String,
        before_tree: String,
        target_tree: String,
        user_feedback: Option<&str>,
        template: &PromptTemplate,
    ) -> Result<String, AppError> {
        let plan_json = partition
            .plan_json
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("no plan accepted".into()))?;
        let split = parse_split_plan(plan_json).map_err(|e| match e {
            AppError::BadRequest(_) => {
                AppError::BadRequest("cannot run constructor: plan is indivisible".into())
            }
            other => other,
        })?;
        let strategy = partition
            .strategy
            .ok_or_else(|| AppError::BadRequest("no strategy on partition".into()))?;
        let ctx = subagents::constructor::ConstructContext {
            parent_commit,
            before_tree: before_tree.clone(),
            target_tree,
            strategy: strategy.as_str().to_string(),
            slice_title: split.edges[0].title.clone(),
            slice_description: split.edges[0].description.clone(),
            user_feedback: user_feedback.unwrap_or("").to_string(),
        };
        Ok(subagents::constructor::render_prompt(&ctx, template))
    }

    async fn lookup_prior_attempt(
        &self,
        state: &AppState,
        org_id: &str,
        partition_id: &str,
    ) -> Result<Option<PriorAttempt>, AppError> {
        let runs = state.datastore.runs().list_for_partition(org_id, partition_id).await?;
        let last_construct = runs.iter().find(|r| {
            r.kind == RunKind::Construct
                && matches!(r.status, RunStatus::Finished | RunStatus::Error)
        });
        let Some(construct_run) = last_construct else {
            return Ok(None);
        };
        let Some(json) = construct_run.result_json.as_deref() else {
            return Ok(None);
        };
        let Ok(parsed) = serde_json::from_str::<ConstructResultJson>(json) else {
            return Ok(None);
        };
        match parsed {
            ConstructResultJson::Blocked { reason } => Ok(Some(PriorAttempt::Blocked { reason })),
            ConstructResultJson::Ok => Ok(last_split_plan_edge_zero(&runs)
                .map(|edge| PriorAttempt::Candidate {
                    slice_title: edge.title,
                    slice_description: edge.description,
                })),
        }
    }
}

fn last_split_plan_edge_zero(
    runs: &[RunRow],
) -> Option<crate::subagents::planner::PlanEdge> {
    let plan_run = runs
        .iter()
        .find(|r| r.kind == RunKind::Plan && r.status == RunStatus::Finished)?;
    let plan_json = plan_run.result_json.as_deref()?;
    match serde_json::from_str(plan_json).ok()? {
        crate::subagents::planner::PlanOutput::Split { edges, .. } if !edges.is_empty() => {
            Some(edges[0].clone())
        }
        _ => None,
    }
}

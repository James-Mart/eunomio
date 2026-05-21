use crate::{
    error::AppError,
    repo,
    state::AppState,
    subagents::{
        self,
        planner::PriorAttempt,
    },
    types::*,
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
    ) -> Result<String, AppError> {
        let (target_node, parent_node) = repo::node::target_and_parent(
            state,
            &partition.session_id,
            &partition.target_node_id,
        )
        .await?;
        let parent =
            parent_node.ok_or_else(|| AppError::BadRequest("no parent node".into()))?;
        let before_tree = parent.tree_sha.clone();
        let target_tree = target_node.tree_sha.clone();

        let prompt = match kind {
            RunKind::Survey => self.survey_prompt(before_tree, target_tree, user_feedback),
            RunKind::Plan => {
                self.plan_prompt(
                    state,
                    partition,
                    before_tree,
                    target_tree,
                    user_feedback,
                    strategy_override,
                )
                .await?
            }
            RunKind::Construct => self.construct_prompt(
                partition,
                before_tree,
                target_tree,
                user_feedback,
            )?,
        };
        Ok(prompt)
    }

    fn survey_prompt(
        &self,
        before_tree: String,
        target_tree: String,
        user_feedback: Option<&str>,
    ) -> String {
        let ctx = subagents::surveyor::SurveyContext {
            before_tree,
            target_tree,
            user_feedback: user_feedback.unwrap_or("").to_string(),
        };
        subagents::surveyor::render_prompt(&ctx, &self.inner.subagents)
    }

    async fn plan_prompt(
        &self,
        state: &AppState,
        partition: &PartitionRow,
        before_tree: String,
        target_tree: String,
        user_feedback: Option<&str>,
        strategy_override: Option<PartitionStrategy>,
    ) -> Result<String, AppError> {
        let survey_json = partition
            .change_survey_json
            .clone()
            .unwrap_or_else(|| "{}".to_string());
        let strategy_override_str = strategy_override
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "auto".to_string());
        let prior_attempt = self.lookup_prior_attempt(state, partition.id).await?;
        let ctx = subagents::planner::PlanContext {
            before_tree,
            target_tree,
            change_survey_json: survey_json,
            strategy_override: strategy_override_str,
            user_feedback: user_feedback.unwrap_or("").to_string(),
            prior_attempt,
        };
        Ok(subagents::planner::render_prompt(&ctx, &self.inner.subagents))
    }

    fn construct_prompt(
        &self,
        partition: &PartitionRow,
        before_tree: String,
        target_tree: String,
        user_feedback: Option<&str>,
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
            before_tree: before_tree.clone(),
            target_tree,
            worktree_head_tree: before_tree,
            strategy: strategy.as_str().to_string(),
            slice_title: split.edges[0].title.clone(),
            slice_description: split.edges[0].description.clone(),
            user_feedback: user_feedback.unwrap_or("").to_string(),
        };
        Ok(subagents::constructor::render_prompt(&ctx, &self.inner.subagents))
    }

    async fn lookup_prior_attempt(
        &self,
        state: &AppState,
        partition_id: i64,
    ) -> Result<Option<PriorAttempt>, AppError> {
        let runs = repo::run::list_for_partition(state, partition_id).await?;
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

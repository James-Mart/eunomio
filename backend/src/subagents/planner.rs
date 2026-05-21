use crate::subagents::loader::{ParseError, PromptTemplate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum PlanOutput {
    #[serde(rename_all = "camelCase")]
    Split {
        strategy: PlanStrategy,
        strategy_rationale: String,
        edges: Vec<PlanEdge>,
    },
    #[serde(rename_all = "camelCase")]
    Indivisible { rationale: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlanStrategy {
    Synthetic,
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEdge {
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum PriorAttempt {
    Blocked {
        reason: String,
    },
    Candidate {
        slice_title: String,
        slice_description: String,
    },
}

pub struct PlanContext {
    pub before_tree: String,
    pub target_tree: String,
    pub change_survey_json: String,
    pub strategy_override: String,
    pub user_feedback: String,
    pub prior_attempt: Option<PriorAttempt>,
}

pub fn render_prompt(ctx: &PlanContext, template: &PromptTemplate) -> String {
    let mut map = serde_json::Map::new();
    map.insert("BEFORE_TREE".into(), serde_json::json!(ctx.before_tree));
    map.insert("TARGET_TREE".into(), serde_json::json!(ctx.target_tree));
    map.insert(
        "CHANGE_SURVEY_JSON".into(),
        serde_json::json!(ctx.change_survey_json),
    );
    map.insert(
        "STRATEGY_OVERRIDE".into(),
        serde_json::json!(ctx.strategy_override),
    );
    map.insert(
        "USER_FEEDBACK".into(),
        serde_json::json!(if ctx.user_feedback.trim().is_empty() {
            "(none)".to_string()
        } else {
            ctx.user_feedback.clone()
        }),
    );
    map.insert(
        "PRIOR_BLOCK_OR_CANDIDATE".into(),
        serde_json::json!(format_prior_attempt(ctx.prior_attempt.as_ref())),
    );
    template.render(&map)
}

fn format_prior_attempt(prior: Option<&PriorAttempt>) -> String {
    match prior {
        None => "(none)".to_string(),
        Some(PriorAttempt::Blocked { reason }) => {
            format!("Previous Construct attempt was BLOCKED with reason: {reason}")
        }
        Some(PriorAttempt::Candidate {
            slice_title,
            slice_description,
        }) => format!(
            "Previous attempt produced this slice (the user has asked for a different slice; see USER_FEEDBACK):\n  title: {slice_title}\n  description: {slice_description}"
        ),
    }
}

pub fn parse_output(raw: &str) -> Result<PlanOutput, ParseError> {
    let block = extract_json_block(raw)
        .ok_or_else(|| ParseError::Malformed("no fenced ```json``` block found".into()))?;
    let parsed: PlanOutput = serde_json::from_str(&block)
        .map_err(|e| ParseError::Malformed(format!("invalid JSON: {e}")))?;
    match &parsed {
        PlanOutput::Split { edges, .. } => {
            if edges.len() != 2 {
                return Err(ParseError::Malformed(format!(
                    "split plan must have exactly 2 edges, got {}",
                    edges.len()
                )));
            }
        }
        PlanOutput::Indivisible { .. } => {}
    }
    Ok(parsed)
}

fn extract_json_block(raw: &str) -> Option<String> {
    let re =
        regex::Regex::new(r"(?s)```(?:json)?\s*\n(.*?)\n```").expect("valid json fence regex");
    if let Some(cap) = re.captures(raw) {
        return Some(cap.get(1)?.as_str().to_string());
    }
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::loader::load_subagents;

    #[test]
    fn renders_with_context() {
        let defs = load_subagents().unwrap();
        let out = render_prompt(
            &PlanContext {
                before_tree: "before".into(),
                target_tree: "target".into(),
                change_survey_json: "{\"summary\":\"x\",\"themes\":[]}".into(),
                strategy_override: "auto".into(),
                user_feedback: "".into(),
                prior_attempt: None,
            },
            &defs.planner.template,
        );
        assert!(out.contains("before"));
        assert!(out.contains("target"));
        assert!(out.contains("auto"));
        assert!(out.contains("(none)"));
    }

    #[test]
    fn renders_prior_blocked() {
        let defs = load_subagents().unwrap();
        let out = render_prompt(
            &PlanContext {
                before_tree: "b".into(),
                target_tree: "t".into(),
                change_survey_json: "{}".into(),
                strategy_override: "auto".into(),
                user_feedback: "".into(),
                prior_attempt: Some(PriorAttempt::Blocked {
                    reason: "needs leftover hunks".into(),
                }),
            },
            &defs.planner.template,
        );
        assert!(out.contains("BLOCKED"));
        assert!(out.contains("needs leftover hunks"));
    }

    #[test]
    fn parses_split_two_edges() {
        let raw = r#"```json
{
  "outcome": "split",
  "strategy": "synthetic",
  "strategyRationale": "the diff has two clearly separated concerns",
  "edges": [
    { "id": "slice", "title": "Add config loader", "description": "Extract loader." },
    { "id": "leftover", "title": "Wire it up", "description": "Use loader." }
  ]
}
```"#;
        let parsed = parse_output(raw).unwrap();
        match parsed {
            PlanOutput::Split { strategy, edges, .. } => {
                assert_eq!(strategy, PlanStrategy::Synthetic);
                assert_eq!(edges.len(), 2);
                assert_eq!(edges[0].id, "slice");
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn rejects_split_with_one_edge() {
        let raw = r#"```json
{
  "outcome": "split",
  "strategy": "vertical",
  "strategyRationale": "x",
  "edges": [
    { "id": "only", "title": "x", "description": "y" }
  ]
}
```"#;
        let err = parse_output(raw).unwrap_err();
        match err {
            ParseError::Malformed(msg) => assert!(msg.contains("exactly 2")),
        }
    }

    #[test]
    fn parses_indivisible() {
        let raw = r#"```json
{
  "outcome": "indivisible",
  "rationale": "the diff is one tight refactor"
}
```"#;
        let parsed = parse_output(raw).unwrap();
        match parsed {
            PlanOutput::Indivisible { rationale } => {
                assert!(rationale.contains("tight refactor"));
            }
            _ => panic!("expected indivisible"),
        }
    }
}

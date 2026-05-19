use crate::subagents::loader::{ParseError, Subagents};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOutput {
    pub strategy: PlanStrategy,
    pub strategy_rationale: String,
    pub edges: Vec<PlanEdge>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlanStrategy {
    Semantic,
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

pub struct PlanContext {
    pub before_tree: String,
    pub target_tree: String,
    pub change_survey_json: String,
    pub strategy_override: String,
    pub user_feedback: String,
    pub prior_block_or_candidate: String,
}

pub fn render_prompt(ctx: &PlanContext, defs: &Subagents) -> String {
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
        serde_json::json!(if ctx.prior_block_or_candidate.trim().is_empty() {
            "(none)".to_string()
        } else {
            ctx.prior_block_or_candidate.clone()
        }),
    );
    defs.planner.template.render(&map)
}

pub fn parse_output(raw: &str) -> Result<PlanOutput, ParseError> {
    let block = extract_json_block(raw)
        .ok_or_else(|| ParseError::Malformed("no fenced ```json``` block found".into()))?;
    let parsed: PlanOutput = serde_json::from_str(&block)
        .map_err(|e| ParseError::Malformed(format!("invalid JSON: {e}")))?;
    if parsed.edges.len() != 2 {
        return Err(ParseError::Malformed(format!(
            "plan must have exactly 2 edges, got {}",
            parsed.edges.len()
        )));
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
                prior_block_or_candidate: "".into(),
            },
            &defs,
        );
        assert!(out.contains("before"));
        assert!(out.contains("target"));
        assert!(out.contains("auto"));
        assert!(out.contains("(none)"));
    }

    #[test]
    fn parses_two_edges() {
        let raw = r#"```json
{
  "strategy": "semantic",
  "strategyRationale": "the diff has two clearly separated concerns",
  "edges": [
    { "id": "slice", "title": "Add config loader", "description": "Extract loader." },
    { "id": "leftover", "title": "Wire it up", "description": "Use loader." }
  ]
}
```"#;
        let parsed = parse_output(raw).unwrap();
        assert_eq!(parsed.strategy, PlanStrategy::Semantic);
        assert_eq!(parsed.edges.len(), 2);
        assert_eq!(parsed.edges[0].id, "slice");
    }

    #[test]
    fn rejects_one_edge_plan() {
        let raw = r#"```json
{
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
}

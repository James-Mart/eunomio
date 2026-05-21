use crate::subagents::loader::{ParseError, PromptTemplate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum ConstructOutput {
    #[serde(rename_all = "camelCase")]
    Ok,
    #[serde(rename_all = "camelCase")]
    Blocked { reason: String },
}

pub struct ConstructContext {
    pub before_tree: String,
    pub target_tree: String,
    pub strategy: String,
    pub slice_title: String,
    pub slice_description: String,
    pub user_feedback: String,
}

pub fn render_prompt(ctx: &ConstructContext, template: &PromptTemplate) -> String {
    let mut map = serde_json::Map::new();
    map.insert("BEFORE_TREE".into(), serde_json::json!(ctx.before_tree));
    map.insert("TARGET_TREE".into(), serde_json::json!(ctx.target_tree));
    map.insert("STRATEGY".into(), serde_json::json!(ctx.strategy));
    map.insert("SLICE_TITLE".into(), serde_json::json!(ctx.slice_title));
    map.insert(
        "SLICE_DESCRIPTION".into(),
        serde_json::json!(ctx.slice_description),
    );
    map.insert(
        "USER_FEEDBACK".into(),
        serde_json::json!(if ctx.user_feedback.trim().is_empty() {
            "(none)".to_string()
        } else {
            ctx.user_feedback.clone()
        }),
    );
    template.render(&map)
}

pub fn parse_output(raw: &str) -> Result<ConstructOutput, ParseError> {
    let line = pick_outcome_line(raw)
        .ok_or_else(|| ParseError::Malformed("no OK / BLOCKED line found".into()))?;
    if line == "OK" {
        return Ok(ConstructOutput::Ok);
    }
    if let Some(reason) = line.strip_prefix("BLOCKED:") {
        return Ok(ConstructOutput::Blocked {
            reason: reason.trim().to_string(),
        });
    }
    Err(ParseError::Malformed(format!(
        "expected `OK` or `BLOCKED: <reason>`, got: {line}"
    )))
}

fn pick_outcome_line(raw: &str) -> Option<String> {
    for line in raw.lines().rev() {
        let l = line.trim();
        if l == "OK" || l.starts_with("BLOCKED:") {
            return Some(l.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::loader::load_subagents;

    #[test]
    fn renders_with_context() {
        let defs = load_subagents().unwrap();
        let out = render_prompt(
            &ConstructContext {
                before_tree: "B".into(),
                target_tree: "T".into(),
                strategy: "synthetic".into(),
                slice_title: "Add loader".into(),
                slice_description: "Extracts the loader module".into(),
                user_feedback: "".into(),
            },
            &defs.constructor.template,
        );
        assert!(out.contains("Add loader"));
        assert!(out.contains("synthetic"));
        assert!(out.contains("(none)"));
        assert!(out.contains("BEFORE_TREE:"));
        assert!(out.contains("git rev-parse HEAD^{tree}"));
        assert!(!out.contains("WORKTREE_HEAD_TREE"));
        assert!(!out.contains("If (none) is non-empty"));
    }

    #[test]
    fn parses_ok() {
        assert!(matches!(parse_output("OK"), Ok(ConstructOutput::Ok)));
        assert!(matches!(
            parse_output("did the thing\nOK\n"),
            Ok(ConstructOutput::Ok),
        ));
    }

    #[test]
    fn parses_blocked() {
        match parse_output("BLOCKED: would need hunks from another theme").unwrap() {
            ConstructOutput::Blocked { reason } => assert!(reason.contains("hunks")),
            _ => panic!("expected blocked"),
        }
    }

    #[test]
    fn rejects_garbage() {
        let err = parse_output("nothing useful here").unwrap_err();
        match err {
            ParseError::Malformed(_) => {}
        }
    }
}

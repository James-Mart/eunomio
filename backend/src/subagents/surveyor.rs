use crate::subagents::loader::{ParseError, PromptTemplate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurveyOutput {
    pub summary: String,
    pub themes: Vec<SurveyTheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurveyTheme {
    pub id: String,
    pub title: String,
    pub description: String,
}

pub struct SurveyContext {
    pub before_tree: String,
    pub target_tree: String,
    pub user_feedback: String,
}

pub fn render_prompt(ctx: &SurveyContext, template: &PromptTemplate) -> String {
    let mut map = serde_json::Map::new();
    map.insert("BEFORE_TREE".into(), serde_json::json!(ctx.before_tree));
    map.insert("TARGET_TREE".into(), serde_json::json!(ctx.target_tree));
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

pub fn parse_output(raw: &str) -> Result<SurveyOutput, ParseError> {
    let block = extract_json_block(raw)
        .ok_or_else(|| ParseError::Malformed("no fenced ```json``` block found".into()))?;
    serde_json::from_str(&block).map_err(|e| ParseError::Malformed(format!("invalid JSON: {e}")))
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
            &SurveyContext {
                before_tree: "deadbeef".into(),
                target_tree: "cafebabe".into(),
                user_feedback: "".into(),
            },
            &defs.surveyor.template,
        );
        assert!(out.contains("deadbeef"));
        assert!(out.contains("cafebabe"));
        assert!(out.contains("(none)"));
    }

    #[test]
    fn parses_fenced_json_block() {
        let raw = "Here you go:\n\n```json\n{\n  \"summary\": \"s\",\n  \"themes\": []\n}\n```\n";
        let parsed = parse_output(raw).unwrap();
        assert_eq!(parsed.summary, "s");
        assert!(parsed.themes.is_empty());
    }

    #[test]
    fn parses_themes() {
        let raw = "```json\n{\"summary\":\"x\",\"themes\":[{\"id\":\"t-1\",\"title\":\"T\",\"description\":\"D\"}]}\n```";
        let parsed = parse_output(raw).unwrap();
        assert_eq!(parsed.themes.len(), 1);
        assert_eq!(parsed.themes[0].id, "t-1");
    }

    #[test]
    fn missing_block_fails() {
        let err = parse_output("no json here").unwrap_err();
        match err {
            ParseError::Malformed(msg) => assert!(msg.contains("no fenced")),
        }
    }
}

// SPDX-License-Identifier: Apache-2.0

use crate::subagents::loader::{ParseError, PromptTemplate};
use serde::Deserialize;

pub struct ShaverContext {
    pub worktree_path: String,
    pub parent_commit: String,
    pub before_tree: String,
    pub target_tree: String,
    pub target_title: String,
    pub target_description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaverOutput {
    pub head_commit: String,
}

pub fn render_prompt(ctx: &ShaverContext, template: &PromptTemplate) -> String {
    let mut map = serde_json::Map::new();
    map.insert("WORKTREE_PATH".into(), serde_json::json!(ctx.worktree_path));
    map.insert("PARENT_COMMIT".into(), serde_json::json!(ctx.parent_commit));
    map.insert("BEFORE_TREE".into(), serde_json::json!(ctx.before_tree));
    map.insert("TARGET_TREE".into(), serde_json::json!(ctx.target_tree));
    map.insert("TARGET_TITLE".into(), serde_json::json!(ctx.target_title));
    map.insert(
        "TARGET_DESCRIPTION".into(),
        serde_json::json!(ctx.target_description),
    );
    template.render(&map)
}

pub fn parse_output(raw: &str) -> Result<ShaverOutput, ParseError> {
    let block = extract_json_block(raw)
        .ok_or_else(|| ParseError::Malformed("no fenced ```json``` block found".into()))?;
    serde_json::from_str(&block).map_err(|e| ParseError::Malformed(format!("invalid JSON: {e}")))
}

fn extract_json_block(raw: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?s)```(?:json)?\s*\n(.*?)\n```").expect("valid json fence regex");
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

    #[test]
    fn parses_head_commit() {
        let raw = r#"```json
{"headCommit":"abc"}
```"#;
        assert_eq!(parse_output(raw).unwrap().head_commit, "abc");
    }
}

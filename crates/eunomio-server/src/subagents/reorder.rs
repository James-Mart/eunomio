// SPDX-License-Identifier: Apache-2.0

use crate::subagents::loader::{ParseError, PromptTemplate};
use eunomio_core::types::ReorderRelation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderOutput {
    pub proposed_order: Vec<String>,
    #[serde(default)]
    pub hard_deps: Vec<ReorderRelation>,
    #[serde(default)]
    pub soft_prefs: Vec<ReorderRelation>,
    #[serde(default)]
    pub uncertain_pairs: Vec<[String; 2]>,
    pub rationale: String,
}

pub struct ReorderContext {
    pub base_commit: String,
    pub final_commit: String,
    pub base_tree: String,
    pub final_tree: String,
    pub chain_json: String,
}

pub fn render_prompt(ctx: &ReorderContext, template: &PromptTemplate) -> String {
    let mut map = serde_json::Map::new();
    map.insert("BASE_COMMIT".into(), serde_json::json!(ctx.base_commit));
    map.insert("FINAL_COMMIT".into(), serde_json::json!(ctx.final_commit));
    map.insert("BASE_TREE".into(), serde_json::json!(ctx.base_tree));
    map.insert("FINAL_TREE".into(), serde_json::json!(ctx.final_tree));
    map.insert("CHAIN_JSON".into(), serde_json::json!(ctx.chain_json));
    template.render(&map)
}

pub fn parse_output(raw: &str) -> Result<ReorderOutput, ParseError> {
    let block = extract_json_block(raw)
        .ok_or_else(|| ParseError::Malformed("no fenced ```json``` block found".into()))?;
    let parsed: ReorderOutput = serde_json::from_str(&block)
        .map_err(|e| ParseError::Malformed(format!("invalid JSON: {e}")))?;
    if parsed.proposed_order.is_empty() {
        return Err(ParseError::Malformed("proposedOrder is empty".into()));
    }
    Ok(parsed)
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

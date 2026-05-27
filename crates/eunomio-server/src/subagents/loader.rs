// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use rust_embed::Embed;
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Embed)]
#[folder = "../../subagents/"]
struct SubagentAssets;

pub struct Subagents {
    pub planner: SubagentDef,
    pub constructor: SubagentDef,
    pub shaver: SubagentDef,
    pub reorder: SubagentDef,
}

pub struct SubagentDef {
    pub name: String,
    pub template: PromptTemplate,
}

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    body: String,
    placeholders: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("malformed subagent output: {0}")]
    Malformed(String),
}

impl PromptTemplate {
    pub fn parse(body: String, allowed: &[&str]) -> Result<Self> {
        let re = placeholder_re();
        let mut placeholders = Vec::new();
        let mut seen = HashSet::new();
        for cap in re.captures_iter(&body) {
            let ident = cap.get(1).unwrap().as_str();
            if !allowed.iter().any(|a| *a == ident) {
                return Err(anyhow!(
                    "unknown placeholder {{{{{ident}}}}} in subagent prompt (allowed: {allowed:?})"
                ));
            }
            if seen.insert(ident.to_string()) {
                placeholders.push(ident.to_string());
            }
        }
        Ok(Self { body, placeholders })
    }

    pub fn render(&self, ctx: &serde_json::Map<String, serde_json::Value>) -> String {
        let re = placeholder_re();
        re.replace_all(&self.body, |caps: &regex::Captures<'_>| {
            let ident = caps.get(1).unwrap().as_str();
            match ctx.get(ident) {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            }
        })
        .into_owned()
    }

    pub fn placeholders(&self) -> &[String] {
        &self.placeholders
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

pub fn resolve_prompt_template(
    default: &PromptTemplate,
    allowed: &[&str],
    override_text: Option<&str>,
) -> Result<PromptTemplate> {
    if let Some(text) = override_text.map(str::trim).filter(|s| !s.is_empty()) {
        PromptTemplate::parse(text.to_string(), allowed)
    } else {
        Ok(default.clone())
    }
}

fn placeholder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{\{([A-Z][A-Z0-9_]*)\}\}").unwrap())
}

pub fn planner_placeholders() -> &'static [&'static str] {
    &[
        "PARENT_COMMIT",
        "BEFORE_TREE",
        "TARGET_TREE",
        "STRATEGY_OVERRIDE",
        "USER_FEEDBACK",
        "PRIOR_BLOCK_OR_CANDIDATE",
    ]
}

pub fn constructor_placeholders() -> &'static [&'static str] {
    &[
        "WORKTREE_PATH",
        "PARENT_COMMIT",
        "BEFORE_TREE",
        "TARGET_TREE",
        "STRATEGY",
        "SLICE_TITLE",
        "SLICE_DESCRIPTION",
        "USER_FEEDBACK",
    ]
}

pub fn shaver_placeholders() -> &'static [&'static str] {
    &[
        "WORKTREE_PATH",
        "PARENT_COMMIT",
        "BEFORE_TREE",
        "TARGET_TREE",
        "TARGET_TITLE",
        "TARGET_DESCRIPTION",
    ]
}

pub fn reorder_placeholders() -> &'static [&'static str] {
    &[
        "BASE_COMMIT",
        "FINAL_COMMIT",
        "BASE_TREE",
        "FINAL_TREE",
        "CHAIN_JSON",
    ]
}

fn load_one(file: &str, allowed: &[&str]) -> Result<SubagentDef> {
    let raw = SubagentAssets::get(file)
        .ok_or_else(|| anyhow!("missing embedded subagent prompt: {file}"))?;
    let body = std::str::from_utf8(raw.data.as_ref())
        .with_context(|| format!("subagent prompt {file} is not UTF-8"))?
        .to_string();
    let name = file.strip_suffix(".md").unwrap_or(file).to_string();
    let template = PromptTemplate::parse(body, allowed)
        .with_context(|| format!("parsing subagent prompt {file}"))?;
    Ok(SubagentDef { name, template })
}

pub fn load_subagents() -> Result<Subagents> {
    Ok(Subagents {
        planner: load_one("planner.md", planner_placeholders())?,
        constructor: load_one("constructor.md", constructor_placeholders())?,
        shaver: load_one("shaver.md", shaver_placeholders())?,
        reorder: load_one("reorder.md", reorder_placeholders())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_known_placeholders() {
        let tpl = PromptTemplate::parse(
            "A {{BEFORE_TREE}} B {{TARGET_TREE}} C {{USER_FEEDBACK}}".to_string(),
            planner_placeholders(),
        )
        .unwrap();
        let mut ctx = serde_json::Map::new();
        ctx.insert("BEFORE_TREE".into(), serde_json::json!("aaa"));
        ctx.insert("TARGET_TREE".into(), serde_json::json!("bbb"));
        ctx.insert("USER_FEEDBACK".into(), serde_json::json!("(none)"));
        assert_eq!(tpl.render(&ctx), "A aaa B bbb C (none)");
    }

    #[test]
    fn unknown_placeholder_fails_to_parse() {
        let err = PromptTemplate::parse("hello {{NOPE}}".to_string(), planner_placeholders())
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("NOPE"), "msg = {msg}");
    }

    #[test]
    fn embedded_prompts_parse() {
        let defs = load_subagents().expect("load embedded subagents");
        assert_eq!(defs.planner.name, "planner");
        assert_eq!(defs.constructor.name, "constructor");
        assert_eq!(defs.shaver.name, "shaver");
        assert_eq!(defs.reorder.name, "reorder");
    }
}

// SPDX-License-Identifier: Apache-2.0

use serde_json::Value;

pub fn fold_sdk_event(message: &Value) -> Option<String> {
    let obj = message.as_object()?;

    match obj.get("type").and_then(|v| v.as_str()) {
        Some("status") => None,
        Some("tool_use") => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            Some(format!("\n[tool: {name}]\n"))
        }
        Some("assistant") => text_from_content(message).or_else(|| direct_text(obj)),
        Some(_) => direct_text(obj).or_else(|| text_from_content(message)),
        None => direct_text(obj).or_else(|| text_from_content(message)),
    }
}

fn direct_text(obj: &serde_json::Map<String, Value>) -> Option<String> {
    obj.get("text")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn text_from_content(message: &Value) -> Option<String> {
    let content = message
        .pointer("/message/content")
        .or_else(|| message.get("content"))?;
    let arr = content.as_array()?;
    let parts: Vec<&str> = arr
        .iter()
        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::fold_sdk_event;
    use serde_json::json;

    #[test]
    fn folds_assistant_text_fixture() {
        assert_eq!(
            fold_sdk_event(&json!({"type": "assistant", "text": "thinking 1"})),
            Some("thinking 1".into())
        );
    }

    #[test]
    fn folds_top_level_text_fixture() {
        assert_eq!(
            fold_sdk_event(&json!({"text": "s"})),
            Some("s".into())
        );
    }

    #[test]
    fn folds_message_content_stream() {
        assert_eq!(
            fold_sdk_event(&json!({
                "type": "assistant",
                "message": {
                    "content": [
                        {"type": "text", "text": "I'll"},
                        {"type": "text", "text": " help"}
                    ]
                }
            })),
            Some("I'll help".into())
        );
    }

    #[test]
    fn folds_tool_use_marker() {
        assert_eq!(
            fold_sdk_event(&json!({"type": "tool_use", "name": "ls"})),
            Some("\n[tool: ls]\n".into())
        );
    }

    #[test]
    fn skips_status_events() {
        assert_eq!(
            fold_sdk_event(&json!({"type": "status", "status": "RUNNING"})),
            None
        );
    }
}

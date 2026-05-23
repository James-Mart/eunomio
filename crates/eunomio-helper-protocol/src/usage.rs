// SPDX-License-Identifier: Apache-2.0

use eunomio_core::traits::quota::TokenUsage;

/// Parse token usage from a Cursor SDK message with `type: "turn-ended"`.
pub fn parse_turn_ended_usage(message: &serde_json::Value) -> Option<TokenUsage> {
    if message.get("type")?.as_str()? != "turn-ended" {
        return None;
    }
    let usage = message.get("usage")?;
    Some(TokenUsage {
        input_tokens: usage.get("inputTokens")?.as_u64()?,
        output_tokens: usage.get("outputTokens")?.as_u64()?,
        cache_read_tokens: usage
            .get("cacheReadTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        cache_write_tokens: usage
            .get("cacheWriteTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_turn_ended_usage_from_sdk_message() {
        let msg = json!({
            "type": "turn-ended",
            "usage": {
                "inputTokens": 100,
                "outputTokens": 50,
                "cacheReadTokens": 10,
                "cacheWriteTokens": 5
            }
        });
        let usage = parse_turn_ended_usage(&msg).expect("usage");
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_read_tokens, 10);
        assert_eq!(usage.cache_write_tokens, 5);
    }

    #[test]
    fn ignores_non_turn_ended_messages() {
        assert!(parse_turn_ended_usage(&json!({"type": "assistant"})).is_none());
    }
}

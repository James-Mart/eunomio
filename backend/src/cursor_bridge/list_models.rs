use super::{helper_assets::ensure_helper_extracted, unavailable};
use crate::{error::AppError, state::AppState, types::CursorModel};
use serde::Deserialize;

pub async fn list_models(state: &AppState) -> Result<Vec<CursorModel>, AppError> {
    let api_key = state
        .cursor_api_key
        .as_deref()
        .ok_or_else(|| unavailable("CURSOR_API_KEY not configured"))?;
    let binary = ensure_helper_extracted(&state.data_dir).await?;
    let output = tokio::process::Command::new(&binary)
        .arg("list-models")
        .env("CURSOR_API_KEY", api_key)
        .output()
        .await
        .map_err(|e| unavailable(&format!("spawning cursor-helper: {e}")))?;
    if !output.status.success() {
        if let Some(msg) = parse_helper_error(&output.stdout) {
            return Err(unavailable(&msg));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(unavailable(&format!(
            "cursor-helper exited {}: {}",
            output.status, stderr
        )));
    }
    let parsed: ListModelsResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| unavailable(&format!("parsing cursor-helper output: {e}")))?;
    Ok(parsed.models)
}

/// Best-effort extraction of the helper's structured `{ "error": "..." }`
/// failure from its stdout. Falls back to the raw stderr in the caller when
/// the bytes are not JSON (or aren't shaped like an error object).
fn parse_helper_error(stdout: &[u8]) -> Option<String> {
    let parsed = serde_json::from_slice::<serde_json::Value>(stdout).ok()?;
    parsed.get("error")?.as_str().map(|s| s.to_string())
}

#[derive(Deserialize)]
struct ListModelsResponse {
    models: Vec<CursorModel>,
}

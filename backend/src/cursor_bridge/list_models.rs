use super::{helper_assets::ensure_helper_extracted, helper_stdio::launch_helper_stdio, unavailable};
use crate::{credentials::KeyStore, error::AppError, state::AppState, types::CursorModel};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub async fn list_models(state: &AppState, user_id: &str) -> Result<Vec<CursorModel>, AppError> {
    list_models_with_keystore(&state.keystore, &state.data_dir, user_id).await
}

#[derive(Serialize)]
struct ListModelsRequest {
    #[serde(rename = "cursorApiKey")]
    cursor_api_key: String,
}

pub async fn list_models_with_keystore(
    keystore: &KeyStore,
    data_dir: &Path,
    user_id: &str,
) -> Result<Vec<CursorModel>, AppError> {
    let api_key = keystore
        .get(user_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("reading cursor api key: {e}")))?
        .ok_or_else(|| unavailable("Cursor API key not configured"))?;
    let binary = ensure_helper_extracted(data_dir).await?;
    let payload = serde_json::to_vec(&ListModelsRequest {
        cursor_api_key: api_key,
    })
    .map_err(|e| unavailable(&format!("serializing list-models request: {e}")))?;
    let mut child = launch_helper_stdio(&binary, "list-models", &payload).await?;
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| unavailable(&format!("waiting on cursor-helper: {e}")))?;
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

fn parse_helper_error(stdout: &[u8]) -> Option<String> {
    let parsed = serde_json::from_slice::<serde_json::Value>(stdout).ok()?;
    parsed.get("error")?.as_str().map(|s| s.to_string())
}

#[derive(Deserialize)]
struct ListModelsResponse {
    models: Vec<CursorModel>,
}

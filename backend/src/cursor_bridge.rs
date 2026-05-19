use crate::{
    error::AppError,
    server::AppState,
    types::CursorModelDto,
};
use axum::http::StatusCode;
use rust_embed::Embed;
use std::path::PathBuf;

#[derive(Embed)]
#[folder = "../helper/dist/"]
struct HelperAssets;

const HELPER_FILE: &str = "cursor-helper";

pub async fn list_models(state: &AppState) -> Result<Vec<CursorModelDto>, AppError> {
    let api_key = state
        .cursor_api_key
        .as_deref()
        .ok_or_else(|| unavailable("CURSOR_API_KEY not configured"))?;
    let binary = ensure_helper_extracted().await?;
    let output = tokio::process::Command::new(&binary)
        .arg("list-models")
        .env("CURSOR_API_KEY", api_key)
        .output()
        .await
        .map_err(|e| unavailable(&format!("spawning cursor-helper: {e}")))?;
    if !output.status.success() {
        if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(msg) = parsed.get("error").and_then(|v| v.as_str()) {
                return Err(unavailable(msg));
            }
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

#[derive(serde::Deserialize)]
struct ListModelsResponse {
    models: Vec<CursorModelDto>,
}

fn unavailable(message: &str) -> AppError {
    AppError::Unrecoverable {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "cursor_sdk_unavailable".into(),
        message: message.into(),
    }
}

async fn ensure_helper_extracted() -> Result<PathBuf, AppError> {
    let file = HelperAssets::get(HELPER_FILE)
        .ok_or_else(|| unavailable("cursor-helper binary not embedded in this build"))?;
    let version = env!("CARGO_PKG_VERSION");
    let dir = std::env::temp_dir().join(format!("eunomia-helper-{version}"));
    let target = dir.join(HELPER_FILE);
    if !target.exists() {
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| unavailable(&format!("creating helper temp dir: {e}")))?;
        tokio::fs::write(&target, file.data.as_ref())
            .await
            .map_err(|e| unavailable(&format!("writing helper binary: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&target)
                .await
                .map_err(|e| unavailable(&format!("stat helper binary: {e}")))?
                .permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&target, perms)
                .await
                .map_err(|e| unavailable(&format!("chmod helper binary: {e}")))?;
        }
    }
    Ok(target)
}

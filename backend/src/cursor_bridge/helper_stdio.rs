use crate::error::AppError;
use anyhow::anyhow;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

use super::unavailable;

pub async fn launch_helper_stdio(
    binary: &Path,
    subcommand: &str,
    payload: &[u8],
) -> Result<Child, AppError> {
    let helper_dir = binary
        .parent()
        .ok_or_else(|| AppError::Internal(anyhow!("helper binary has no parent directory")))?;
    let rg_path = helper_dir.join("rg");
    let rg_path = rg_path.canonicalize().unwrap_or(rg_path);

    let mut child = Command::new(binary)
        .arg(subcommand)
        .env_clear()
        .env("CURSOR_RIPGREP_PATH", rg_path)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(false)
        .spawn()
        .map_err(|e| unavailable(&format!("spawning cursor-helper {subcommand}: {e}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal(anyhow!("helper stdin not available")))?;
    stdin
        .write_all(payload)
        .await
        .map_err(|e| AppError::Internal(anyhow!("writing helper stdin: {e}")))?;
    drop(stdin);

    Ok(child)
}

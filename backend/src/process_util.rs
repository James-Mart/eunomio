use anyhow::{Context, Result};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;

/// Sets `path`'s file mode to `0o755` on Unix (no-op on other platforms).
/// Used by helper-asset extraction and `cloudflared` install paths so each
/// caller doesn't recompute the same chmod boilerplate.
pub async fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(path)
            .await
            .with_context(|| format!("stat {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(path, perms)
            .await
            .with_context(|| format!("chmod +x {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Spawns a task that reads `reader` line-by-line and forwards each line
/// (without its trailing newline) into `tx`. The task exits when EOF is
/// reached or the receiver is dropped.
pub fn spawn_line_forwarder<R>(reader: R, tx: mpsc::UnboundedSender<String>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

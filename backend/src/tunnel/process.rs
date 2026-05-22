use super::{snapshot, transition_to, Inner, Running, TunnelState};
use crate::{
    db,
    error::AppError,
    types::TunnelStatus,
};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    sync::{mpsc, oneshot},
};
use uuid::Uuid;

const URL_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

#[allow(clippy::too_many_arguments)]
pub(super) async fn supervise(
    inner: Arc<Inner>,
    binary: PathBuf,
    target_port: u16,
    token: Option<Uuid>,
    ready_tx: oneshot::Sender<Result<TunnelStatus, AppError>>,
    stop_tx: oneshot::Sender<()>,
    stop_rx: oneshot::Receiver<()>,
    serve_shutdown_tx: Option<oneshot::Sender<()>>,
) {
    let (child, line_rx, url) =
        match start_cloudflared_until_url(&binary, target_port).await {
            Ok(triple) => triple,
            Err(e) => {
                fail_startup(&inner, ready_tx, serve_shutdown_tx, e);
                return;
            }
        };

    let started_at = db::unix_seconds();
    let (dto_full, dto_redacted) = {
        let mut state = inner.state.lock().unwrap();
        *state = TunnelState::Running(Running {
            url: url.clone(),
            token,
            started_at,
            stop_tx,
        });
        (
            snapshot(&state, &inner),
            super::snapshot_redacted(&state, &inner),
        )
    };
    let _ = inner.events.send(dto_redacted);
    let _ = ready_tx.send(Ok(dto_full));

    tokio::spawn(drain_lines(line_rx));
    supervise_running(child, inner, stop_rx, token).await;
    if let Some(tx) = serve_shutdown_tx {
        let _ = tx.send(());
    }
}

/// Spawn cloudflared, attach line readers, and block until it emits the
/// public URL (or fails / times out). Wraps the three steps so `supervise`
/// only needs to handle the success/error split, not the
/// kill-on-fail bookkeeping.
async fn start_cloudflared_until_url(
    binary: &Path,
    target_port: u16,
) -> Result<(Child, mpsc::UnboundedReceiver<String>, String)> {
    let mut child = spawn_cloudflared(binary, target_port)?;
    let mut line_rx = match attach_line_reader(&mut child) {
        Ok(rx) => rx,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    };
    let url = match wait_for_url(&mut line_rx, &mut child).await {
        Ok(u) => u,
        Err(e) => {
            let _ = child.kill().await;
            return Err(e);
        }
    };
    Ok((child, line_rx, url))
}

async fn supervise_running(
    mut child: Child,
    inner: Arc<Inner>,
    stop_rx: oneshot::Receiver<()>,
    expected_token: Option<Uuid>,
) {
    tokio::select! {
        _ = stop_rx => {
            let _ = child.kill().await;
        }
        _ = child.wait() => {
            let still_ours = {
                let state = inner.state.lock().unwrap();
                matches!(&*state, TunnelState::Running(r) if r.token == expected_token)
            };
            if still_ours {
                transition_to(
                    &inner,
                    TunnelState::Error {
                        message: "cloudflared exited unexpectedly".into(),
                        at: db::unix_seconds(),
                    },
                );
            }
        }
    }
}

pub(super) fn fail_startup(
    inner: &Arc<Inner>,
    ready_tx: oneshot::Sender<Result<TunnelStatus, AppError>>,
    serve_shutdown_tx: Option<oneshot::Sender<()>>,
    err: anyhow::Error,
) {
    let message = format!("cloudflared: {err:#}");
    transition_to(
        inner,
        TunnelState::Error {
            message: message.clone(),
            at: db::unix_seconds(),
        },
    );
    let _ = ready_tx.send(Err(AppError::Internal(anyhow!(message))));
    if let Some(tx) = serve_shutdown_tx {
        let _ = tx.send(());
    }
}

fn spawn_cloudflared(binary: &Path, target_port: u16) -> Result<Child> {
    Command::new(binary)
        .args([
            "tunnel",
            "--no-autoupdate",
            "--url",
            &format!("http://localhost:{target_port}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("spawning cloudflared")
}

fn attach_line_reader(child: &mut Child) -> Result<mpsc::UnboundedReceiver<String>> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("cloudflared stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("cloudflared stderr unavailable"))?;
    let (tx, rx) = mpsc::unbounded_channel();
    crate::process_util::spawn_line_forwarder(stdout, tx.clone());
    crate::process_util::spawn_line_forwarder(stderr, tx);
    Ok(rx)
}

async fn wait_for_url(
    line_rx: &mut mpsc::UnboundedReceiver<String>,
    child: &mut Child,
) -> Result<String> {
    let re = url_regex();
    let deadline = tokio::time::Instant::now() + URL_WAIT_TIMEOUT;
    loop {
        tokio::select! {
            line = line_rx.recv() => {
                let Some(line) = line else {
                    return Err(anyhow!("cloudflared output stream closed before URL"));
                };
                tracing::debug!(target: "tunnel.cloudflared", "{line}");
                if let Some(m) = re.find(&line) {
                    return Ok(m.as_str().to_string());
                }
            }
            status = child.wait() => {
                let status = status.context("waiting on cloudflared")?;
                return Err(anyhow!("cloudflared exited before emitting URL (status: {status})"));
            }
            _ = tokio::time::sleep_until(deadline) => {
                return Err(anyhow!("cloudflared did not emit URL within {} seconds", URL_WAIT_TIMEOUT.as_secs()));
            }
        }
    }
}

async fn drain_lines(mut rx: mpsc::UnboundedReceiver<String>) {
    while let Some(line) = rx.recv().await {
        tracing::debug!(target: "tunnel.cloudflared", "{line}");
    }
}

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"https://[a-zA-Z0-9-]+\.trycloudflare\.com").unwrap())
}

use super::{helper_assets::ensure_helper_extracted, unavailable, wire::HelperWireEvent};
use crate::error::AppError;
use anyhow::anyhow;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub run_id: i64,
}

#[derive(Debug, Clone)]
pub enum HelperEvent {
    Started {
        run_id: i64,
        agent_id: String,
    },
    SdkMessage {
        run_id: i64,
        message: serde_json::Value,
    },
    Finished {
        run_id: i64,
        result: String,
        duration_ms: Option<u64>,
    },
    Error {
        run_id: i64,
        code: String,
        message: String,
    },
    Cancelled {
        run_id: i64,
    },
}

pub struct RunHandle {
    pub cancel: Box<dyn Fn() + Send + Sync>,
}

#[async_trait::async_trait]
pub trait SubagentRunner: Send + Sync {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError>;
}

pub struct CursorHelperRunner {
    api_key: Option<String>,
    data_dir: PathBuf,
}

impl CursorHelperRunner {
    pub fn new(api_key: Option<String>, data_dir: PathBuf) -> Self {
        Self { api_key, data_dir }
    }
}

#[async_trait::async_trait]
impl SubagentRunner for CursorHelperRunner {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError> {
        let api_key = self
            .api_key
            .clone()
            .ok_or_else(|| unavailable("CURSOR_API_KEY not configured"))?;
        let binary = ensure_helper_extracted(&self.data_dir).await?;
        let run_id = request.run_id;
        let HelperChild {
            child,
            stdout,
            stderr,
            pid,
        } = spawn_helper_child(&binary, &api_key, &request).await?;

        let ctx = HelperRunContext::new(run_id, tx.clone());
        let stdout_done = spawn_stdout_parser(stdout, ctx.clone());
        let stderr_done = spawn_stderr_collector(stderr, ctx.clone());
        spawn_exit_watchdog(child, ctx, stdout_done, stderr_done, tx);

        Ok(RunHandle {
            cancel: helper_cancel_fn(pid),
        })
    }
}

struct HelperChild {
    child: Child,
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    pid: Option<u32>,
}

async fn spawn_helper_child(
    binary: &std::path::Path,
    api_key: &str,
    request: &RunRequest,
) -> Result<HelperChild, AppError> {
    let request_json = serde_json::to_vec(request)
        .map_err(|e| AppError::Internal(anyhow!("serializing helper run request: {e}")))?;

    let helper_dir = binary.parent().ok_or_else(|| {
        AppError::Internal(anyhow!("helper binary has no parent directory"))
    })?;
    let rg_path = helper_dir.join("rg");
    let rg_path = rg_path
        .canonicalize()
        .unwrap_or(rg_path);

    let mut child = Command::new(binary)
        .arg("run")
        .env_clear()
        .env("CURSOR_API_KEY", api_key)
        .env("CURSOR_RIPGREP_PATH", rg_path)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(false)
        .spawn()
        .map_err(|e| unavailable(&format!("spawning cursor-helper run: {e}")))?;

    let pid = child.id();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal(anyhow!("helper stdin not available")))?;
    stdin
        .write_all(&request_json)
        .await
        .map_err(|e| AppError::Internal(anyhow!("writing helper stdin: {e}")))?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal(anyhow!("helper stdout not available")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal(anyhow!("helper stderr not available")))?;
    Ok(HelperChild {
        child,
        stdout,
        stderr,
        pid,
    })
}

#[derive(Clone)]
struct HelperRunContext {
    run_id: i64,
    tx: mpsc::Sender<HelperEvent>,
    saw_terminal: Arc<Mutex<bool>>,
    stderr_lines: Arc<Mutex<Vec<String>>>,
    stdout_garbage: Arc<Mutex<Vec<String>>>,
}

impl HelperRunContext {
    fn new(run_id: i64, tx: mpsc::Sender<HelperEvent>) -> Self {
        Self {
            run_id,
            tx,
            saw_terminal: Arc::new(Mutex::new(false)),
            stderr_lines: Arc::new(Mutex::new(Vec::new())),
            stdout_garbage: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn mark_terminal(&self) {
        *self.saw_terminal.lock().unwrap() = true;
    }

    fn saw_terminal(&self) -> bool {
        *self.saw_terminal.lock().unwrap()
    }

    fn capture_garbage(&self, line: &str) {
        let mut buf = self.stdout_garbage.lock().unwrap();
        if buf.len() < 32 {
            buf.push(line.to_string());
        }
    }

    fn capture_stderr(&self, line: &str) {
        let mut buf = self.stderr_lines.lock().unwrap();
        if buf.len() < 64 {
            buf.push(line.to_string());
        }
    }

    fn snapshot_stderr(&self) -> Vec<String> {
        self.stderr_lines.lock().unwrap().clone()
    }

    fn snapshot_garbage(&self) -> Vec<String> {
        self.stdout_garbage.lock().unwrap().clone()
    }
}

fn spawn_stdout_parser(
    stdout: tokio::process::ChildStdout,
    ctx: HelperRunContext,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\n', '\r']);
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<HelperWireEvent>(trimmed) {
                        Ok(ev) => {
                            let mapped = ev.into_helper_event(ctx.run_id);
                            if matches!(
                                &mapped,
                                HelperEvent::Finished { .. }
                                    | HelperEvent::Error { .. }
                                    | HelperEvent::Cancelled { .. }
                            ) {
                                ctx.mark_terminal();
                            }
                            if ctx.tx.send(mapped).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(target: "cursor-helper", run_id = ctx.run_id, error = %e, line = %trimmed, "malformed helper NDJSON line");
                            ctx.capture_garbage(trimmed);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(target: "cursor-helper", run_id = ctx.run_id, error = %e, "reading helper stdout failed");
                    break;
                }
            }
        }
    })
}

fn spawn_stderr_collector(
    stderr: tokio::process::ChildStderr,
    ctx: HelperRunContext,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\n', '\r']);
                    if !trimmed.is_empty() {
                        tracing::info!(target: "cursor-helper", run_id = ctx.run_id, "stderr: {trimmed}");
                        ctx.capture_stderr(trimmed);
                    }
                }
                Err(_) => break,
            }
        }
    })
}

fn spawn_exit_watchdog(
    mut child: Child,
    ctx: HelperRunContext,
    stdout_done: JoinHandle<()>,
    stderr_done: JoinHandle<()>,
    tx_for_emit: mpsc::Sender<HelperEvent>,
) {
    tokio::spawn(async move {
        let status = child.wait().await;
        // Drain the readers so any final lines land in the buffers before we
        // use them in the failure-detail message.
        let _ = stdout_done.await;
        let _ = stderr_done.await;

        let status_str = match &status {
            Ok(s) => s.to_string(),
            Err(e) => format!("wait failed: {e}"),
        };
        let exited_cleanly = matches!(&status, Ok(s) if s.success());
        let saw_terminal_event = ctx.saw_terminal();
        let stderr_lines = ctx.snapshot_stderr();
        let garbage_lines = ctx.snapshot_garbage();

        if !exited_cleanly || !saw_terminal_event {
            tracing::warn!(
                target: "cursor-helper",
                run_id = ctx.run_id,
                status = %status_str,
                saw_terminal_event,
                stderr_lines = stderr_lines.len(),
                garbage_lines = garbage_lines.len(),
                "helper finished without clean terminal event",
            );
        }

        if !saw_terminal_event {
            let mut msg = match &status {
                Ok(s) => format!("helper exited {s}"),
                Err(e) => format!("helper wait failed: {e}"),
            };
            let detail = build_helper_failure_detail(&stderr_lines, &garbage_lines);
            if !detail.is_empty() {
                msg.push_str("; ");
                msg.push_str(&detail);
            }
            let _ = tx_for_emit
                .send(HelperEvent::Error {
                    run_id: ctx.run_id,
                    code: "helper_exited".into(),
                    message: msg,
                })
                .await;
        }
        drop(tx_for_emit);
    });
}

fn helper_cancel_fn(pid: Option<u32>) -> Box<dyn Fn() + Send + Sync> {
    Box::new(move || {
        if let Some(pid) = pid {
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                let _ = pid;
            }
        }
    })
}

/// Build a human-readable suffix for the `helper_exited` error message that
/// includes the tail of helper stderr and any non-NDJSON stdout chatter, so
/// the user can actually see why the helper died.
fn build_helper_failure_detail(stderr_lines: &[String], garbage_lines: &[String]) -> String {
    const MAX_TAIL: usize = 8;
    const MAX_TOTAL_CHARS: usize = 1500;

    let mut parts: Vec<String> = Vec::new();
    if !stderr_lines.is_empty() {
        let tail = if stderr_lines.len() > MAX_TAIL {
            &stderr_lines[stderr_lines.len() - MAX_TAIL..]
        } else {
            stderr_lines
        };
        parts.push(format!("stderr: {}", tail.join(" | ")));
    }
    if !garbage_lines.is_empty() {
        let tail = if garbage_lines.len() > MAX_TAIL {
            &garbage_lines[garbage_lines.len() - MAX_TAIL..]
        } else {
            garbage_lines
        };
        parts.push(format!("non-NDJSON stdout: {}", tail.join(" | ")));
    }

    let mut detail = parts.join("; ");
    if detail.chars().count() > MAX_TOTAL_CHARS {
        detail = detail.chars().take(MAX_TOTAL_CHARS).collect::<String>() + "…";
    }
    detail
}

// SPDX-License-Identifier: Apache-2.0

use super::{
    helper_assets::ensure_helper_extracted, helper_stdio::spawn_helper_via_sandbox, unavailable,
    wire::HelperWireEvent,
};
use eunomio_core::{
    traits::sandbox::{SandboxRuntime, SandboxScope},
    types::CursorModel,
    AppError,
};
use eunomio_helper_protocol::{
    HelperEvent, ListModelsRequest, ListModelsResponse, RunHandle, RunRequest, SubagentRunner,
};
use anyhow::anyhow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct CursorHelperRunner {
    data_dir: PathBuf,
    sandbox: Arc<dyn SandboxRuntime>,
}

impl CursorHelperRunner {
    pub fn new(data_dir: PathBuf, sandbox: Arc<dyn SandboxRuntime>) -> Self {
        Self { data_dir, sandbox }
    }
}

#[async_trait::async_trait]
impl SubagentRunner for CursorHelperRunner {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError> {
        if request.cursor_api_key.is_none() {
            return Err(unavailable("Cursor API key not configured"));
        }
        let binary = ensure_helper_extracted(&self.data_dir).await?;
        let run_id = request.run_id.clone();
        let HelperChild {
            child,
            stdout,
            stderr,
            pid,
        } = spawn_helper_child(&self.sandbox, &binary, &request).await?;

        let ctx = HelperRunContext::new(run_id, tx.clone());
        let stdout_done = spawn_stdout_parser(stdout, ctx.clone());
        let stderr_done = spawn_stderr_collector(stderr, ctx.clone());
        spawn_exit_watchdog(child, ctx, stdout_done, stderr_done, tx);

        Ok(RunHandle {
            cancel: helper_cancel_fn(pid),
        })
    }

    async fn list_models(&self, cursor_api_key: &str) -> Result<Vec<CursorModel>, AppError> {
        let binary = ensure_helper_extracted(&self.data_dir).await?;
        let payload = serde_json::to_vec(&ListModelsRequest {
            cursor_api_key: cursor_api_key.to_string(),
        })
        .map_err(|e| unavailable(&format!("serializing list-models request: {e}")))?;
        let child = spawn_helper_via_sandbox(
            self.sandbox.as_ref(),
            &binary,
            "list-models",
            &payload,
            SandboxScope {
                org_id: String::new(),
                user_id: String::new(),
                session_id: String::new(),
                partition_id: String::new(),
                run_id: String::new(),
            },
        )
        .await?;
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
}

fn parse_helper_error(stdout: &[u8]) -> Option<String> {
    let parsed = serde_json::from_slice::<serde_json::Value>(stdout).ok()?;
    parsed.get("error")?.as_str().map(|s| s.to_string())
}

struct HelperChild {
    child: Child,
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    pid: Option<u32>,
}

async fn spawn_helper_child(
    sandbox: &Arc<dyn SandboxRuntime>,
    binary: &std::path::Path,
    request: &RunRequest,
) -> Result<HelperChild, AppError> {
    let request_json = serde_json::to_vec(request)
        .map_err(|e| AppError::Internal(anyhow!("serializing helper run request: {e}")))?;

    let cmd = sandbox
        .wrap(
            eunomio_core::traits::sandbox::SubprocessCommand {
                program: binary.to_path_buf(),
                args: vec!["run".into()],
                stdin_json: Some(request_json),
                cwd: None,
            },
            eunomio_core::traits::sandbox::SandboxScope {
                org_id: String::new(),
                user_id: String::new(),
                session_id: String::new(),
                partition_id: String::new(),
                run_id: request.run_id.clone(),
            },
        )
        .await?;

    let mut child = super::helper_stdio::launch_helper_stdio(
        &cmd.program,
        cmd.args.first().map(String::as_str).unwrap_or("run"),
        cmd.stdin_json.as_deref().unwrap_or(&[]),
    )
    .await?;

    let pid = child.id();
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
    run_id: String,
    tx: mpsc::Sender<HelperEvent>,
    saw_terminal: Arc<Mutex<bool>>,
    stderr_lines: Arc<Mutex<Vec<String>>>,
    stdout_garbage: Arc<Mutex<Vec<String>>>,
}

impl HelperRunContext {
    fn new(run_id: String, tx: mpsc::Sender<HelperEvent>) -> Self {
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
                            let mapped = ev.into_helper_event(ctx.run_id.clone());
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
                            tracing::warn!(target: "cursor-helper", run_id = %ctx.run_id, error = %e, line = %trimmed, "malformed helper NDJSON line");
                            ctx.capture_garbage(trimmed);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(target: "cursor-helper", run_id = %ctx.run_id, error = %e, "reading helper stdout failed");
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
                        tracing::info!(target: "cursor-helper", run_id = %ctx.run_id, "stderr: {trimmed}");
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
                run_id = %ctx.run_id,
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

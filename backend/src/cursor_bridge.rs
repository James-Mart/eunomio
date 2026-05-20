use crate::{error::AppError, server::AppState, types::CursorModelDto};
use anyhow::anyhow;
use axum::http::StatusCode;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Embed)]
#[folder = "../helper/dist/"]
struct HelperAssets;

const HELPER_FILE: &str = "cursor-helper";

/// Native bindings the helper bundle dlopens at runtime. Each entry must live
/// in `helper/dist/` (placed there by `helper/build.mjs`) and be embedded
/// alongside `cursor-helper`. They are extracted into the same temp directory
/// so `helper/src/bindings-loader.cjs` can find them next to `process.execPath`.
const HELPER_NATIVE_FILES: &[&str] = &["node_sqlite3.node"];

pub async fn list_models(state: &AppState) -> Result<Vec<CursorModelDto>, AppError> {
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

fn unavailable(message: &str) -> AppError {
    AppError::Unrecoverable {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "cursor_sdk_unavailable".into(),
        message: message.into(),
    }
}

async fn ensure_helper_extracted(data_dir: &Path) -> Result<PathBuf, AppError> {
    let version = env!("CARGO_PKG_VERSION");
    let dir = data_dir.join("helper").join(version);
    create_private_dir(&dir).await?;

    extract_helper_asset(&dir, HELPER_FILE, true).await?;
    for name in HELPER_NATIVE_FILES {
        extract_helper_asset(&dir, name, false).await?;
    }

    Ok(dir.join(HELPER_FILE))
}

async fn create_private_dir(dir: &Path) -> Result<(), AppError> {
    let dir_owned = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&dir_owned)
    })
    .await
    .map_err(|e| unavailable(&format!("spawn_blocking for helper dir: {e}")))?
    .map_err(|e| unavailable(&format!("creating helper dir {}: {e}", dir.display())))
}

async fn extract_helper_asset(dir: &Path, name: &str, executable: bool) -> Result<(), AppError> {
    let target = dir.join(name);
    if target.exists() {
        return Ok(());
    }
    let asset = HelperAssets::get(name)
        .ok_or_else(|| unavailable(&format!("{name} not embedded in this build")))?;
    let tmp = dir.join(format!("{name}.tmp"));
    tokio::fs::write(&tmp, asset.data.as_ref())
        .await
        .map_err(|e| unavailable(&format!("writing helper asset {name}: {e}")))?;
    if executable {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&tmp)
                .await
                .map_err(|e| unavailable(&format!("stat helper asset {name}: {e}")))?
                .permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&tmp, perms)
                .await
                .map_err(|e| unavailable(&format!("chmod helper asset {name}: {e}")))?;
        }
    }
    if let Err(e) = tokio::fs::rename(&tmp, &target).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        if !target.exists() {
            return Err(unavailable(&format!("renaming helper asset {name}: {e}")));
        }
    }
    Ok(())
}

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

        let request_json = serde_json::to_vec(&request).map_err(|e| {
            AppError::Internal(anyhow!("serializing helper run request: {e}"))
        })?;

        let mut child = Command::new(&binary)
            .arg("run")
            .env_clear()
            .env("CURSOR_API_KEY", &api_key)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(false)
            .spawn()
            .map_err(|e| unavailable(&format!("spawning cursor-helper run: {e}")))?;

        let pid = child.id();
        let run_id = request.run_id;

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

        let saw_terminal = Arc::new(Mutex::new(false));
        let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let stdout_garbage: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let saw_terminal_for_stdout = saw_terminal.clone();
        let stdout_garbage_for_stdout = stdout_garbage.clone();
        let tx_for_stdout = tx.clone();
        let stdout_done = tokio::spawn(async move {
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
                                let mapped = ev.into_helper_event(run_id);
                                if matches!(
                                    &mapped,
                                    HelperEvent::Finished { .. }
                                        | HelperEvent::Error { .. }
                                        | HelperEvent::Cancelled { .. }
                                ) {
                                    *saw_terminal_for_stdout.lock().unwrap() = true;
                                }
                                if tx_for_stdout.send(mapped).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(target: "cursor-helper", run_id, error = %e, line = %trimmed, "malformed helper NDJSON line");
                                let mut buf = stdout_garbage_for_stdout.lock().unwrap();
                                if buf.len() < 32 {
                                    buf.push(trimmed.to_string());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: "cursor-helper", run_id, error = %e, "reading helper stdout failed");
                        break;
                    }
                }
            }
        });

        let stderr_buf_for_stderr = stderr_buf.clone();
        let stderr_done = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end_matches(['\n', '\r']);
                        if !trimmed.is_empty() {
                            tracing::info!(target: "cursor-helper", run_id, "stderr: {trimmed}");
                            let mut buf = stderr_buf_for_stderr.lock().unwrap();
                            if buf.len() < 64 {
                                buf.push(trimmed.to_string());
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let tx_for_wait = tx.clone();
        let saw_terminal_for_wait = saw_terminal.clone();
        let stderr_buf_for_wait = stderr_buf.clone();
        let stdout_garbage_for_wait = stdout_garbage.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            // Drain stdout/stderr readers so any final lines land in the buffers
            // before we use them in the error message.
            let _ = stdout_done.await;
            let _ = stderr_done.await;

            let status_str = match &status {
                Ok(s) => s.to_string(),
                Err(e) => format!("wait failed: {e}"),
            };
            let exited_cleanly = matches!(&status, Ok(s) if s.success());
            let saw_terminal_event = *saw_terminal_for_wait.lock().unwrap();

            let stderr_lines = stderr_buf_for_wait.lock().unwrap().clone();
            let garbage_lines = stdout_garbage_for_wait.lock().unwrap().clone();

            if !exited_cleanly || !saw_terminal_event {
                tracing::warn!(
                    target: "cursor-helper",
                    run_id,
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
                let _ = tx_for_wait
                    .send(HelperEvent::Error {
                        run_id,
                        code: "helper_exited".into(),
                        message: msg,
                    })
                    .await;
            }
            drop(tx_for_wait);
        });

        let pid_opt = pid;
        let cancel: Box<dyn Fn() + Send + Sync> = Box::new(move || {
            if let Some(pid) = pid_opt {
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
        });
        Ok(RunHandle { cancel })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum HelperWireEvent {
    #[serde(rename_all = "camelCase")]
    Started { agent_id: String },
    #[serde(rename_all = "camelCase")]
    SdkMessage { message: serde_json::Value },
    #[serde(rename_all = "camelCase")]
    Finished {
        result: String,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Error { code: String, message: String },
    #[serde(rename_all = "camelCase")]
    Cancelled,
}

impl HelperWireEvent {
    fn into_helper_event(self, run_id: i64) -> HelperEvent {
        match self {
            HelperWireEvent::Started { agent_id } => HelperEvent::Started { run_id, agent_id },
            HelperWireEvent::SdkMessage { message } => HelperEvent::SdkMessage { run_id, message },
            HelperWireEvent::Finished { result, duration_ms } => HelperEvent::Finished {
                run_id,
                result,
                duration_ms,
            },
            HelperWireEvent::Error { code, message } => HelperEvent::Error {
                run_id,
                code,
                message,
            },
            HelperWireEvent::Cancelled => HelperEvent::Cancelled { run_id },
        }
    }
}

pub struct FakeSubagentRunner {
    scripts: tokio::sync::Mutex<Vec<Vec<HelperEvent>>>,
    spawned: std::sync::atomic::AtomicUsize,
}

impl FakeSubagentRunner {
    pub fn new(scripts: Vec<Vec<HelperEvent>>) -> Self {
        Self {
            scripts: tokio::sync::Mutex::new(scripts),
            spawned: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub async fn push_script(&self, events: Vec<HelperEvent>) {
        self.scripts.lock().await.push(events);
    }

    pub fn spawn_count(&self) -> usize {
        self.spawned.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl SubagentRunner for FakeSubagentRunner {
    async fn run(
        &self,
        request: RunRequest,
        tx: mpsc::Sender<HelperEvent>,
    ) -> Result<RunHandle, AppError> {
        let mut scripts = self.scripts.lock().await;
        let events = if scripts.is_empty() {
            vec![HelperEvent::Finished {
                run_id: request.run_id,
                result: String::new(),
                duration_ms: None,
            }]
        } else {
            scripts.remove(0)
        };
        drop(scripts);
        self.spawned
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let run_id = request.run_id;
        let events: Vec<HelperEvent> = events
            .into_iter()
            .map(|e| rebind_run_id(e, run_id))
            .collect();
        tokio::spawn(async move {
            for ev in events {
                if tx.send(ev).await.is_err() {
                    break;
                }
            }
        });
        let cancel: Box<dyn Fn() + Send + Sync> = Box::new(|| {});
        Ok(RunHandle { cancel })
    }
}

fn rebind_run_id(ev: HelperEvent, run_id: i64) -> HelperEvent {
    match ev {
        HelperEvent::Started { agent_id, .. } => HelperEvent::Started { run_id, agent_id },
        HelperEvent::SdkMessage { message, .. } => HelperEvent::SdkMessage { run_id, message },
        HelperEvent::Finished {
            result,
            duration_ms,
            ..
        } => HelperEvent::Finished {
            run_id,
            result,
            duration_ms,
        },
        HelperEvent::Error { code, message, .. } => HelperEvent::Error {
            run_id,
            code,
            message,
        },
        HelperEvent::Cancelled { .. } => HelperEvent::Cancelled { run_id },
    }
}

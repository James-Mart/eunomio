// Cloudflared upgrade procedure: bump `CLOUDFLARED_VERSION`, then update every
// `expected_sha256` in `platform_target` by downloading each asset from the new
// release tag and running `sha256sum` (or `shasum -a 256`) on it. Hashes are
// computed over the published asset as-downloaded (tarballs are hashed before
// extraction, since Cloudflare publishes asset-level hashes).
use crate::{
    error::AppError,
    types::{TunnelStateName, TunnelStatusDto},
};
use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, Response, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::IntoResponse,
    Router,
};
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::{broadcast, mpsc, oneshot},
};
use uuid::Uuid;

const BROADCAST_CAPACITY: usize = 16;
const URL_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const SHARE_COOKIE: &str = "eunomia_share_token";
const SHARE_QUERY: &str = "eunomia_token";
const CLOUDFLARED_VERSION: &str = "2026.5.0";
const DEV_TUNNEL_TARGET_PORT: u16 = 5173;

#[derive(Clone)]
pub struct TunnelRegistry {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<TunnelState>,
    events: broadcast::Sender<TunnelStatusDto>,
    data_dir: PathBuf,
    start_gate: tokio::sync::Mutex<()>,
    dev_mode: bool,
}

enum TunnelState {
    Idle,
    Running(Running),
    Error { message: String, at: i64 },
}

struct Running {
    url: String,
    token: Option<Uuid>,
    started_at: i64,
    stop_tx: oneshot::Sender<()>,
}

impl TunnelRegistry {
    pub fn new(data_dir: PathBuf, dev_mode: bool) -> Self {
        let (events, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(TunnelState::Idle),
                events,
                data_dir,
                start_gate: tokio::sync::Mutex::new(()),
                dev_mode,
            }),
        }
    }

    pub fn status(&self) -> TunnelStatusDto {
        let state = self.inner.state.lock().unwrap();
        snapshot(&state, self.inner.dev_mode)
    }

    /// Same as [`Self::status`] with the share token stripped. Use this for
    /// anything that might be observed via the tunnel itself.
    pub fn status_redacted(&self) -> TunnelStatusDto {
        let state = self.inner.state.lock().unwrap();
        snapshot_redacted(&state, self.inner.dev_mode)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TunnelStatusDto> {
        self.inner.events.subscribe()
    }

    pub async fn start(&self, router: Router) -> Result<TunnelStatusDto, AppError> {
        let _gate = self.inner.start_gate.lock().await;
        {
            let state = self.inner.state.lock().unwrap();
            if matches!(*state, TunnelState::Running(_)) {
                return Err(AppError::Conflict {
                    code: "tunnel_already_running".into(),
                    message: "a tunnel is already running".into(),
                });
            }
        }

        let binary = ensure_binary(&self.inner.data_dir)
            .await
            .map_err(map_binary_error)?;

        let (target_port, token, serve_shutdown_tx) = if self.inner.dev_mode {
            drop(router);
            (DEV_TUNNEL_TARGET_PORT, None, None)
        } else {
            let token = Uuid::new_v4();
            let listener = TcpListener::bind(("127.0.0.1", 0u16))
                .await
                .context("binding tunnel auth listener")?;
            let port = listener
                .local_addr()
                .context("reading tunnel listener address")?
                .port();
            let auth_router = router.layer(from_fn_with_state(token, check_token));
            let (tx, rx) = oneshot::channel::<()>();
            tokio::spawn(async move {
                let server =
                    axum::serve(listener, auth_router).with_graceful_shutdown(async move {
                        let _ = rx.await;
                    });
                if let Err(e) = server.await {
                    tracing::error!(error = %e, "tunnel listener exited with error");
                }
            });
            (port, Some(token), Some(tx))
        };

        let (ready_tx, ready_rx) = oneshot::channel::<Result<TunnelStatusDto, AppError>>();
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let inner = self.inner.clone();
        tokio::spawn(async move {
            supervise(
                inner,
                binary,
                target_port,
                token,
                ready_tx,
                stop_tx,
                stop_rx,
                serve_shutdown_tx,
            )
            .await;
        });

        match ready_rx.await {
            Ok(result) => result,
            Err(_) => {
                self.set_error("cloudflared supervisor crashed".into());
                Err(AppError::Internal(anyhow!(
                    "cloudflared supervisor crashed before reporting status"
                )))
            }
        }
    }

    pub fn stop(&self) -> Result<(), AppError> {
        let prev = {
            let mut state = self.inner.state.lock().unwrap();
            match std::mem::replace(&mut *state, TunnelState::Idle) {
                TunnelState::Running(r) => Some(r),
                other => {
                    *state = other;
                    None
                }
            }
        };
        let Some(r) = prev else {
            return Err(AppError::Conflict {
                code: "tunnel_not_running".into(),
                message: "no tunnel is running".into(),
            });
        };
        let _ = r.stop_tx.send(());
        let _ = self.inner.events.send(self.status_redacted());
        Ok(())
    }

    fn set_error(&self, message: String) {
        {
            let mut state = self.inner.state.lock().unwrap();
            *state = TunnelState::Error {
                message,
                at: now_secs(),
            };
        }
        let _ = self.inner.events.send(self.status_redacted());
    }
}

fn snapshot(state: &TunnelState, dev_mode: bool) -> TunnelStatusDto {
    let token_required = !dev_mode;
    match state {
        TunnelState::Idle => TunnelStatusDto {
            state: TunnelStateName::Idle,
            token_required,
            url: None,
            token: None,
            started_at: None,
            error_message: None,
        },
        TunnelState::Running(r) => TunnelStatusDto {
            state: TunnelStateName::Running,
            token_required,
            url: Some(r.url.clone()),
            token: r.token.map(|t| t.to_string()),
            started_at: Some(r.started_at),
            error_message: None,
        },
        TunnelState::Error { message, at } => TunnelStatusDto {
            state: TunnelStateName::Error,
            token_required,
            url: None,
            token: None,
            started_at: Some(*at),
            error_message: Some(message.clone()),
        },
    }
}

/// Like `snapshot` but never includes the share token. Used for any
/// broadcast that may be observed via the tunnel itself (so a holder
/// of the token cannot use the SSE stream to see future tokens after
/// rotation).
fn snapshot_redacted(state: &TunnelState, dev_mode: bool) -> TunnelStatusDto {
    let mut dto = snapshot(state, dev_mode);
    dto.token = None;
    dto
}

#[allow(clippy::too_many_arguments)]
async fn supervise(
    inner: Arc<Inner>,
    binary: PathBuf,
    target_port: u16,
    token: Option<Uuid>,
    ready_tx: oneshot::Sender<Result<TunnelStatusDto, AppError>>,
    stop_tx: oneshot::Sender<()>,
    stop_rx: oneshot::Receiver<()>,
    serve_shutdown_tx: Option<oneshot::Sender<()>>,
) {
    let dev_mode = inner.dev_mode;
    let mut child = match spawn_cloudflared(&binary, target_port) {
        Ok(c) => c,
        Err(e) => {
            fail_startup(&inner, ready_tx, serve_shutdown_tx, e);
            return;
        }
    };
    let mut line_rx = match attach_line_reader(&mut child) {
        Ok(r) => r,
        Err(e) => {
            let _ = child.kill().await;
            fail_startup(&inner, ready_tx, serve_shutdown_tx, e);
            return;
        }
    };

    let url = match wait_for_url(&mut line_rx, &mut child).await {
        Ok(u) => u,
        Err(e) => {
            let _ = child.kill().await;
            fail_startup(&inner, ready_tx, serve_shutdown_tx, e);
            return;
        }
    };

    let started_at = now_secs();
    let (dto_full, dto_redacted) = {
        let mut state = inner.state.lock().unwrap();
        *state = TunnelState::Running(Running {
            url: url.clone(),
            token,
            started_at,
            stop_tx,
        });
        (snapshot(&state, dev_mode), snapshot_redacted(&state, dev_mode))
    };
    let _ = inner.events.send(dto_redacted);
    let _ = ready_tx.send(Ok(dto_full));

    tokio::spawn(drain_lines(line_rx));

    tokio::select! {
        _ = stop_rx => {
            let _ = child.kill().await;
        }
        _ = child.wait() => {
            let mut state = inner.state.lock().unwrap();
            if let TunnelState::Running(r) = &*state {
                if r.token == token {
                    *state = TunnelState::Error {
                        message: "cloudflared exited unexpectedly".into(),
                        at: now_secs(),
                    };
                    let _ = inner.events.send(snapshot_redacted(&state, dev_mode));
                }
            }
        }
    }
    if let Some(tx) = serve_shutdown_tx {
        let _ = tx.send(());
    }
}

fn fail_startup(
    inner: &Arc<Inner>,
    ready_tx: oneshot::Sender<Result<TunnelStatusDto, AppError>>,
    serve_shutdown_tx: Option<oneshot::Sender<()>>,
    err: anyhow::Error,
) {
    let message = format!("cloudflared: {err:#}");
    {
        let mut state = inner.state.lock().unwrap();
        *state = TunnelState::Error {
            message: message.clone(),
            at: now_secs(),
        };
        let _ = inner
            .events
            .send(snapshot_redacted(&state, inner.dev_mode));
    }
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
    let tx_out = tx.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_out.send(line).is_err() {
                break;
            }
        }
    });
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
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

async fn check_token(State(token): State<Uuid>, req: Request, next: Next) -> Response<Body> {
    if cookie_matches(&req, &token) {
        return next.run(req).await;
    }
    if query_matches(&req, &token) {
        return redirect_and_set_cookie(&req, &token);
    }
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "unauthorized\n",
    )
        .into_response()
}

fn cookie_matches(req: &Request, token: &Uuid) -> bool {
    let token_str = token.to_string();
    req.headers().get_all(header::COOKIE).iter().any(|v| {
        v.to_str()
            .ok()
            .and_then(|s| extract_cookie(s, SHARE_COOKIE))
            .map(|val| val == token_str)
            .unwrap_or(false)
    })
}

fn query_matches(req: &Request, token: &Uuid) -> bool {
    let Some(query) = req.uri().query() else {
        return false;
    };
    extract_query_param(query, SHARE_QUERY)
        .map(|v| v == token.to_string())
        .unwrap_or(false)
}

fn redirect_and_set_cookie(req: &Request, token: &Uuid) -> Response<Body> {
    let path = req.uri().path();
    let new_query = req
        .uri()
        .query()
        .map(|q| strip_query_param(q, SHARE_QUERY))
        .unwrap_or_default();
    let target = if new_query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{new_query}")
    };
    let cookie = format!(
        "{SHARE_COOKIE}={token}; HttpOnly; Secure; SameSite=Lax; Path=/",
        token = token
    );
    let location = HeaderValue::from_str(&target).unwrap_or_else(|_| HeaderValue::from_static("/"));
    let cookie_header =
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""));
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            (header::SET_COOKIE, cookie_header),
        ],
    )
        .into_response()
}

fn extract_cookie(header_value: &str, key: &str) -> Option<String> {
    for part in header_value.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn extract_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn strip_query_param(query: &str, key: &str) -> String {
    query
        .split('&')
        .filter(|p| !p.starts_with(&format!("{key}=")) && *p != key)
        .collect::<Vec<_>>()
        .join("&")
}

async fn ensure_binary(data_dir: &Path) -> Result<PathBuf> {
    if let Some(p) = which_on_path("cloudflared") {
        return Ok(p);
    }
    let target = platform_target()?;
    let bin_dir = data_dir.join("bin");
    tokio::fs::create_dir_all(&bin_dir).await.with_context(|| {
        format!(
            "creating cloudflared install directory at {}",
            bin_dir.display()
        )
    })?;
    let final_path = bin_dir.join(target.final_name);
    if final_path.exists() {
        return Ok(final_path);
    }

    let url = format!(
        "https://github.com/cloudflare/cloudflared/releases/download/{}/{}",
        CLOUDFLARED_VERSION, target.asset
    );
    let download_path = bin_dir.join(target.asset);
    tracing::info!(url = %url, dest = %download_path.display(), "downloading cloudflared");

    let status = Command::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(&download_path)
        .status()
        .await
        .context("running curl to download cloudflared (is curl installed?)")?;
    if !status.success() {
        return Err(anyhow!(
            "curl exited with status {status} while downloading cloudflared"
        ));
    }

    verify_sha256(&download_path, target.expected_sha256).await?;

    if target.is_tarball {
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(&download_path)
            .arg("-C")
            .arg(&bin_dir)
            .status()
            .await
            .context("running tar to extract cloudflared archive")?;
        if !status.success() {
            return Err(anyhow!("tar exited with status {status} extracting cloudflared"));
        }
        let _ = tokio::fs::remove_file(&download_path).await;
    } else if download_path != final_path {
        tokio::fs::rename(&download_path, &final_path)
            .await
            .with_context(|| {
                format!(
                    "renaming {} -> {}",
                    download_path.display(),
                    final_path.display()
                )
            })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&final_path)
            .await
            .with_context(|| format!("stat {}", final_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&final_path, perms)
            .await
            .with_context(|| format!("chmod +x {}", final_path.display()))?;
    }

    Ok(final_path)
}

struct PlatformTarget {
    asset: &'static str,
    final_name: &'static str,
    is_tarball: bool,
    expected_sha256: &'static str,
}

fn platform_target() -> Result<PlatformTarget> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("linux", "x86_64") => Ok(PlatformTarget {
            asset: "cloudflared-linux-amd64",
            final_name: "cloudflared",
            is_tarball: false,
            expected_sha256: "0095e46fdc88855d801c4d304cb1f5dd4bd656116c47ab94c2ad0ae7cda1c7ec",
        }),
        ("linux", "aarch64") => Ok(PlatformTarget {
            asset: "cloudflared-linux-arm64",
            final_name: "cloudflared",
            is_tarball: false,
            expected_sha256: "2dc0945345677d27de3ae390a31c3b168866b48766da5f4cfd3fc473ce572303",
        }),
        ("macos", "x86_64") => Ok(PlatformTarget {
            asset: "cloudflared-darwin-amd64.tgz",
            final_name: "cloudflared",
            is_tarball: true,
            expected_sha256: "7f2c4c8c86e787226804694112682aefacd4cfb98f54508f1a5a841a78bbbef9",
        }),
        ("macos", "aarch64") => Ok(PlatformTarget {
            asset: "cloudflared-darwin-arm64.tgz",
            final_name: "cloudflared",
            is_tarball: true,
            expected_sha256: "116ef11a59fc4f31e7f1bcc4378070cd7ca053fa37b4484b1432bb150b358219",
        }),
        ("windows", "x86_64") => Ok(PlatformTarget {
            asset: "cloudflared-windows-amd64.exe",
            final_name: "cloudflared.exe",
            is_tarball: false,
            expected_sha256: "f141cded099c239171ad2cea6fb5da0fdaa2bd36104c3074d883f9546519eba7",
        }),
        _ => Err(anyhow!(
            "cloudflared auto-install is not available for {os}/{arch}; install cloudflared manually and ensure it is on PATH"
        )),
    }
}

async fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let path_owned = path.to_path_buf();
    let expected_owned = expected.to_string();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut file = std::fs::File::open(&path_owned)
            .with_context(|| format!("opening downloaded cloudflared at {}", path_owned.display()))?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)
            .with_context(|| format!("hashing downloaded cloudflared at {}", path_owned.display()))?;
        let actual = format!("{:x}", hasher.finalize());
        if actual.eq_ignore_ascii_case(&expected_owned) {
            return Ok(());
        }
        let _ = std::fs::remove_file(&path_owned);
        Err(anyhow!(
            "cloudflared sha256 mismatch: expected {expected_owned}, got {actual}"
        ))
    })
    .await
    .context("joining verify_sha256 task")?
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(&exe);
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

fn map_binary_error(e: anyhow::Error) -> AppError {
    let msg = format!("{e:#}");
    if msg.contains("auto-install is not available") {
        AppError::Unrecoverable {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "cloudflared_unsupported_platform".into(),
            message: msg,
        }
    } else if msg.contains("sha256 mismatch") {
        AppError::Unrecoverable {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "cloudflared_sha_mismatch".into(),
            message: msg,
        }
    } else {
        AppError::Internal(e)
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

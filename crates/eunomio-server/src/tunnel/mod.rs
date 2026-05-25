// SPDX-License-Identifier: Apache-2.0

// Cloudflared upgrade procedure: bump `CLOUDFLARED_VERSION` (in `install.rs`),
// then update every `expected_sha256` by downloading each asset from the new
// release tag and running `sha256sum` (or `shasum -a 256`) on it. Hashes are
// computed over the published asset as-downloaded (tarballs are hashed
// before extraction, since Cloudflare publishes asset-level hashes).
use crate::{AppError, TunnelStateName, TunnelStatus};
use anyhow::{anyhow, Context, Result};
use axum::{http::StatusCode, middleware::from_fn_with_state, Router};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    net::TcpListener,
    sync::{broadcast, oneshot},
};
use uuid::Uuid;

mod auth;
mod install;
mod process;

const BROADCAST_CAPACITY: usize = 16;
#[derive(Clone)]
pub struct TunnelRegistry {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    state: Mutex<TunnelState>,
    events: broadcast::Sender<TunnelStatus>,
    data_dir: PathBuf,
    start_gate: tokio::sync::Mutex<()>,
    enabled: bool,
    allow_dev_url: bool,
}

pub(crate) enum TunnelState {
    Idle,
    Running(Running),
    Error { message: String, at: i64 },
}

pub(crate) struct Running {
    pub url: String,
    pub token: Option<Uuid>,
    pub started_at: i64,
    pub stop_tx: oneshot::Sender<()>,
}

impl TunnelRegistry {
    pub fn new(data_dir: PathBuf, enabled: bool, allow_dev_url: bool) -> Self {
        let (events, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(TunnelState::Idle),
                events,
                data_dir,
                start_gate: tokio::sync::Mutex::new(()),
                enabled,
                allow_dev_url,
            }),
        }
    }

    pub fn status(&self) -> TunnelStatus {
        let state = self.inner.state.lock().unwrap();
        snapshot(&state, &self.inner)
    }

    pub fn allow_dev_url(&self) -> bool {
        self.inner.allow_dev_url
    }

    /// Same as [`Self::status`] with the share token stripped. Use this for
    /// anything that might be observed via the tunnel itself.
    pub fn status_redacted(&self) -> TunnelStatus {
        let state = self.inner.state.lock().unwrap();
        snapshot_redacted(&state, &self.inner)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TunnelStatus> {
        self.inner.events.subscribe()
    }

    pub async fn start(&self, router: Router) -> Result<TunnelStatus, AppError> {
        ensure_enabled(&self.inner)?;
        match self.start_inner(router).await {
            Ok(dto) => Ok(dto),
            Err(e @ AppError::Conflict { .. }) => Err(e),
            Err(e) => Err(record_start_failure(&self.inner, e)),
        }
    }

    async fn start_inner(&self, router: Router) -> Result<TunnelStatus, AppError> {
        if self.inner.allow_dev_url {
            return Err(AppError::Conflict {
                code: "tunnel_external_dev".into(),
                message: "in-app tunnel is disabled with --allow-dev-url; use npm run dev".into(),
            });
        }

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

        let binary = install::ensure_binary(&self.inner.data_dir)
            .await
            .map_err(map_binary_error)?;

        let (port, token, serve_shutdown_tx) = spawn_local_auth_listener(router).await?;

        let (ready_tx, ready_rx) = oneshot::channel::<Result<TunnelStatus, AppError>>();
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let inner = self.inner.clone();
        tokio::spawn(async move {
            process::supervise(
                inner,
                binary,
                port,
                Some(token),
                ready_tx,
                stop_tx,
                stop_rx,
                Some(serve_shutdown_tx),
            )
            .await;
        });

        match ready_rx.await {
            Ok(Ok(dto)) => Ok(dto),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                self.set_error("cloudflared supervisor crashed".into());
                Err(AppError::Internal(anyhow!(
                    "cloudflared supervisor crashed before reporting status"
                )))
            }
        }
    }

    pub fn stop(&self) -> Result<(), AppError> {
        ensure_enabled(&self.inner)?;
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
        transition_to(
            &self.inner,
            TunnelState::Error {
                message,
                at: eunomio_core::unix_seconds(),
            },
        );
    }
}

fn ensure_enabled(inner: &Inner) -> Result<(), AppError> {
    if inner.enabled {
        Ok(())
    } else {
        Err(tunnel_disabled())
    }
}

fn tunnel_disabled() -> AppError {
    AppError::Unrecoverable {
        status: StatusCode::FORBIDDEN,
        code: "tunnel_disabled".into(),
        message: "tunnel sharing is not enabled".into(),
    }
}

fn record_start_failure(inner: &Arc<Inner>, err: AppError) -> AppError {
    match &err {
        AppError::Conflict { .. } => return err,
        AppError::Unrecoverable { code, .. } if code == "tunnel_disabled" => return err,
        _ => {}
    }
    let state = inner.state.lock().unwrap();
    if !matches!(*state, TunnelState::Error { .. }) {
        drop(state);
        transition_to(
            inner,
            TunnelState::Error {
                message: err.to_string(),
                at: eunomio_core::unix_seconds(),
            },
        );
    }
    err
}

async fn spawn_local_auth_listener(
    router: Router,
) -> Result<(u16, Uuid, oneshot::Sender<()>), AppError> {
    let token = Uuid::new_v4();
    let listener = TcpListener::bind(("127.0.0.1", 0u16))
        .await
        .context("binding tunnel auth listener")?;
    let port = listener
        .local_addr()
        .context("reading tunnel listener address")?
        .port();
    let auth_router = router.layer(from_fn_with_state(token, auth::check_token));
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, auth_router).with_graceful_shutdown(async move {
            let _ = rx.await;
        });
        if let Err(e) = server.await {
            tracing::error!(error = %e, "tunnel listener exited with error");
        }
    });
    Ok((port, token, tx))
}

/// Mutate the tunnel state and broadcast the redacted snapshot in one go.
/// Used by every transition that should be observable to subscribers
/// (`set_error`, the unexpected-exit branch in `process::supervise`, and
/// `process::fail_startup`).
pub(crate) fn transition_to(inner: &Arc<Inner>, next: TunnelState) {
    let mut state = inner.state.lock().unwrap();
    *state = next;
    let _ = inner.events.send(snapshot_redacted(&state, inner));
}

pub(crate) fn snapshot(state: &TunnelState, inner: &Inner) -> TunnelStatus {
    let token_required = inner.enabled && !inner.allow_dev_url;
    let enabled = inner.enabled;
    match state {
        TunnelState::Idle => TunnelStatus {
            enabled,
            state: TunnelStateName::Idle,
            token_required,
            url: None,
            token: None,
            started_at: None,
            error_message: None,
        },
        TunnelState::Running(r) => TunnelStatus {
            enabled,
            state: TunnelStateName::Running,
            token_required,
            url: Some(r.url.clone()),
            token: r.token.map(|t| t.to_string()),
            started_at: Some(r.started_at),
            error_message: None,
        },
        TunnelState::Error { message, at } => TunnelStatus {
            enabled,
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
/// broadcast that may be observed via the tunnel itself (so a holder of the
/// token cannot use the SSE stream to see future tokens after rotation).
pub(crate) fn snapshot_redacted(state: &TunnelState, inner: &Inner) -> TunnelStatus {
    let mut dto = snapshot(state, inner);
    dto.token = None;
    dto
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

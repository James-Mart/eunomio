use crate::{
    coordinator::Coordinator,
    credentials::KeyStore,
    cursor_bridge::{CursorHelperRunner, SubagentRunner},
    db,
    subagents::load_subagents,
    tunnel::TunnelRegistry,
    types::*,
};
use anyhow::{Context, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::OnceCell;
use tokio_rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentMode {
    Local,
    #[allow(dead_code)]
    Hosted,
}

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub data_dir: PathBuf,
    pub db: Connection,
    pub keystore: KeyStore,
    pub deployment_mode: DeploymentMode,
    pub cursor_models: OnceCell<Vec<CursorModel>>,
    pub coordinator: Coordinator,
    pub tunnel: TunnelRegistry,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct BuildStateOptions {
    pub data_dir: PathBuf,
    pub launch_key_hint: Option<String>,
    pub tunnel_enabled: bool,
    pub dev_tunnel: bool,
    pub runner: Option<Arc<dyn SubagentRunner>>,
}

pub async fn build_state(
    data_dir: PathBuf,
    launch_key_hint: Option<String>,
    tunnel_enabled: bool,
    dev_tunnel: bool,
) -> Result<AppState> {
    build_state_with_options(BuildStateOptions {
        data_dir,
        launch_key_hint,
        tunnel_enabled,
        dev_tunnel,
        runner: None,
    })
    .await
}

pub async fn build_state_with_runner(
    data_dir: PathBuf,
    launch_key_hint: Option<String>,
    tunnel_enabled: bool,
    dev_tunnel: bool,
    runner: Arc<dyn SubagentRunner>,
) -> Result<AppState> {
    build_state_with_options(BuildStateOptions {
        data_dir,
        launch_key_hint,
        tunnel_enabled,
        dev_tunnel,
        runner: Some(runner),
    })
    .await
}

pub async fn build_state_with_options(opts: BuildStateOptions) -> Result<AppState> {
    tokio::fs::create_dir_all(&opts.data_dir)
        .await
        .with_context(|| format!("create_dir_all {}", opts.data_dir.display()))?;
    let db = db::open(&opts.data_dir.join("eunomio.db")).await?;
    let keystore = KeyStore::new(opts.data_dir.clone(), opts.launch_key_hint);
    let tunnel = TunnelRegistry::new(opts.data_dir.clone(), opts.tunnel_enabled, opts.dev_tunnel);
    let subagents = load_subagents()?;
    let runner = opts.runner.unwrap_or_else(|| {
        Arc::new(CursorHelperRunner::new(opts.data_dir.clone())) as Arc<dyn SubagentRunner>
    });
    let coordinator = Coordinator::new(subagents, runner);
    let state = AppState(Arc::new(AppStateInner {
        data_dir: opts.data_dir,
        db,
        keystore,
        deployment_mode: DeploymentMode::Local,
        cursor_models: OnceCell::new(),
        coordinator: coordinator.clone(),
        tunnel,
    }));
    coordinator.process_startup_recovery(&state).await?;
    Ok(state)
}

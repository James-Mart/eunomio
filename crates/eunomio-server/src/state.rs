// SPDX-License-Identifier: Apache-2.0

use crate::{
    coordinator::Coordinator,
    launch::LaunchIntent,
    subagents::load_subagents,
    tunnel::TunnelRegistry,
};
use anyhow::{Context, Result};
use eunomio_core::traits::{AuthProvider, Datastore, KeyStore, QuotaEnforcer};
use eunomio_core::types::CursorModel;
use eunomio_helper_protocol::SubagentRunner;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub data_dir: PathBuf,
    pub datastore: Arc<dyn Datastore>,
    pub keystore: Arc<dyn KeyStore>,
    pub auth: Arc<dyn AuthProvider>,
    pub cursor_models: OnceCell<Vec<CursorModel>>,
    pub coordinator: Coordinator,
    pub tunnel: TunnelRegistry,
    pub launch: LaunchIntent,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct BuildStateOptions {
    pub data_dir: PathBuf,
    pub datastore: Arc<dyn Datastore>,
    pub keystore: Arc<dyn KeyStore>,
    pub auth: Arc<dyn AuthProvider>,
    pub runner: Arc<dyn SubagentRunner>,
    pub quota: Arc<dyn QuotaEnforcer>,
    pub launch_pull_request: Option<String>,
    pub tunnel_enabled: bool,
    pub dev_tunnel: bool,
}

pub async fn build_state(opts: BuildStateOptions) -> Result<AppState> {
    tokio::fs::create_dir_all(&opts.data_dir)
        .await
        .with_context(|| format!("create_dir_all {}", opts.data_dir.display()))?;
    let tunnel = TunnelRegistry::new(opts.data_dir.clone(), opts.tunnel_enabled, opts.dev_tunnel);
    let subagents = load_subagents()?;
    let coordinator = Coordinator::new(subagents, opts.runner, opts.quota);
    let state = AppState(Arc::new(AppStateInner {
        data_dir: opts.data_dir,
        datastore: opts.datastore,
        keystore: opts.keystore,
        auth: opts.auth,
        cursor_models: OnceCell::new(),
        coordinator: coordinator.clone(),
        tunnel,
        launch: LaunchIntent::new(opts.launch_pull_request),
    }));
    coordinator.process_startup_recovery(&state).await?;
    Ok(state)
}

use crate::{
    coordinator::Coordinator,
    cursor_bridge::{CursorHelperRunner, SubagentRunner},
    db,
    partition_settings::PartitionSettingsStore,
    subagents::load_subagents,
    tunnel::TunnelRegistry,
    types::*,
};
use anyhow::{Context, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::OnceCell;
use tokio_rusqlite::Connection;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub data_dir: PathBuf,
    pub db: Connection,
    pub cursor_api_key: Option<String>,
    pub cursor_models: OnceCell<Vec<CursorModel>>,
    pub partition_settings: PartitionSettingsStore,
    pub coordinator: Coordinator,
    pub tunnel: TunnelRegistry,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn build_state(
    data_dir: PathBuf,
    cursor_api_key: Option<String>,
    dev_tunnel: bool,
) -> Result<AppState> {
    let runner: Arc<dyn SubagentRunner> = Arc::new(CursorHelperRunner::new(
        cursor_api_key.clone(),
        data_dir.clone(),
    ));
    build_state_with_runner(data_dir, cursor_api_key, dev_tunnel, runner).await
}

pub async fn build_state_with_runner(
    data_dir: PathBuf,
    cursor_api_key: Option<String>,
    dev_tunnel: bool,
    runner: Arc<dyn SubagentRunner>,
) -> Result<AppState> {
    tokio::fs::create_dir_all(&data_dir)
        .await
        .with_context(|| format!("create_dir_all {}", data_dir.display()))?;
    let db = db::open(&data_dir.join("eunomia.db")).await?;
    let settings_path = data_dir.join("settings.json");
    let partition_settings = PartitionSettingsStore::load(settings_path).await?;
    let tunnel = TunnelRegistry::new(data_dir.clone(), dev_tunnel);
    let subagents = load_subagents()?;
    let coordinator = Coordinator::new(subagents, runner);
    let state = AppState(Arc::new(AppStateInner {
        data_dir,
        db,
        cursor_api_key,
        cursor_models: OnceCell::new(),
        partition_settings,
        coordinator: coordinator.clone(),
        tunnel,
    }));
    coordinator.process_startup_recovery(&state).await?;
    Ok(state)
}

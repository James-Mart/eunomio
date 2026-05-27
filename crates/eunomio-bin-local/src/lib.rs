// SPDX-License-Identifier: Apache-2.0

pub mod cli;

use eunomio_auth_local::LocalAuthProvider;
use eunomio_core::traits::{
    AuthProvider, Datastore, KeyStore, NoopQuotaEnforcer, QuotaEnforcer, SandboxRuntime,
};
use eunomio_helper_protocol::SubagentRunner;
use eunomio_keystore_file::FileKeyStore;
use eunomio_sandbox_linux::LinuxSandboxRuntime;
use eunomio_server::cursor_bridge::CursorHelperRunner;
use eunomio_server::{build_state, AppState, BuildStateOptions};
use eunomio_sqlite::SqliteDatastore;
use anyhow::Context;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub async fn ensure_data_dir(data_dir: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(data_dir)
        .await
        .with_context(|| format!("create_dir_all {}", data_dir.display()))
}

/// Wire the local deployment stack the same way `main.rs` does (for integration tests).
pub async fn build_local_state(
    data_dir: PathBuf,
    launch_key_hint: Option<String>,
    launch_pull_request: Option<String>,
    runner: Option<Arc<dyn SubagentRunner>>,
) -> anyhow::Result<AppState> {
    ensure_data_dir(&data_dir).await?;
    let datastore: Arc<dyn Datastore> =
        Arc::new(SqliteDatastore::open(&data_dir.join("eunomio.db")).await?);
    let keystore: Arc<dyn KeyStore> =
        Arc::new(FileKeyStore::new(data_dir.clone(), launch_key_hint));
    let auth: Arc<dyn AuthProvider> = Arc::new(LocalAuthProvider::new(
        datastore.clone(),
        keystore.clone(),
        data_dir.clone(),
    ));
    let sandbox: Arc<dyn SandboxRuntime> = Arc::new(LinuxSandboxRuntime::new());
    let quota: Arc<dyn QuotaEnforcer> = Arc::new(NoopQuotaEnforcer::new());
    let runner = runner.unwrap_or_else(|| {
        Arc::new(CursorHelperRunner::new(data_dir.clone(), sandbox.clone()))
            as Arc<dyn SubagentRunner>
    });
    build_state(BuildStateOptions {
        data_dir,
        datastore,
        keystore,
        auth,
        runner,
        quota,
        launch_pull_request,
        tunnel_enabled: false,
        allow_dev_url: false,
    })
    .await
}

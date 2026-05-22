#![allow(dead_code)]

use axum::{http::StatusCode, Router};
use eunomio::cursor_bridge::SubagentRunner;
use eunomio::server::router;
use eunomio::state::{build_state, build_state_with_options, build_state_with_runner, BuildStateOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

use super::git::default_repo;
use super::http::login;

pub const TEST_USERNAME: &str = "testuser";
pub const TEST_CURSOR_KEY: &str = "test-cursor-key";

pub struct TestApp {
    pub router: Router,
    pub repo: TempDir,
    pub data: TempDir,
    pub state: eunomio::state::AppState,
    pub cookie: Option<String>,
}

impl TestApp {
    pub async fn spawn() -> Self {
        Self::spawn_with_repo(default_repo).await
    }

    pub async fn spawn_authenticated() -> Self {
        let mut app = Self::spawn().await;
        let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
        app.cookie = Some(cookie);
        app
    }

    pub async fn spawn_authenticated_with_repo<F>(setup: F) -> Self
    where
        F: FnOnce(&Path),
    {
        let mut app = Self::spawn_with_repo(setup).await;
        let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
        app.cookie = Some(cookie);
        app
    }

    pub async fn spawn_with_repo<F>(setup: F) -> Self
    where
        F: FnOnce(&Path),
    {
        Self::build_with_repo(setup, None).await
    }

    pub async fn spawn_with_runner(runner: Arc<dyn SubagentRunner>) -> Self {
        Self::spawn_with_repo_and_runner(default_repo, runner).await
    }

    pub async fn spawn_authenticated_with_runner(runner: Arc<dyn SubagentRunner>) -> Self {
        let mut app = Self::spawn_with_repo_and_runner(default_repo, runner).await;
        let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
        app.cookie = Some(cookie);
        app
    }

    pub async fn spawn_with_repo_and_runner<F>(setup: F, runner: Arc<dyn SubagentRunner>) -> Self
    where
        F: FnOnce(&Path),
    {
        Self::build_with_repo(setup, Some(runner)).await
    }

    pub async fn spawn_with_launch_key_hint() -> Self {
        Self::spawn_with_launch_key_hint_and_repo(default_repo).await
    }

    pub async fn spawn_with_launch_key_hint_and_repo<F>(setup: F) -> Self
    where
        F: FnOnce(&Path),
    {
        let repo = tempfile::tempdir().expect("tempdir for repo");
        let data = tempfile::tempdir().expect("tempdir for data");
        setup(repo.path());
        let data_root = data.path().canonicalize().expect("canonicalise data path");
        let state = build_state_with_options(BuildStateOptions {
            data_dir: data_root,
            launch_key_hint: Some("env-launch-key".to_string()),
            tunnel_enabled: false,
            dev_tunnel: false,
            runner: None,
        })
        .await
        .expect("build_state_with_options");
        let router = router(state.clone());
        TestApp {
            router,
            repo,
            data,
            state,
            cookie: None,
        }
    }

    async fn build_with_repo<F>(setup: F, runner: Option<Arc<dyn SubagentRunner>>) -> Self
    where
        F: FnOnce(&Path),
    {
        let repo = tempfile::tempdir().expect("tempdir for repo");
        let data = tempfile::tempdir().expect("tempdir for data");
        setup(repo.path());
        let data_root = data.path().canonicalize().expect("canonicalise data path");
        let state = if let Some(runner) = runner {
            build_state_with_runner(data_root, None, false, false, runner)
                .await
                .expect("build_state_with_runner")
        } else {
            build_state(data_root, None, false, false)
                .await
                .expect("build_state")
        };
        let router = router(state.clone());
        TestApp {
            router,
            repo,
            data,
            state,
            cookie: None,
        }
    }

    pub fn repo_path(&self) -> PathBuf {
        self.repo.path().canonicalize().unwrap()
    }

    pub fn cookie(&self) -> &str {
        self.cookie.as_deref().expect("TestApp not authenticated")
    }

    pub async fn auth_json(
        &self,
        method: &str,
        path: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        super::http::authenticated_json_request(&self.router, self.cookie(), method, path, body)
            .await
    }

    pub async fn auth_empty(&self, method: &str, path: &str) -> (StatusCode, serde_json::Value) {
        super::http::authenticated_empty_request(&self.router, self.cookie(), method, path).await
    }
}

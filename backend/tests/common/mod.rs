use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use eunomia::cursor_bridge::SubagentRunner;
use eunomia::server::router;
use eunomia::state::{build_state, build_state_with_runner, AppState};
use http_body_util::BodyExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

#[allow(dead_code)]
pub struct TestApp {
    pub router: Router,
    pub repo: TempDir,
    pub data: TempDir,
    pub state: AppState,
}

impl TestApp {
    #[allow(dead_code)]
    pub async fn spawn() -> Self {
        Self::spawn_with_repo(default_repo).await
    }

    pub async fn spawn_with_repo<F>(setup: F) -> Self
    where
        F: FnOnce(&Path),
    {
        let repo = tempfile::tempdir().expect("tempdir for repo");
        let data = tempfile::tempdir().expect("tempdir for data");
        setup(repo.path());
        let data_root = data.path().canonicalize().expect("canonicalise data path");
        let state = build_state(data_root, None, false, false)
            .await
            .expect("build_state");
        let router = router(state.clone());
        TestApp {
            router,
            repo,
            data,
            state,
        }
    }

    #[allow(dead_code)]
    pub async fn spawn_with_runner(runner: Arc<dyn SubagentRunner>) -> Self {
        Self::spawn_with_repo_and_runner(default_repo, runner).await
    }

    #[allow(dead_code)]
    pub async fn spawn_with_repo_and_runner<F>(setup: F, runner: Arc<dyn SubagentRunner>) -> Self
    where
        F: FnOnce(&Path),
    {
        let repo = tempfile::tempdir().expect("tempdir for repo");
        let data = tempfile::tempdir().expect("tempdir for data");
        setup(repo.path());
        let data_root = data.path().canonicalize().expect("canonicalise data path");
        let state = build_state_with_runner(data_root, None, false, false, runner)
            .await
            .expect("build_state_with_runner");
        let router = router(state.clone());
        TestApp {
            router,
            repo,
            data,
            state,
        }
    }

    #[allow(dead_code)]
    pub fn repo_path(&self) -> PathBuf {
        self.repo.path().canonicalize().unwrap()
    }
}

pub fn local_session_body(repo: &Path, base_ref: &str, source_ref: &str) -> serde_json::Value {
    serde_json::json!({
        "remoteUrl": repo.canonicalize().unwrap().display().to_string(),
        "baseRef": base_ref,
        "sourceRef": source_ref,
    })
}

pub fn default_repo(path: &Path) {
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "test@example.com"]);
    git(path, &["config", "user.name", "Test"]);
    write(path, "a.txt", "a\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "base commit"]);
    git(path, &["checkout", "-q", "-b", "feature"]);
    write(path, "b.txt", "b\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "add b"]);
    git(path, &["checkout", "-q", "main"]);
}

pub fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {:?} spawn: {e}", args));
    if !out.status.success() {
        panic!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn write(repo: &Path, rel: &str, contents: &str) {
    let full = repo.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, contents).unwrap();
}

pub async fn json_request(
    router: &Router,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            panic!(
                "non-JSON body for {method} {path}: {e}; body={}",
                String::from_utf8_lossy(&bytes)
            )
        })
    };
    (status, value)
}

pub async fn empty_request(
    router: &Router,
    method: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, value)
}

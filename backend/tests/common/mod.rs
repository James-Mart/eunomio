use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use eunomio::auth::session::COOKIE_NAME;
use eunomio::cursor_bridge::SubagentRunner;
use eunomio::repo::{org, user};
use eunomio::server::router;
use eunomio::state::{build_state, build_state_with_options, build_state_with_runner, AppState, BuildStateOptions};
use http_body_util::BodyExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

pub const TEST_USERNAME: &str = "testuser";
pub const TEST_CURSOR_KEY: &str = "test-cursor-key";
const CSRF_HEADER: &str = "X-Eunomio-Request";

#[allow(dead_code)]
pub struct TestApp {
    pub router: Router,
    pub repo: TempDir,
    pub data: TempDir,
    pub state: AppState,
    pub cookie: Option<String>,
}

impl TestApp {
    #[allow(dead_code)]
    pub async fn spawn() -> Self {
        Self::spawn_with_repo(default_repo).await
    }

    #[allow(dead_code)]
    pub async fn spawn_authenticated() -> Self {
        let mut app = Self::spawn().await;
        let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
        app.cookie = Some(cookie);
        app
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub async fn spawn_with_runner(runner: Arc<dyn SubagentRunner>) -> Self {
        Self::spawn_with_repo_and_runner(default_repo, runner).await
    }

    #[allow(dead_code)]
    pub async fn spawn_authenticated_with_runner(runner: Arc<dyn SubagentRunner>) -> Self {
        let mut app = Self::spawn_with_repo_and_runner(default_repo, runner).await;
        let cookie = login(&app.router, TEST_USERNAME, TEST_CURSOR_KEY).await;
        app.cookie = Some(cookie);
        app
    }

    #[allow(dead_code)]
    pub async fn spawn_with_repo_and_runner<F>(setup: F, runner: Arc<dyn SubagentRunner>) -> Self
    where
        F: FnOnce(&Path),
    {
        Self::build_with_repo(setup, Some(runner)).await
    }

    #[allow(dead_code)]
    pub async fn spawn_with_launch_key_hint() -> Self {
        Self::spawn_with_launch_key_hint_and_repo(default_repo).await
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn repo_path(&self) -> PathBuf {
        self.repo.path().canonicalize().unwrap()
    }

    #[allow(dead_code)]
    pub fn cookie(&self) -> &str {
        self.cookie.as_deref().expect("TestApp not authenticated")
    }

    #[allow(dead_code)]
    pub async fn auth_json(
        &self,
        method: &str,
        path: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        authenticated_json_request(&self.router, self.cookie(), method, path, body).await
    }

    #[allow(dead_code)]
    pub async fn auth_empty(&self, method: &str, path: &str) -> (StatusCode, serde_json::Value) {
        authenticated_empty_request(&self.router, self.cookie(), method, path).await
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

pub fn parse_session_cookie(set_cookie: &str) -> String {
    for part in set_cookie.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
            if !value.is_empty() {
                return format!("{COOKIE_NAME}={value}");
            }
        }
    }
    panic!("no {COOKIE_NAME} in Set-Cookie: {set_cookie}");
}

pub async fn insert_local_fixture(state: &AppState) -> (String, String) {
    org::ensure_singleton_local(state).await.expect("ensure org");
    let user_row = user::insert(state, "fixture-user")
        .await
        .expect("insert user");
    user::ensure_membership(state, org::LOCAL_ORG_ID, &user_row.id, "Owner")
        .await
        .expect("membership");
    (org::LOCAL_ORG_ID.to_string(), user_row.id)
}

fn csrf_request(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(CSRF_HEADER, "1")
        .header(header::HOST, "127.0.0.1")
}

async fn send(
    router: &Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            panic!(
                "non-JSON body: {e}; body={}",
                String::from_utf8_lossy(&bytes)
            )
        })
    };
    (status, headers, value)
}

pub async fn login(router: &Router, username: &str, cursor_api_key: &str) -> String {
    let req = csrf_request("POST", "/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({
                "username": username,
                "cursorApiKey": cursor_api_key,
            }))
            .unwrap(),
        ))
        .unwrap();
    let (status, headers, body) = send(router, req).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "login failed for {username}: {body}"
    );
    let set_cookie = headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("login response missing Set-Cookie");
    parse_session_cookie(set_cookie)
}

pub async fn json_request(
    router: &Router,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

pub async fn empty_request(
    router: &Router,
    method: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path).body(Body::empty()).unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

pub async fn request_with_headers(
    router: &Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    send(router, req).await
}

pub async fn authenticated_json_request(
    router: &Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path)
        .header("content-type", "application/json")
        .header(header::COOKIE, cookie)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

pub async fn authenticated_empty_request(
    router: &Router,
    cookie: &str,
    method: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let req = csrf_request(method, path)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let (status, _, value) = send(router, req).await;
    (status, value)
}

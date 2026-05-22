use crate::{error::AppError, git, types::RepoHints};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

type CloneLockMap = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;

static CLONE_LOCKS: std::sync::OnceLock<CloneLockMap> = std::sync::OnceLock::new();

fn clone_locks() -> CloneLockMap {
    CLONE_LOCKS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

async fn with_slug_lock<T, E, F, Fut>(slug: &str, _data_dir: &Path, f: F) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let locks = clone_locks();
    let lock = {
        let mut map = locks.lock().await;
        map.entry(slug.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().await;
    f().await
}

#[derive(Debug, Clone)]
pub struct ParsedRemote {
    pub literal_remote: String,
    pub is_local: bool,
    pub normalized_remote: String,
}

pub fn parse_remote_url(input: &str) -> Result<ParsedRemote, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("remoteUrl is required".into()));
    }

    let is_local = git::is_local_repo_input(trimmed);
    let literal = if is_local {
        resolve_local_literal(trimmed)?
    } else {
        trimmed.to_string()
    };

    if is_local && !std::path::Path::new(&literal).is_dir() {
        return Err(AppError::BadRequest(format!(
            "local path {} is not a directory",
            literal
        )));
    }

    let normalized_remote = git::normalize_remote_identity(&literal, is_local);
    Ok(ParsedRemote {
        literal_remote: literal,
        is_local,
        normalized_remote,
    })
}

fn resolve_local_literal(input: &str) -> Result<String, AppError> {
    let path_str = input.strip_prefix("file://").unwrap_or(input);
    let path = std::path::Path::new(path_str);
    let canonical = path.canonicalize().map_err(|e| {
        AppError::BadRequest(format!("invalid local path {path_str}: {e}"))
    })?;
    Ok(canonical.to_string_lossy().into_owned())
}

pub fn clone_path(data_dir: &Path, normalized_remote: &str) -> PathBuf {
    let slug = git::slug_from_identity(normalized_remote);
    data_dir.join("repos").join(slug)
}

pub fn git_root(data_dir: &Path, parsed: &ParsedRemote) -> PathBuf {
    if parsed.is_local {
        PathBuf::from(&parsed.literal_remote)
    } else {
        clone_path(data_dir, &parsed.normalized_remote)
    }
}

pub async fn materialize_git_root(
    data_dir: &Path,
    parsed: &ParsedRemote,
) -> Result<PathBuf, AppError> {
    if parsed.is_local {
        git::ensure_repo(Path::new(&parsed.literal_remote))
            .await
            .map_err(|e| AppError::BadRequest(format!("{} is not a git repository: {e}", parsed.literal_remote)))?;
        return Ok(PathBuf::from(&parsed.literal_remote));
    }

    let slug = git::slug_from_identity(&parsed.normalized_remote);
    let clone_dir = clone_path(data_dir, &parsed.normalized_remote);
    tokio::fs::create_dir_all(data_dir.join("repos"))
        .await
        .map_err(|e| AppError::Internal(anyhow!("create repos dir: {e}")))?;

    with_slug_lock(&slug, data_dir, || async {
        if clone_dir.exists() {
            git::remote_set_url(&clone_dir, &parsed.literal_remote)
                .await
                .map_err(|e| AppError::BadRequest(format!("git remote set-url: {e}")))?;
            git::fetch_origin(&clone_dir)
                .await
                .map_err(|e| AppError::BadRequest(format!("git fetch: {e}")))?;
        } else {
            git::clone_bare(&parsed.literal_remote, &clone_dir)
                .await
                .map_err(|e| AppError::BadRequest(format!("git clone --bare: {e}")))?;
        }
        Ok(clone_dir)
    })
    .await
}

pub async fn fetch_for_session(
    data_dir: &Path,
    normalized_remote: &str,
    literal_remote: &str,
    is_local: bool,
) -> Result<(), AppError> {
    if is_local {
        return Ok(());
    }
    let git_root = clone_path(data_dir, normalized_remote);
    git::remote_set_url(&git_root, literal_remote)
        .await
        .map_err(|e| AppError::BadRequest(format!("git remote set-url: {e}")))?;
    git::fetch_origin(&git_root)
        .await
        .map_err(|e| AppError::BadRequest(format!("git fetch: {e}")))?;
    Ok(())
}

pub async fn maybe_remove_clone(
    data_dir: &Path,
    normalized_remote: &str,
    is_local: bool,
    remaining_sessions: i64,
) -> Result<(), AppError> {
    if is_local || remaining_sessions > 0 {
        return Ok(());
    }
    if !normalized_remote.starts_with("remote:") {
        return Ok(());
    }
    let clone_dir = clone_path(data_dir, normalized_remote);
    if clone_dir.exists() {
        tokio::fs::remove_dir_all(&clone_dir)
            .await
            .map_err(|e| AppError::Internal(anyhow!("remove clone dir: {e}")))?;
    }
    let lock_path = data_dir
        .join("repos")
        .join(format!("{}.lock", git::slug_from_identity(normalized_remote)));
    let _ = tokio::fs::remove_file(lock_path).await;
    Ok(())
}

pub async fn cwd_hints() -> RepoHints {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return RepoHints::default(),
    };
    if git::ensure_repo(&cwd).await.is_err() {
        return RepoHints::default();
    }
    let literal = cwd.to_string_lossy().into_owned();
    let branch = git::current_branch(&cwd).await.ok().flatten();
    let trunk = git::detect_trunk_ref(&cwd)
        .await
        .map(|t| format!("origin/{t}"))
        .or_else(|| Some("origin/main".to_string()));
    RepoHints {
        suggested_remote_url: Some(literal),
        suggested_source_ref: branch,
        suggested_base_ref: trunk,
    }
}

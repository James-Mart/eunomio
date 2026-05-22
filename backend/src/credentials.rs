use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct KeyStore {
    data_dir: PathBuf,
    launch_key_hint: Arc<Mutex<Option<String>>>,
}

impl KeyStore {
    pub fn new(data_dir: PathBuf, launch_key_hint: Option<String>) -> Self {
        Self {
            data_dir,
            launch_key_hint: Arc::new(Mutex::new(launch_key_hint)),
        }
    }

    pub fn has_launch_key_hint(&self) -> bool {
        self.launch_key_hint.lock().unwrap().is_some()
    }

    pub fn take_launch_key_hint(&self) -> Option<String> {
        self.launch_key_hint.lock().unwrap().take()
    }

    fn credentials_path(&self, user_id: &str) -> PathBuf {
        self.data_dir
            .join("users")
            .join(user_id)
            .join("credentials")
    }

    pub async fn get(&self, user_id: &str) -> Result<Option<String>> {
        let path = self.credentials_path(user_id);
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => {
                let parsed: serde_json::Value = serde_json::from_str(&text)
                    .with_context(|| format!("parsing credentials at {}", path.display()))?;
                Ok(parsed
                    .get("cursorApiKey")
                    .and_then(|v| v.as_str())
                    .map(str::to_string))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub async fn set(&self, user_id: &str, key: &str) -> Result<()> {
        let path = self.credentials_path(user_id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create_dir_all {}", parent.display()))?;
        }
        let body = serde_json::json!({ "cursorApiKey": key });
        tokio::fs::write(&path, serde_json::to_string_pretty(&body)?)
            .await
            .with_context(|| format!("writing {}", path.display()))?;
        set_file_mode_0600(&path)?;
        Ok(())
    }
}

#[cfg(unix)]
fn set_file_mode_0600(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_mode_0600(_path: &Path) -> Result<()> {
    Ok(())
}

pub async fn read_last_username(data_dir: &Path) -> Option<String> {
    let path = data_dir.join("last_username");
    tokio::fs::read_to_string(&path)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub async fn write_last_username(data_dir: &Path, username: &str) -> Result<()> {
    let path = data_dir.join("last_username");
    tokio::fs::write(&path, username).await?;
    Ok(())
}

pub async fn suggested_username(data_dir: &Path) -> String {
    if let Some(u) = read_last_username(data_dir).await {
        return u;
    }
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into())
}

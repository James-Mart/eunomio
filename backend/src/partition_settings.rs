use crate::{error::AppError, types::*};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct PartitionSettingsStore {
    path: Arc<PathBuf>,
    inner: Arc<RwLock<PartitionSettings>>,
}

impl PartitionSettingsStore {
    pub async fn load(path: PathBuf) -> Result<Self> {
        let value = read_or_init(&path).await?;
        Ok(Self {
            path: Arc::new(path),
            inner: Arc::new(RwLock::new(value)),
        })
    }

    pub async fn snapshot(&self) -> PartitionSettings {
        self.inner.read().await.clone()
    }

    pub async fn apply_patch(
        &self,
        patch: PartitionSettingsPatch,
    ) -> Result<PartitionSettings, AppError> {
        let mut guard = self.inner.write().await;
        let mut merged = guard.clone();
        merged.apply_patch(patch);
        let serialized = serde_json::to_string_pretty(&merged)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("serializing settings: {e}")))?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("creating settings dir: {e}")))?;
        }
        tokio::fs::write(&*self.path, serialized.as_bytes())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("writing settings: {e}")))?;
        *guard = merged.clone();
        Ok(merged)
    }
}

async fn read_or_init(path: &PathBuf) -> Result<PartitionSettings> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("parsing partition settings at {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let initial = PartitionSettings::default();
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("create_dir_all {}", parent.display()))?;
            }
            let serialized = serde_json::to_string_pretty(&initial)
                .context("serializing default partition settings")?;
            tokio::fs::write(path, serialized.as_bytes())
                .await
                .with_context(|| format!("writing default settings to {}", path.display()))?;
            Ok(initial)
        }
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn resolve_model(settings: &PartitionSettings, role: PhaseName) -> String {
    let role_settings = match role {
        PhaseName::Survey => &settings.surveyor,
        PhaseName::Plan => &settings.planner,
        PhaseName::Construct => &settings.constructor,
    };
    if role_settings.override_model {
        role_settings.model.clone()
    } else {
        settings.coordinator.model.clone()
    }
}

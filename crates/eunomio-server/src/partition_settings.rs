// SPDX-License-Identifier: Apache-2.0

use eunomio_core::types::*;
use crate::{AppError, state::AppState };
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn user_settings_path(data_dir: &Path, user_id: &str) -> PathBuf {
    data_dir.join("users").join(user_id).join("settings.json")
}

/// Load partition settings for a user from their settings file.
pub async fn load_for_user(data_dir: &Path, user_id: &str) -> Result<PartitionSettings, AppError> {
    let user_path = user_settings_path(data_dir, user_id);
    read_or_init(&user_path)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))
}

pub async fn save_for_user(
    data_dir: &Path,
    user_id: &str,
    settings: &PartitionSettings,
) -> Result<(), AppError> {
    let path = user_settings_path(data_dir, user_id);
    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serializing settings: {e}")))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("creating settings dir: {e}")))?;
    }
    tokio::fs::write(&path, serialized.as_bytes())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("writing settings: {e}")))?;
    Ok(())
}

/// Resolve settings for a partition via partition → session → user_id.
pub async fn load_for_partition(
    state: &AppState,
    org_id: &str,
    session_id: &str,
) -> Result<PartitionSettings, AppError> {
    let user_id = state.datastore.sessions().user_id(org_id, session_id).await?;
    load_for_user(&state.data_dir, &user_id).await
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

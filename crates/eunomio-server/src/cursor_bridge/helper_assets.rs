// SPDX-License-Identifier: Apache-2.0

use super::unavailable;
use crate::AppError;
use rust_embed::Embed;
use std::path::{Path, PathBuf};

#[derive(Embed)]
#[folder = "../../helper/dist/"]
pub(super) struct HelperAssets;

const HELPER_FILE: &str = "cursor-helper";

/// Native bindings the helper bundle dlopens at runtime. Each entry must live
/// in `helper/dist/` (placed there by `helper/build.mjs`) and be embedded
/// alongside `cursor-helper`. They are extracted into the same temp directory
/// so `helper/src/bindings-loader.cjs` can find them next to `process.execPath`.
const HELPER_NATIVE_FILES: &[&str] = &["node_sqlite3.node"];
const HELPER_EXECUTABLE_NATIVE_FILES: &[&str] = &["rg"];

pub(super) async fn ensure_helper_extracted(data_dir: &Path) -> Result<PathBuf, AppError> {
    let version = env!("CARGO_PKG_VERSION");
    let dir = data_dir.join("helper").join(version);
    create_private_dir(&dir).await?;
    extract_helper_asset(&dir, HELPER_FILE, true).await?;
    for name in HELPER_NATIVE_FILES {
        extract_helper_asset(&dir, name, false).await?;
    }
    for name in HELPER_EXECUTABLE_NATIVE_FILES {
        extract_helper_asset(&dir, name, true).await?;
    }
    Ok(dir.join(HELPER_FILE))
}

async fn create_private_dir(dir: &Path) -> Result<(), AppError> {
    let dir_owned = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&dir_owned)
    })
    .await
    .map_err(|e| unavailable(&format!("spawn_blocking for helper dir: {e}")))?
    .map_err(|e| unavailable(&format!("creating helper dir {}: {e}", dir.display())))
}

async fn extract_helper_asset(dir: &Path, name: &str, executable: bool) -> Result<(), AppError> {
    let target = dir.join(name);
    if target.exists() {
        return Ok(());
    }
    let asset = HelperAssets::get(name)
        .ok_or_else(|| unavailable(&format!("{name} not embedded in this build")))?;
    let tmp = dir.join(format!("{name}.tmp"));
    tokio::fs::write(&tmp, asset.data.as_ref())
        .await
        .map_err(|e| unavailable(&format!("writing helper asset {name}: {e}")))?;
    if executable {
        crate::process_util::set_executable(&tmp)
            .await
            .map_err(|e| unavailable(&format!("chmod helper asset {name}: {e}")))?;
    }
    if let Err(e) = tokio::fs::rename(&tmp, &target).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        if !target.exists() {
            return Err(unavailable(&format!("renaming helper asset {name}: {e}")));
        }
    }
    Ok(())
}

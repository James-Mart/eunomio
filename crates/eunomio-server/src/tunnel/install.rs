// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::process::Command;

const CLOUDFLARED_VERSION: &str = "2026.5.0";

pub(super) async fn ensure_binary(data_dir: &Path) -> Result<PathBuf> {
    if let Some(p) = which_on_path("cloudflared") {
        return Ok(p);
    }
    let target = platform_target()?;
    let bin_dir = data_dir.join("bin");
    tokio::fs::create_dir_all(&bin_dir).await.with_context(|| {
        format!(
            "creating cloudflared install directory at {}",
            bin_dir.display()
        )
    })?;
    let final_path = bin_dir.join(target.final_name);
    if final_path.exists() {
        return Ok(final_path);
    }

    let download_path = bin_dir.join(target.asset);
    download_cloudflared(target, &download_path).await?;
    verify_sha256(&download_path, target.expected_sha256).await?;
    install_from_download(target, &download_path, &final_path, &bin_dir).await?;
    crate::process_util::set_executable(&final_path).await?;
    Ok(final_path)
}

async fn download_cloudflared(target: &PlatformTarget, dest: &Path) -> Result<()> {
    let url = format!(
        "https://github.com/cloudflare/cloudflared/releases/download/{}/{}",
        CLOUDFLARED_VERSION, target.asset
    );
    tracing::info!(url = %url, dest = %dest.display(), "downloading cloudflared");

    let status = Command::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(dest)
        .status()
        .await
        .context("running curl to download cloudflared (is curl installed?)")?;
    if !status.success() {
        return Err(anyhow!(
            "curl exited with status {status} while downloading cloudflared"
        ));
    }
    Ok(())
}

async fn install_from_download(
    target: &PlatformTarget,
    download_path: &Path,
    final_path: &Path,
    bin_dir: &Path,
) -> Result<()> {
    if target.is_tarball {
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(download_path)
            .arg("-C")
            .arg(bin_dir)
            .status()
            .await
            .context("running tar to extract cloudflared archive")?;
        if !status.success() {
            return Err(anyhow!(
                "tar exited with status {status} extracting cloudflared"
            ));
        }
        let _ = tokio::fs::remove_file(download_path).await;
    } else if download_path != final_path {
        tokio::fs::rename(download_path, final_path)
            .await
            .with_context(|| {
                format!(
                    "renaming {} -> {}",
                    download_path.display(),
                    final_path.display()
                )
            })?;
    }
    Ok(())
}

async fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let path_owned = path.to_path_buf();
    let expected_owned = expected.to_string();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut file = std::fs::File::open(&path_owned).with_context(|| {
            format!("opening downloaded cloudflared at {}", path_owned.display())
        })?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher).with_context(|| {
            format!("hashing downloaded cloudflared at {}", path_owned.display())
        })?;
        let actual = format!("{:x}", hasher.finalize());
        if actual.eq_ignore_ascii_case(&expected_owned) {
            return Ok(());
        }
        let _ = std::fs::remove_file(&path_owned);
        Err(anyhow!(
            "cloudflared sha256 mismatch: expected {expected_owned}, got {actual}"
        ))
    })
    .await
    .context("joining verify_sha256 task")?
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(&exe);
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

struct PlatformTarget {
    asset: &'static str,
    final_name: &'static str,
    is_tarball: bool,
    expected_sha256: &'static str,
}

const PLATFORM_TARGETS: &[(&str, &str, PlatformTarget)] = &[
    (
        "linux",
        "x86_64",
        PlatformTarget {
            asset: "cloudflared-linux-amd64",
            final_name: "cloudflared",
            is_tarball: false,
            expected_sha256: "0095e46fdc88855d801c4d304cb1f5dd4bd656116c47ab94c2ad0ae7cda1c7ec",
        },
    ),
    (
        "linux",
        "aarch64",
        PlatformTarget {
            asset: "cloudflared-linux-arm64",
            final_name: "cloudflared",
            is_tarball: false,
            expected_sha256: "2dc0945345677d27de3ae390a31c3b168866b48766da5f4cfd3fc473ce572303",
        },
    ),
    (
        "macos",
        "x86_64",
        PlatformTarget {
            asset: "cloudflared-darwin-amd64.tgz",
            final_name: "cloudflared",
            is_tarball: true,
            expected_sha256: "7f2c4c8c86e787226804694112682aefacd4cfb98f54508f1a5a841a78bbbef9",
        },
    ),
    (
        "macos",
        "aarch64",
        PlatformTarget {
            asset: "cloudflared-darwin-arm64.tgz",
            final_name: "cloudflared",
            is_tarball: true,
            expected_sha256: "116ef11a59fc4f31e7f1bcc4378070cd7ca053fa37b4484b1432bb150b358219",
        },
    ),
    (
        "windows",
        "x86_64",
        PlatformTarget {
            asset: "cloudflared-windows-amd64.exe",
            final_name: "cloudflared.exe",
            is_tarball: false,
            expected_sha256: "f141cded099c239171ad2cea6fb5da0fdaa2bd36104c3074d883f9546519eba7",
        },
    ),
];

fn platform_target() -> Result<&'static PlatformTarget> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    PLATFORM_TARGETS
        .iter()
        .find_map(|(o, a, t)| (*o == os && *a == arch).then_some(t))
        .ok_or_else(|| anyhow!(
            "cloudflared auto-install is not available for {os}/{arch}; install cloudflared manually and ensure it is on PATH"
        ))
}

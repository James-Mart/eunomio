use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::Path;
use tokio::process::Command;

const MAX_BLOB_BYTES: usize = 512 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBlob {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

async fn run(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Returns Ok if `path` is inside a git repository, Err otherwise.
pub async fn ensure_repo(path: &Path) -> Result<()> {
    run(path, &["rev-parse", "--git-dir"]).await.map(|_| ())
}

pub async fn merge_base(repo: &Path, base: &str, source: &str) -> Result<String> {
    run(repo, &["merge-base", "--end-of-options", base, source]).await
}

pub async fn diff_text(repo: &Path, from_tree: &str, to_tree: &str) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--diff-algorithm=histogram",
            "--end-of-options",
            from_tree,
            to_tree,
        ])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git diff {} {}: {}",
            from_tree,
            to_tree,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn changed_files(
    repo: &Path,
    from_tree: &str,
    to_tree: &str,
) -> Result<Vec<FileBlob>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "diff",
            "--name-status",
            "-z",
            "-M",
            "--no-ext-diff",
            "--end-of-options",
            from_tree,
            to_tree,
        ])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git diff --name-status {} {}: {}",
            from_tree,
            to_tree,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let entries = parse_name_status_z(&out.stdout)?;
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        let old_content = match (&entry.status, &entry.old_path) {
            (NameStatus::Added, _) | (_, None) => None,
            (_, Some(path)) => fetch_blob_text(repo, from_tree, path).await?,
        };
        let new_content = match (&entry.status, &entry.new_path) {
            (NameStatus::Deleted, _) | (_, None) => None,
            (_, Some(path)) => fetch_blob_text(repo, to_tree, path).await?,
        };
        files.push(FileBlob {
            old_path: entry.old_path,
            new_path: entry.new_path,
            old_content,
            new_content,
        });
    }
    Ok(files)
}

#[derive(Debug, PartialEq, Eq)]
enum NameStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    TypeChanged,
    Other,
}

#[derive(Debug)]
struct NameStatusEntry {
    status: NameStatus,
    old_path: Option<String>,
    new_path: Option<String>,
}

fn parse_name_status_z(bytes: &[u8]) -> Result<Vec<NameStatusEntry>> {
    let mut entries = Vec::new();
    let mut iter = bytes.split(|&b| b == 0).filter(|s| !s.is_empty()).peekable();
    while let Some(status_bytes) = iter.next() {
        let status_str = std::str::from_utf8(status_bytes)
            .map_err(|e| anyhow!("non-utf8 status in name-status output: {}", e))?;
        let first = status_str
            .chars()
            .next()
            .ok_or_else(|| anyhow!("empty status field in name-status output"))?;
        let status = match first {
            'A' => NameStatus::Added,
            'D' => NameStatus::Deleted,
            'M' => NameStatus::Modified,
            'R' => NameStatus::Renamed,
            'C' => NameStatus::Copied,
            'T' => NameStatus::TypeChanged,
            _ => NameStatus::Other,
        };
        let needs_two_paths = matches!(status, NameStatus::Renamed | NameStatus::Copied);
        let path1 = iter
            .next()
            .ok_or_else(|| anyhow!("missing path after status `{}`", status_str))?;
        let path1 = path_from_bytes(path1)?;
        let (old_path, new_path) = if needs_two_paths {
            let path2 = iter
                .next()
                .ok_or_else(|| anyhow!("missing second path after status `{}`", status_str))?;
            let path2 = path_from_bytes(path2)?;
            (Some(path1), Some(path2))
        } else {
            match status {
                NameStatus::Added => (None, Some(path1)),
                NameStatus::Deleted => (Some(path1), None),
                _ => (Some(path1.clone()), Some(path1)),
            }
        };
        entries.push(NameStatusEntry {
            status,
            old_path,
            new_path,
        });
    }
    Ok(entries)
}

fn path_from_bytes(bytes: &[u8]) -> Result<String> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_owned())
        .map_err(|e| anyhow!("non-utf8 path in name-status output: {}", e))
}

async fn fetch_blob_text(repo: &Path, tree: &str, path: &str) -> Result<Option<String>> {
    let spec = format!("{}:{}", tree, path);
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show", "--end-of-options", &spec])
        .output()
        .await?;
    if !out.status.success() {
        return Ok(None);
    }
    if out.stdout.len() > MAX_BLOB_BYTES {
        return Ok(None);
    }
    match String::from_utf8(out.stdout) {
        Ok(s) => Ok(Some(s)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod name_status_tests {
    use super::*;

    #[test]
    fn parses_added_modified_deleted_entries() {
        let input = b"M\0modified.rs\0A\0added.rs\0D\0deleted.rs\0";
        let entries = parse_name_status_z(input).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].status, NameStatus::Modified);
        assert_eq!(entries[1].status, NameStatus::Added);
        assert_eq!(entries[2].status, NameStatus::Deleted);
    }

    #[test]
    fn parses_rename_and_copy_entries() {
        let input = b"R100\0old.rs\0new.rs\0C75\0src.rs\0copy.rs\0";
        let entries = parse_name_status_z(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].old_path.as_deref(), Some("old.rs"));
        assert_eq!(entries[0].new_path.as_deref(), Some("new.rs"));
    }

    #[test]
    fn empty_input_yields_no_entries() {
        assert!(parse_name_status_z(&[]).unwrap().is_empty());
    }
}

#[cfg(test)]
mod repo_name_tests {
    use super::repo_name_from_remote_url;

    #[test]
    fn parses_https_remote() {
        assert_eq!(
            repo_name_from_remote_url("https://github.com/psibase/eunomia.git"),
            "eunomia.git"
        );
    }

    #[test]
    fn parses_scp_style_remote() {
        assert_eq!(
            repo_name_from_remote_url("git@github.com:James-Mart/eunomia.git"),
            "eunomia.git"
        );
    }

    #[test]
    fn parses_ssh_url_remote() {
        assert_eq!(
            repo_name_from_remote_url("ssh://git@github.com/psibase/eunomia.git"),
            "eunomia.git"
        );
    }

    #[test]
    fn strips_trailing_slash() {
        assert_eq!(
            repo_name_from_remote_url("https://github.com/org/repo.git/"),
            "repo.git"
        );
    }
}

pub async fn rev_parse(repo: &Path, refname: &str) -> Result<String> {
    run(repo, &["rev-parse", "--verify", "--end-of-options", refname]).await
}

pub async fn commit_tree(
    repo: &Path,
    tree: &str,
    parents: &[&str],
    message: &str,
) -> Result<String> {
    let mut args: Vec<&str> = vec!["commit-tree", tree];
    for p in parents {
        args.push("-p");
        args.push(p);
    }
    args.push("-m");
    args.push(message);
    run(repo, &args).await
}

pub async fn worktree_add(repo: &Path, path: &Path, commit: &str) -> Result<()> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("worktree path is not valid UTF-8: {}", path.display()))?;
    run(repo, &["worktree", "add", "--detach", path_str, commit]).await?;
    Ok(())
}

pub async fn worktree_remove(repo: &Path, path: &Path) -> Result<()> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("worktree path is not valid UTF-8: {}", path.display()))?;
    run(repo, &["worktree", "remove", "--force", path_str]).await?;
    Ok(())
}

pub async fn branch_create(repo: &Path, name: &str, commit: &str, force: bool) -> Result<()> {
    let mut args: Vec<&str> = vec!["branch"];
    if force {
        args.push("-f");
    }
    args.push("--end-of-options");
    args.push(name);
    args.push(commit);
    run(repo, &args).await?;
    Ok(())
}

pub async fn run_in(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub async fn write_tree(cwd: &Path) -> Result<String> {
    run_in(cwd, &["write-tree"]).await
}

pub async fn rev_parse_in(cwd: &Path, refname: &str) -> Result<String> {
    run_in(cwd, &["rev-parse", refname]).await
}

pub async fn current_branch(repo: &Path) -> Result<Option<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .await?;
    if !out.status.success() {
        return Ok(None);
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(name))
    }
}

pub async fn origin_remote_url(repo: &Path) -> Result<Option<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "get-url", "origin"])
        .output()
        .await?;
    if !out.status.success() {
        return Ok(None);
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if url.is_empty() {
        Ok(None)
    } else {
        Ok(Some(url))
    }
}

/// Derive a short repo name from a git remote URL (HTTPS, SSH, or SCP-style).
pub fn repo_name_from_remote_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');

    // SCP-style: git@host:owner/repo
    if let Some((_, path)) = trimmed.rsplit_once('@') {
        if !trimmed.contains("://") {
            if let Some(name) = path.rsplit('/').next().filter(|s| !s.is_empty()) {
                return name.to_string();
            }
        }
    }

    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

pub async fn repo_name(repo: &Path) -> Result<String> {
    if let Some(url) = origin_remote_url(repo).await? {
        return Ok(repo_name_from_remote_url(&url));
    }
    Ok(repo
        .file_name()
        .and_then(|s| s.to_str())
        .map(String::from)
        .unwrap_or_else(|| repo.to_string_lossy().into_owned()))
}

pub async fn branch_exists(repo: &Path, name: &str) -> Result<bool> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{name}")])
        .output()
        .await?;
    Ok(out.status.success())
}

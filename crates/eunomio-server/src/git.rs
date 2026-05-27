// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use eunomio_core::FileBlob;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const MAX_BLOB_BYTES: usize = 512 * 1024;

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
    let base = resolve_ref_name(repo, base).await?;
    let source = resolve_ref_name(repo, source).await?;
    run(repo, &["merge-base", "--end-of-options", &base, &source]).await
}

/// Resolves `refname` in `repo`, mapping `origin/<branch>` to `<branch>` when needed (bare clones).
pub async fn resolve_ref_name(repo: &Path, refname: &str) -> Result<String> {
    if rev_parse(repo, refname).await.is_ok() {
        return Ok(refname.to_string());
    }
    if let Some(short) = refname.strip_prefix("origin/") {
        if !short.is_empty() && rev_parse(repo, short).await.is_ok() {
            return Ok(short.to_string());
        }
    }
    rev_parse(repo, refname).await?;
    Ok(refname.to_string())
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

pub async fn diff_binary(repo: &Path, from_tree: &str, to_tree: &str) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "diff",
            "--binary",
            "--full-index",
            "--no-color",
            "--no-ext-diff",
            "--end-of-options",
            from_tree,
            to_tree,
        ])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git diff --binary {} {}: {}",
            from_tree,
            to_tree,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

pub async fn apply_patch_bytes(cwd: &Path, patch: &[u8]) -> Result<()> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["apply", "--index", "--3way"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("git apply stdin unavailable"))?;
    stdin.write_all(patch).await?;
    drop(stdin);
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git apply --index --3way: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

pub async fn changed_files(repo: &Path, from_tree: &str, to_tree: &str) -> Result<Vec<FileBlob>> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeChange {
    pub status: TreeChangeStatus,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
}

pub async fn changed_entries(
    repo: &Path,
    from_tree: &str,
    to_tree: &str,
) -> Result<Vec<TreeChange>> {
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
    parse_name_status_z(&out.stdout)?
        .into_iter()
        .map(|entry| {
            let status = match entry.status {
                NameStatus::Added => TreeChangeStatus::Added,
                NameStatus::Deleted => TreeChangeStatus::Deleted,
                NameStatus::Modified => TreeChangeStatus::Modified,
                NameStatus::Renamed => TreeChangeStatus::Renamed,
                NameStatus::Copied => TreeChangeStatus::Copied,
                NameStatus::TypeChanged => TreeChangeStatus::TypeChanged,
                NameStatus::Other => return Err(anyhow!("unsupported tree change status")),
            };
            Ok(TreeChange {
                status,
                old_path: entry.old_path,
                new_path: entry.new_path,
            })
        })
        .collect()
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
    let mut iter = bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .peekable();
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

pub(crate) async fn fetch_blob_text(repo: &Path, tree: &str, path: &str) -> Result<Option<String>> {
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
    use std::path::Path;
    use std::process::Command;

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

    #[tokio::test]
    async fn changed_entries_reports_delete_and_rename() {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-q", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "test@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
        run_git(dir.path(), &["add", "."]);
        run_git(dir.path(), &["commit", "-q", "-m", "base"]);
        let base_tree = run_git(dir.path(), &["rev-parse", "HEAD^{tree}"]);
        run_git(dir.path(), &["mv", "a.txt", "c.txt"]);
        std::fs::remove_file(dir.path().join("b.txt")).unwrap();
        std::fs::write(dir.path().join("d.txt"), "d\n").unwrap();
        run_git(dir.path(), &["add", "-A"]);
        run_git(dir.path(), &["commit", "-q", "-m", "target"]);
        let target_tree = run_git(dir.path(), &["rev-parse", "HEAD^{tree}"]);

        let entries = changed_entries(dir.path(), &base_tree, &target_tree)
            .await
            .unwrap();
        assert!(entries.iter().any(|e| {
            e.status == TreeChangeStatus::Renamed
                && e.old_path.as_deref() == Some("a.txt")
                && e.new_path.as_deref() == Some("c.txt")
        }));
        assert!(entries.iter().any(|e| {
            e.status == TreeChangeStatus::Deleted && e.old_path.as_deref() == Some("b.txt")
        }));
        assert!(entries.iter().any(|e| {
            e.status == TreeChangeStatus::Added && e.new_path.as_deref() == Some("d.txt")
        }));
    }

    fn run_git(repo: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

#[cfg(test)]
mod repo_name_tests {
    use super::{repo_name_from_remote_url, repo_owner_from_remote_url};

    #[test]
    fn parses_https_remote() {
        assert_eq!(
            repo_name_from_remote_url("https://github.com/psibase/eunomio.git"),
            "eunomio.git"
        );
        assert_eq!(
            repo_owner_from_remote_url("https://github.com/psibase/eunomio.git"),
            Some("psibase".to_string())
        );
    }

    #[test]
    fn parses_scp_style_remote() {
        assert_eq!(
            repo_name_from_remote_url("git@github.com:James-Mart/eunomio.git"),
            "eunomio.git"
        );
        assert_eq!(
            repo_owner_from_remote_url("git@github.com:James-Mart/eunomio.git"),
            Some("James-Mart".to_string())
        );
    }

    #[test]
    fn parses_ssh_url_remote() {
        assert_eq!(
            repo_name_from_remote_url("ssh://git@github.com/psibase/eunomio.git"),
            "eunomio.git"
        );
        assert_eq!(
            repo_owner_from_remote_url("ssh://git@github.com/psibase/eunomio.git"),
            Some("psibase".to_string())
        );
    }

    #[test]
    fn strips_trailing_slash() {
        assert_eq!(
            repo_name_from_remote_url("https://github.com/org/repo.git/"),
            "repo.git"
        );
        assert_eq!(
            repo_owner_from_remote_url("https://github.com/org/repo.git/"),
            Some("org".to_string())
        );
    }

    #[test]
    fn normalizes_https_and_scp_to_same_network_key() {
        use super::normalize_network_url;
        assert_eq!(
            normalize_network_url("https://github.com/org/repo.git"),
            "github.com/org/repo"
        );
        assert_eq!(
            normalize_network_url("git@github.com:org/repo.git"),
            "github.com/org/repo"
        );
    }

    #[test]
    fn separates_local_and_remote_identities() {
        use super::normalize_remote_identity;
        assert_eq!(
            normalize_remote_identity("/tmp/myrepo", true),
            "local:/tmp/myrepo"
        );
        assert_eq!(
            normalize_remote_identity("https://github.com/org/repo.git", false),
            "remote:github.com/org/repo"
        );
    }
}

pub async fn rev_parse(repo: &Path, refname: &str) -> Result<String> {
    run(
        repo,
        &["rev-parse", "--verify", "--end-of-options", refname],
    )
    .await
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

pub async fn update_ref(repo: &Path, ref_name: &str, commit: &str) -> Result<()> {
    run(repo, &["update-ref", ref_name, commit])
        .await
        .map(|_| ())
}

pub async fn delete_ref(repo: &Path, ref_name: &str) -> Result<()> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["update-ref", "-d", ref_name])
        .output()
        .await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !stderr.contains("not found") {
            return Err(anyhow!("git update-ref -d {}: {}", ref_name, stderr.trim()));
        }
    }
    Ok(())
}

pub async fn commit_parents(repo: &Path, commit: &str) -> Result<Vec<String>> {
    let line = run(repo, &["rev-list", "--parents", "-n", "1", commit]).await?;
    let mut parts = line.split_whitespace();
    let _commit = parts
        .next()
        .ok_or_else(|| anyhow!("empty rev-list output for {}", commit))?;
    Ok(parts.map(str::to_string).collect())
}

pub async fn commits_between_linear(repo: &Path, parent: &str, head: &str) -> Result<Vec<String>> {
    let out = run(
        repo,
        &["rev-list", "--reverse", &format!("{parent}..{head}")],
    )
    .await?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

pub async fn commit_subject(repo: &Path, commit: &str) -> Result<String> {
    run(repo, &["show", "-s", "--format=%s", commit]).await
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
    remote_path_segments(url)
        .last()
        .cloned()
        .unwrap_or_else(|| url.trim().trim_end_matches('/').to_string())
}

/// Derive repo owner from a git remote URL when path is `owner/repo`.
pub fn repo_owner_from_remote_url(url: &str) -> Option<String> {
    let segments = remote_path_segments(url);
    if segments.len() >= 2 {
        Some(segments[segments.len() - 2].clone())
    } else {
        None
    }
}

fn remote_path_segments(url: &str) -> Vec<String> {
    let trimmed = url.trim().trim_end_matches('/');

    if let Some((_, after_at)) = trimmed.rsplit_once('@') {
        if !trimmed.contains("://") {
            let path = after_at
                .split_once(':')
                .map(|(_, repo_path)| repo_path)
                .unwrap_or(after_at);
            return path
                .split('/')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
        }
    }

    let after_scheme = trimmed.split("://").nth(1).unwrap_or(trimmed);
    let path = after_scheme.split_once('/').map(|(_, p)| p).unwrap_or("");
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
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

pub fn normalize_network_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("file://") {
        return rest.to_string();
    }
    if let Some((_, after_at)) = trimmed.rsplit_once('@') {
        if !trimmed.contains("://") {
            let (host, path) = after_at.split_once(':').unwrap_or((after_at, ""));
            return if path.is_empty() {
                host.to_string()
            } else {
                format!("{host}/{path}")
            };
        }
    }
    let after_scheme = trimmed.split("://").nth(1).unwrap_or(trimmed);
    after_scheme.to_string()
}

pub fn normalize_remote_identity(literal: &str, is_local: bool) -> String {
    if is_local {
        format!("local:{literal}")
    } else {
        format!("remote:{}", normalize_network_url(literal))
    }
}

pub fn is_local_repo_input(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("file://") {
        return true;
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ssh://")
        || trimmed.starts_with("git://")
    {
        return false;
    }
    if trimmed.contains('@') && !trimmed.starts_with('/') {
        return false;
    }
    let path = std::path::Path::new(trimmed);
    path.is_absolute() || trimmed.starts_with('.') || path.exists()
}

pub fn repo_display_parts(
    normalized_remote: &str,
    is_local: bool,
    literal_remote: &str,
) -> (Option<String>, String) {
    if is_local {
        let name = std::path::Path::new(literal_remote)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(literal_remote)
            .to_string();
        (None, name)
    } else {
        let url = normalized_remote
            .strip_prefix("remote:")
            .unwrap_or(normalized_remote);
        (
            repo_owner_from_remote_url(url),
            repo_name_from_remote_url(url),
        )
    }
}

pub async fn detect_trunk_ref(repo: &Path) -> Option<String> {
    let _ = run(repo, &["remote", "set-head", "origin", "--auto"]).await;
    let sym = run(
        repo,
        &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
    )
    .await
    .ok()?;
    sym.strip_prefix("refs/remotes/origin/")
        .map(str::to_string)
        .or_else(|| sym.strip_prefix("origin/").map(str::to_string))
}

pub async fn remote_set_url(repo: &Path, url: &str) -> Result<()> {
    run(repo, &["remote", "set-url", "origin", url])
        .await
        .map(|_| ())
}

pub async fn fetch_origin(repo: &Path) -> Result<()> {
    run(repo, &["fetch", "--prune", "origin"]).await.map(|_| ())
}

pub async fn clone_bare(url: &str, path: &Path) -> Result<()> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("clone path is not valid UTF-8: {}", path.display()))?;
    let out = Command::new("git")
        .args(["clone", "--bare", "--no-hardlinks", url, path_str])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "git clone --bare: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

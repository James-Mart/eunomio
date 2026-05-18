use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::process::Command;

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
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub async fn merge_base(repo: &Path, base: &str, source: &str) -> Result<String> {
    run(repo, &["merge-base", base, source]).await
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
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn rev_parse_tree(repo: &Path, refname: &str) -> Result<String> {
    run(repo, &["rev-parse", refname]).await
}

pub async fn rev_parse(repo: &Path, refname: &str) -> Result<String> {
    run(repo, &["rev-parse", refname]).await
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
    args.push(name);
    args.push(commit);
    run(repo, &args).await?;
    Ok(())
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

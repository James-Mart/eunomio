// SPDX-License-Identifier: Apache-2.0

use crate::{git, AppError, ShavingStep};
use anyhow::anyhow;
use std::path::Path;

pub async fn validate_track(
    repo: &Path,
    parent_tree_sha: &str,
    parent_commit_sha: &str,
    slice_tree_sha: &str,
    steps: &[ShavingStep],
) -> Result<(), AppError> {
    if steps.len() < 2 {
        return Err(AppError::Internal(anyhow!(
            "shaving track has fewer than two steps"
        )));
    }
    let last = steps
        .last()
        .ok_or_else(|| AppError::Internal(anyhow!("shaving track has no steps")))?;
    if last.tree_sha != slice_tree_sha {
        return Err(AppError::Internal(anyhow!(
            "shaving track head tree does not match slice tree"
        )));
    }
    let parent_commit_tree = git::rev_parse(repo, &format!("{parent_commit_sha}^{{tree}}"))
        .await
        .map_err(|e| AppError::Internal(anyhow!("shaving parent tree: {e}")))?;
    if parent_commit_tree != parent_tree_sha {
        return Err(AppError::Internal(anyhow!(
            "shaving parent tree does not match parent commit"
        )));
    }
    let mut expected_parent = parent_commit_sha.to_string();
    for step in steps {
        let parents = git::commit_parents(repo, &step.commit_sha)
            .await
            .map_err(|e| AppError::Internal(anyhow!("shaving commit parents: {e}")))?;
        if parents.len() != 1 || parents[0] != expected_parent {
            return Err(AppError::Internal(anyhow!(
                "shaving commit parent chain is broken"
            )));
        }
        expected_parent = step.commit_sha.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, process::Command};

    #[tokio::test]
    async fn accepts_valid_chain() {
        let repo = TestRepo::new();
        let parent = repo.commit("base", &[("a.txt", Some("a\n"))]);
        let step1 = repo.commit("step1", &[("b.txt", Some("b\n"))]);
        let step2 = repo.commit("step2", &[("c.txt", Some("c\n"))]);
        validate_track(
            repo.path(),
            &parent.tree,
            &parent.commit,
            &step2.tree,
            &[
                ShavingStep {
                    tree_sha: step1.tree,
                    commit_sha: step1.commit,
                },
                ShavingStep {
                    tree_sha: step2.tree.clone(),
                    commit_sha: step2.commit,
                },
            ],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn rejects_bad_head() {
        let repo = TestRepo::new();
        let parent = repo.commit("base", &[("a.txt", Some("a\n"))]);
        let step1 = repo.commit("step1", &[("b.txt", Some("b\n"))]);
        let step2 = repo.commit("step2", &[("c.txt", Some("c\n"))]);
        let wrong = repo.commit("wrong", &[("d.txt", Some("d\n"))]);
        let err = validate_track(
            repo.path(),
            &parent.tree,
            &parent.commit,
            &wrong.tree,
            &[
                ShavingStep {
                    tree_sha: step1.tree,
                    commit_sha: step1.commit,
                },
                ShavingStep {
                    tree_sha: step2.tree.clone(),
                    commit_sha: step2.commit,
                },
            ],
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("head tree"));
    }

    #[tokio::test]
    async fn rejects_broken_parent_chain() {
        let repo = TestRepo::new();
        let parent = repo.commit("base", &[("a.txt", Some("a\n"))]);
        let step1 = repo.commit("step1", &[("b.txt", Some("b\n"))]);
        run_git(repo.path(), &["checkout", "-q", &parent.commit]);
        let step2 = repo.commit("step2", &[("c.txt", Some("c\n"))]);
        let err = validate_track(
            repo.path(),
            &parent.tree,
            &parent.commit,
            &step2.tree,
            &[
                ShavingStep {
                    tree_sha: step1.tree,
                    commit_sha: step1.commit,
                },
                ShavingStep {
                    tree_sha: step2.tree.clone(),
                    commit_sha: step2.commit,
                },
            ],
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("parent chain"));
    }

    struct TestRepo {
        dir: tempfile::TempDir,
    }

    struct Commit {
        commit: String,
        tree: String,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            run_git(dir.path(), &["init", "-q", "-b", "main"]);
            run_git(dir.path(), &["config", "user.email", "test@example.com"]);
            run_git(dir.path(), &["config", "user.name", "Test"]);
            Self { dir }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn commit(&self, message: &str, changes: &[(&str, Option<&str>)]) -> Commit {
            for (rel, contents) in changes {
                let path = self.path().join(rel);
                if let Some(contents) = contents {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).unwrap();
                    }
                    std::fs::write(path, contents).unwrap();
                } else {
                    let _ = std::fs::remove_file(path);
                }
            }
            run_git(self.path(), &["add", "-A"]);
            run_git(self.path(), &["commit", "-q", "-m", message]);
            Commit {
                commit: run_git(self.path(), &["rev-parse", "HEAD"]),
                tree: run_git(self.path(), &["rev-parse", "HEAD^{tree}"]),
            }
        }
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

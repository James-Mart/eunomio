// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

pub const REPO_METADATA_FILE: &str = "eunomio-repo.json";

pub fn storage_slug(input: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(input.as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

pub fn org_slug(org_id: &str) -> String {
    storage_slug(org_id)
}

pub fn remote_slug(normalized_remote: &str) -> String {
    storage_slug(normalized_remote)
}

pub fn clone_path(data_dir: &Path, org_id: &str, normalized_remote: &str) -> PathBuf {
    data_dir
        .join("repos")
        .join(org_slug(org_id))
        .join(remote_slug(normalized_remote))
}

pub fn repo_metadata_path(clone_path: &Path) -> PathBuf {
    clone_path.join(REPO_METADATA_FILE)
}

pub fn worktrees_org_path(data_dir: &Path, org_id: &str) -> PathBuf {
    data_dir.join("worktrees").join(org_slug(org_id))
}

pub fn partition_worktree_path(
    data_dir: &Path,
    org_id: &str,
    session_id: &str,
    partition_id: &str,
) -> PathBuf {
    worktrees_org_path(data_dir, org_id)
        .join(session_id)
        .join(partition_id)
        .join("worktree")
}

pub fn generated_worktree_path(
    data_dir: &Path,
    org_id: &str,
    session_id: &str,
    group: &str,
    id: &str,
) -> PathBuf {
    worktrees_org_path(data_dir, org_id)
        .join(session_id)
        .join(group)
        .join(id)
        .join("worktree")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_paths_are_org_scoped_for_same_remote() {
        let data_dir = Path::new("/tmp/eunomio-data");
        let remote = "remote:github.com/acme/widgets";

        let org_a = clone_path(data_dir, "org-a", remote);
        let org_b = clone_path(data_dir, "org-b", remote);

        assert_ne!(org_a, org_b);
        assert_eq!(
            org_a.parent().unwrap().parent().unwrap(),
            data_dir.join("repos")
        );
        assert_eq!(
            org_b.parent().unwrap().parent().unwrap(),
            data_dir.join("repos")
        );
    }
}

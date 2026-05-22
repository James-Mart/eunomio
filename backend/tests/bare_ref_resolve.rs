mod common;

use common::git;
use std::path::Path;

fn init_repo_with_branches(path: &Path) {
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "test@example.com"]);
    git(path, &["config", "user.name", "Test"]);
    common::write(path, "base.txt", "base\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "base"]);

    git(path, &["checkout", "-q", "-b", "feature"]);
    common::write(path, "feature.txt", "feature\n");
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "feature"]);

    git(path, &["checkout", "-q", "main"]);
}

#[tokio::test]
async fn bare_clone_resolves_origin_prefixed_refs() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let bare = tmp.path().join("bare");
    std::fs::create_dir_all(&source).unwrap();
    init_repo_with_branches(&source);

    git(
        &source,
        &[
            "clone",
            "--bare",
            "-q",
            &source.to_string_lossy(),
            &bare.to_string_lossy(),
        ],
    );

    let mb = eunomio::git::merge_base(&bare, "origin/main", "origin/feature")
        .await
        .expect("merge-base with origin/ refs");
    assert_eq!(mb.len(), 40);

    let resolved = eunomio::git::resolve_ref_name(&bare, "origin/feature")
        .await
        .expect("resolve origin/feature");
    assert_eq!(resolved, "feature");
}

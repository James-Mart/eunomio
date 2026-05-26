// SPDX-License-Identifier: Apache-2.0

use crate::{
    cursor_bridge::{HelperEvent, RunRequest},
    git, partition_settings, repo_store,
    state::AppState,
    subagents::{self, reorder::ReorderOutput},
    worktree, AppError,
};
use anyhow::{anyhow, Result as AnyResult};
use eunomio_core::types::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{shaver::TimelineInput, Coordinator};

#[derive(Debug, Clone)]
struct ChainNode {
    node_id: String,
    parent_node_id: Option<String>,
    tree_sha: String,
    commit_sha: String,
    title: String,
    description: String,
    strategy: Option<PartitionStrategy>,
}

#[derive(Debug, Clone)]
struct ReplayNode {
    node_id: String,
    tree_sha: String,
    commit_sha: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReorderPromptNode {
    node_id: String,
    parent_node_id: Option<String>,
    tree_sha: String,
    commit_sha: String,
    title: String,
    description: String,
    strategy: Option<PartitionStrategy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReorderPromptChain {
    nodes: Vec<ReorderPromptNode>,
}

impl Coordinator {
    pub(super) async fn maybe_finalize_session_partition_pass(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
    ) -> Result<(), AppError> {
        let remaining = state
            .datastore
            .partitions()
            .list(org_id, session_id, None)
            .await?;
        if !remaining.is_empty() {
            return Ok(());
        }
        if !state
            .datastore
            .sessions()
            .try_begin_session_finalization(org_id, session_id)
            .await?
        {
            return Ok(());
        }

        let finalize = self
            .finalize_session_partition_pass(state, org_id, session_id)
            .await;
        if finalize.is_err() {
            let _ = state
                .datastore
                .sessions()
                .finish_session_finalization(org_id, session_id)
                .await;
        }
        finalize
    }

    pub(super) async fn recover_session_partition_finalization(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
    ) -> Result<(), AppError> {
        state
            .datastore
            .sessions()
            .finish_session_finalization(org_id, session_id)
            .await?;
        self.maybe_finalize_session_partition_pass(state, org_id, session_id)
            .await
    }

    async fn finalize_session_partition_pass(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
    ) -> Result<(), AppError> {
        let settings = partition_settings::load_for_partition(state, org_id, session_id).await?;
        let chain = load_chain(state, org_id, session_id).await?;
        let original_order = non_base_ids(&chain);
        let audit = if !settings.coordinator.reorder_enabled {
            ReorderAudit {
                status: ReorderAuditStatus::Disabled,
                original_order: original_order.clone(),
                proposed_order: original_order.clone(),
                applied_order: original_order.clone(),
                hard_deps: vec![],
                soft_prefs: vec![],
                uncertain_pairs: vec![],
                rationale: "Reorder disabled in settings.".into(),
                fallback_reason: None,
                created_at: eunomio_core::unix_seconds(),
            }
        } else {
            match self
                .attempt_reorder(state, org_id, session_id, &settings, &chain)
                .await
            {
                Ok(audit) => audit,
                Err(e) => ReorderAudit {
                    status: ReorderAuditStatus::Fallback,
                    original_order: original_order.clone(),
                    proposed_order: original_order.clone(),
                    applied_order: original_order.clone(),
                    hard_deps: vec![],
                    soft_prefs: vec![],
                    uncertain_pairs: vec![],
                    rationale: "Reorder failed; kept original order.".into(),
                    fallback_reason: Some(e.to_string()),
                    created_at: eunomio_core::unix_seconds(),
                },
            }
        };
        state
            .datastore
            .sessions()
            .set_reorder_audit(org_id, session_id, Some(audit))
            .await?;

        let completed_at = eunomio_core::unix_seconds();
        let marked = state
            .datastore
            .sessions()
            .mark_session_partition_complete(org_id, session_id, completed_at)
            .await?;
        if marked {
            self.emit(
                session_id,
                SseEvent::SessionPartitionComplete {
                    session_id: session_id.to_string(),
                    completed_at,
                },
            );
        }
        self.spawn_missing_timelines_for_session(state, org_id, session_id)
            .await?;
        Ok(())
    }

    async fn attempt_reorder(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        settings: &PartitionSettings,
        chain: &[ChainNode],
    ) -> AnyResult<ReorderAudit> {
        let original_order = non_base_ids(chain);
        if original_order.len() <= 1 {
            return Ok(ReorderAudit {
                status: ReorderAuditStatus::NoChange,
                original_order: original_order.clone(),
                proposed_order: original_order.clone(),
                applied_order: original_order,
                hard_deps: vec![],
                soft_prefs: vec![],
                uncertain_pairs: vec![],
                rationale: "Only one review atom exists.".into(),
                fallback_reason: None,
                created_at: eunomio_core::unix_seconds(),
            });
        }

        let output = self
            .run_reorder_agent(state, org_id, session_id, settings, chain)
            .await?;
        validate_reorder_output(&original_order, &output)?;
        let replayed =
            replay_order(state, org_id, session_id, chain, &output.proposed_order).await?;
        let applied_order: Vec<String> = replayed.iter().map(|n| n.node_id.clone()).collect();
        let status = if applied_order == original_order {
            ReorderAuditStatus::NoChange
        } else {
            let rewrites = replayed
                .iter()
                .map(|n| NodeRewrite {
                    node_id: n.node_id.clone(),
                    parent_node_id: chain_parent_id(&replayed, chain, &n.node_id),
                    tree_sha: n.tree_sha.clone(),
                    commit_sha: n.commit_sha.clone(),
                })
                .collect();
            state
                .datastore
                .nodes()
                .rewrite_chain(org_id, session_id, rewrites)
                .await?;
            ReorderAuditStatus::Applied
        };
        Ok(ReorderAudit {
            status,
            original_order,
            proposed_order: output.proposed_order,
            applied_order,
            hard_deps: output.hard_deps,
            soft_prefs: output.soft_prefs,
            uncertain_pairs: output.uncertain_pairs,
            rationale: output.rationale,
            fallback_reason: None,
            created_at: eunomio_core::unix_seconds(),
        })
    }

    async fn run_reorder_agent(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
        settings: &PartitionSettings,
        chain: &[ChainNode],
    ) -> AnyResult<ReorderOutput> {
        self.inner.quota.check_can_start_run(org_id).await?;
        let user_id = state
            .datastore
            .sessions()
            .user_id(org_id, session_id)
            .await?;
        let cursor_api_key = state
            .keystore
            .get(&user_id)
            .await
            .map_err(|e| AppError::Internal(anyhow!("reading cursor api key: {e}")))?
            .ok_or_else(|| AppError::Unrecoverable {
                status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
                code: "cursor_sdk_unavailable".into(),
                message: "Cursor API key not configured".into(),
            })?;
        let git_root = repo_store::session_git_root(state, org_id, session_id).await?;
        let base = chain
            .first()
            .ok_or_else(|| anyhow!("session chain is empty"))?;
        let final_node = chain
            .last()
            .ok_or_else(|| anyhow!("session chain is empty"))?;
        let worktree_id = format!("reorder-{}", Uuid::new_v4());
        let worktree_path = worktree::provision(
            &git_root,
            &state.data_dir,
            session_id,
            &worktree_id,
            &base.commit_sha,
        )
        .await
        .map_err(|e| anyhow!("reorder worktree: {e}"))?;
        let result = self
            .drive_reorder_agent(
                org_id,
                &worktree_path,
                settings,
                cursor_api_key,
                base,
                final_node,
                chain,
            )
            .await;
        worktree::teardown(&git_root, &worktree_path).await;
        result
    }

    async fn drive_reorder_agent(
        &self,
        org_id: &str,
        worktree_path: &Path,
        settings: &PartitionSettings,
        cursor_api_key: String,
        base: &ChainNode,
        final_node: &ChainNode,
        chain: &[ChainNode],
    ) -> AnyResult<ReorderOutput> {
        let chain_json = serde_json::to_string_pretty(&ReorderPromptChain {
            nodes: chain
                .iter()
                .map(|n| ReorderPromptNode {
                    node_id: n.node_id.clone(),
                    parent_node_id: n.parent_node_id.clone(),
                    tree_sha: n.tree_sha.clone(),
                    commit_sha: n.commit_sha.clone(),
                    title: n.title.clone(),
                    description: n.description.clone(),
                    strategy: n.strategy,
                })
                .collect(),
        })?;
        let prompt = subagents::reorder::render_prompt(
            &subagents::reorder::ReorderContext {
                base_commit: base.commit_sha.clone(),
                final_commit: final_node.commit_sha.clone(),
                base_tree: base.tree_sha.clone(),
                final_tree: final_node.tree_sha.clone(),
                chain_json,
            },
            &self.inner.subagents.reorder.template,
        );
        let model = if settings.reorder.override_model {
            settings.reorder.model.clone()
        } else {
            settings.coordinator.model.clone()
        };
        let (tx, mut rx) = mpsc::channel::<HelperEvent>(64);
        let handle = self
            .inner
            .runner
            .run(
                RunRequest {
                    model,
                    cwd: worktree_path.to_path_buf(),
                    prompt,
                    run_id: Uuid::new_v4().to_string(),
                    cursor_api_key: Some(cursor_api_key),
                    env: Default::default(),
                },
                tx,
            )
            .await?;
        let mut final_result = None;
        let mut error = None;
        let mut cancelled = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                HelperEvent::UsageReported { usage, .. } => {
                    if let Err(e) = self.inner.quota.record_usage(org_id, usage).await {
                        tracing::warn!(error = %e, "quota record_usage failed");
                    }
                }
                HelperEvent::Finished { result, .. } => final_result = Some(result),
                HelperEvent::Error { message, .. } => error = Some(message),
                HelperEvent::Cancelled { .. } => cancelled = true,
                HelperEvent::Started { .. } | HelperEvent::SdkMessage { .. } => {}
            }
        }
        drop(handle);
        if cancelled {
            return Err(anyhow!("reorder agent cancelled"));
        }
        if let Some(message) = error {
            return Err(anyhow!(message));
        }
        let raw = final_result.ok_or_else(|| anyhow!("no terminal event from reorder agent"))?;
        subagents::reorder::parse_output(&raw).map_err(|e| anyhow!("parsing reorder output: {e}"))
    }

    pub(super) async fn spawn_missing_timelines_for_session(
        &self,
        state: &AppState,
        org_id: &str,
        session_id: &str,
    ) -> Result<(), AppError> {
        let settings = partition_settings::load_for_partition(state, org_id, session_id).await?;
        if !settings.coordinator.timeline_enabled {
            return Ok(());
        }
        let user_id = state
            .datastore
            .sessions()
            .user_id(org_id, session_id)
            .await?;
        let chain = load_chain(state, org_id, session_id).await?;
        for node in chain.iter().filter(|n| n.parent_node_id.is_some()) {
            if state
                .datastore
                .shaving_tracks()
                .get(org_id, session_id, &node.node_id)
                .await?
                .is_some()
            {
                continue;
            }
            let Some(parent_id) = &node.parent_node_id else {
                continue;
            };
            let Some(parent) = chain.iter().find(|n| &n.node_id == parent_id) else {
                continue;
            };
            self.maybe_spawn_timeline_generation(
                state.clone(),
                TimelineInput {
                    org_id: org_id.to_string(),
                    user_id: user_id.clone(),
                    session_id: session_id.to_string(),
                    target_node_id: node.node_id.clone(),
                    target_title: node.title.clone(),
                    target_description: node.description.clone(),
                    parent_tree_sha: parent.tree_sha.clone(),
                    parent_commit_sha: parent.commit_sha.clone(),
                    target_tree_sha: node.tree_sha.clone(),
                },
            );
        }
        Ok(())
    }
}

async fn load_chain(
    state: &AppState,
    org_id: &str,
    session_id: &str,
) -> Result<Vec<ChainNode>, AppError> {
    let nodes = state
        .datastore
        .nodes()
        .list_for_session(org_id, session_id)
        .await?;
    let mut by_parent: HashMap<Option<String>, Vec<GraphNode>> = HashMap::new();
    for node in nodes {
        by_parent
            .entry(node.parent_node_id.clone())
            .or_default()
            .push(node);
    }
    let roots = by_parent.remove(&None).unwrap_or_default();
    if roots.len() != 1 {
        return Err(AppError::Internal(anyhow!(
            "expected exactly one base node, got {}",
            roots.len()
        )));
    }
    let mut out = Vec::new();
    let mut current = roots.into_iter().next().unwrap();
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(current.node_id.clone()) {
            return Err(AppError::Internal(anyhow!("cycle in session chain")));
        }
        let node_id = current.node_id.clone();
        out.push(ChainNode {
            node_id: current.node_id,
            parent_node_id: current.parent_node_id,
            tree_sha: current.tree_sha,
            commit_sha: current.commit_sha,
            title: current.title,
            description: current.description,
            strategy: current.strategy,
        });
        let children = by_parent.remove(&Some(node_id)).unwrap_or_default();
        if children.is_empty() {
            break;
        }
        if children.len() != 1 {
            return Err(AppError::Internal(anyhow!(
                "expected linear chain child count 1, got {}",
                children.len()
            )));
        }
        current = children.into_iter().next().unwrap();
    }
    let leftover: usize = by_parent.values().map(Vec::len).sum();
    if leftover != 0 {
        return Err(AppError::Internal(anyhow!(
            "session chain has {leftover} unreachable nodes"
        )));
    }
    Ok(out)
}

fn non_base_ids(chain: &[ChainNode]) -> Vec<String> {
    chain
        .iter()
        .filter(|n| n.parent_node_id.is_some())
        .map(|n| n.node_id.clone())
        .collect()
}

fn validate_reorder_output(original_order: &[String], output: &ReorderOutput) -> AnyResult<()> {
    let expected: HashSet<_> = original_order.iter().cloned().collect();
    let proposed: HashSet<_> = output.proposed_order.iter().cloned().collect();
    if expected != proposed || output.proposed_order.len() != original_order.len() {
        return Err(anyhow!(
            "proposed order does not contain each non-base node exactly once"
        ));
    }
    let pos: HashMap<_, _> = output
        .proposed_order
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.as_str(), idx))
        .collect();
    for dep in &output.hard_deps {
        let Some(before) = pos.get(dep.before.as_str()) else {
            return Err(anyhow!("hard dep references unknown node {}", dep.before));
        };
        let Some(after) = pos.get(dep.after.as_str()) else {
            return Err(anyhow!("hard dep references unknown node {}", dep.after));
        };
        if before >= after {
            return Err(anyhow!(
                "proposed order violates hard dep {} before {}",
                dep.before,
                dep.after
            ));
        }
    }
    Ok(())
}

async fn replay_order(
    state: &AppState,
    org_id: &str,
    session_id: &str,
    chain: &[ChainNode],
    order: &[String],
) -> AnyResult<Vec<ReplayNode>> {
    let git_root = repo_store::session_git_root(state, org_id, session_id).await?;
    let base = chain.first().ok_or_else(|| anyhow!("empty chain"))?;
    let final_tree = state
        .datastore
        .sessions()
        .final_tree(org_id, session_id)
        .await?
        .ok_or_else(|| anyhow!("session final tree missing"))?;
    let worktree_id = format!("replay-{}", Uuid::new_v4());
    let worktree_path = worktree::provision(
        &git_root,
        &state.data_dir,
        session_id,
        &worktree_id,
        &base.commit_sha,
    )
    .await
    .map_err(|e| anyhow!("replay worktree: {e}"))?;
    let replay =
        replay_order_in_worktree(&git_root, &worktree_path, chain, order, &final_tree).await;
    worktree::teardown(&git_root, &worktree_path).await;
    replay
}

async fn replay_order_in_worktree(
    git_root: &Path,
    worktree_path: &Path,
    chain: &[ChainNode],
    order: &[String],
    final_tree: &str,
) -> AnyResult<Vec<ReplayNode>> {
    let by_id: HashMap<_, _> = chain.iter().map(|n| (n.node_id.as_str(), n)).collect();
    let mut replayed = Vec::with_capacity(order.len());
    let mut parent_commit = chain
        .first()
        .ok_or_else(|| anyhow!("empty chain"))?
        .commit_sha
        .clone();
    for node_id in order {
        let node = by_id
            .get(node_id.as_str())
            .ok_or_else(|| anyhow!("unknown node {node_id}"))?;
        let parent_id = node
            .parent_node_id
            .as_deref()
            .ok_or_else(|| anyhow!("base node cannot be replayed"))?;
        let old_parent = by_id
            .get(parent_id)
            .ok_or_else(|| anyhow!("missing parent {parent_id}"))?;
        let patch = git::diff_binary(git_root, &old_parent.tree_sha, &node.tree_sha).await?;
        if !patch.is_empty() {
            git::apply_patch_bytes(worktree_path, &patch).await?;
        }
        let tree_sha = git::write_tree(worktree_path).await?;
        let commit_sha =
            git::commit_tree(git_root, &tree_sha, &[&parent_commit], &node.title).await?;
        parent_commit = commit_sha.clone();
        replayed.push(ReplayNode {
            node_id: node.node_id.clone(),
            tree_sha,
            commit_sha,
        });
    }
    let actual = replayed
        .last()
        .map(|n| n.tree_sha.as_str())
        .ok_or_else(|| anyhow!("replay produced no nodes"))?;
    if actual != final_tree {
        return Err(anyhow!(
            "replayed final tree {actual} != expected {final_tree}"
        ));
    }
    Ok(replayed)
}

fn chain_parent_id(replayed: &[ReplayNode], chain: &[ChainNode], node_id: &str) -> String {
    let idx = replayed
        .iter()
        .position(|n| n.node_id == node_id)
        .expect("replayed node exists");
    if idx == 0 {
        chain.first().expect("base exists").node_id.clone()
    } else {
        replayed[idx - 1].node_id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, process::Command};

    #[tokio::test]
    async fn patch_replay_accepts_independent_reorder() {
        let repo = TestRepo::new();
        let base = repo.commit("base", &[("base.txt", Some("base\n"))], None);
        let a = repo.commit("a", &[("a.txt", Some("a\n"))], Some(&base.commit));
        let b = repo.commit("b", &[("b.txt", Some("b\n"))], Some(&a.commit));
        let worktree = repo.dir.path().join("replay");
        run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                "--detach",
                path_str(&worktree),
                &base.commit,
            ],
        );

        let chain = vec![
            node("base", None, &base.tree, &base.commit, "base"),
            node("a", Some("base"), &a.tree, &a.commit, "Add a"),
            node("b", Some("a"), &b.tree, &b.commit, "Add b"),
        ];
        let replayed = replay_order_in_worktree(
            repo.path(),
            &worktree,
            &chain,
            &["b".to_string(), "a".to_string()],
            &b.tree,
        )
        .await
        .unwrap();
        assert_eq!(
            replayed
                .iter()
                .map(|n| n.node_id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
    }

    #[test]
    fn hard_deps_must_match_proposed_order() {
        let err = validate_reorder_output(
            &["a".into(), "b".into()],
            &ReorderOutput {
                proposed_order: vec!["b".into(), "a".into()],
                hard_deps: vec![ReorderRelation {
                    before: "a".into(),
                    after: "b".into(),
                    reason: "a before b".into(),
                }],
                soft_prefs: vec![],
                uncertain_pairs: vec![],
                rationale: "x".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("violates hard dep"));
    }

    fn node(
        node_id: &str,
        parent_node_id: Option<&str>,
        tree_sha: &str,
        commit_sha: &str,
        title: &str,
    ) -> ChainNode {
        ChainNode {
            node_id: node_id.into(),
            parent_node_id: parent_node_id.map(str::to_string),
            tree_sha: tree_sha.into(),
            commit_sha: commit_sha.into(),
            title: title.into(),
            description: String::new(),
            strategy: None,
        }
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

        fn commit(
            &self,
            message: &str,
            changes: &[(&str, Option<&str>)],
            checkout: Option<&str>,
        ) -> Commit {
            if let Some(commit) = checkout {
                run_git(self.path(), &["checkout", "-q", commit]);
            }
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

    fn path_str(path: &Path) -> &str {
        path.to_str().unwrap()
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

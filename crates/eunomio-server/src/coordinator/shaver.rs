// SPDX-License-Identifier: Apache-2.0

use crate::{
    cursor_bridge::{HelperEvent, RunRequest},
    git, partition_settings, repo_store,
    shavings::validate,
    state::AppState,
    subagents, worktree, AppError, NewShaverRunInsert, NewShavingTrackInsert, ShavingStep,
};
use anyhow::{anyhow, Context, Result};
use eunomio_core::types::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::Coordinator;

pub(super) struct TimelineInput {
    pub org_id: String,
    pub user_id: String,
    pub session_id: String,
    pub target_node_id: String,
    pub target_title: String,
    pub target_description: String,
    pub parent_tree_sha: String,
    pub parent_commit_sha: String,
    pub target_tree_sha: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShaverResult {
    head_commit: String,
}

struct GeneratedTimeline {
    run_id: String,
    raw_result: String,
    head_commit: String,
    steps: Vec<ShavingStep>,
}

impl Coordinator {
    pub(super) fn maybe_spawn_timeline_generation(&self, state: AppState, input: TimelineInput) {
        let coord = self.clone();
        tokio::spawn(async move {
            if let Err(e) = coord.run_timeline_generation(&state, input).await {
                tracing::warn!(error = %e, "timeline generation skipped");
            }
        });
    }

    async fn run_timeline_generation(
        &self,
        state: &AppState,
        input: TimelineInput,
    ) -> Result<(), AppError> {
        let settings = partition_settings::load_for_user(&state.data_dir, &input.user_id).await?;
        if !settings.coordinator.timeline_enabled {
            return Ok(());
        }
        if state
            .datastore
            .shaving_tracks()
            .get(&input.org_id, &input.session_id, &input.target_node_id)
            .await?
            .is_some()
        {
            return Ok(());
        }

        let git_root =
            repo_store::session_git_root(state, &input.org_id, &input.session_id).await?;
        let worktree_path = provision_shaver_worktree(
            &git_root,
            &state.data_dir,
            &input.session_id,
            &input.parent_commit_sha,
        )
        .await
        .map_err(|e| AppError::Internal(anyhow!("timeline worktree: {e}")))?;
        let generated = match self
            .run_shaver_in_worktree(state, &settings, &git_root, &worktree_path, &input)
            .await
        {
            Ok(generated) => generated,
            Err(e) => {
                cleanup_shaver_worktree(&git_root, &worktree_path).await;
                return Err(e);
            }
        };
        let ref_name = format!("refs/eunomio/shavings/{}", input.target_node_id);
        if let Err(e) = git::update_ref(&git_root, &ref_name, &generated.head_commit).await {
            cleanup_shaver_worktree(&git_root, &worktree_path).await;
            let _ = state
                .datastore
                .shaver_runs()
                .finish_error(&input.org_id, &generated.run_id, format!("{e}"))
                .await;
            return Err(AppError::Internal(anyhow!("timeline update-ref: {e}")));
        }
        if let Err(e) = teardown_shaver_worktree(&git_root, &worktree_path).await {
            if let Err(cleanup_err) = git::delete_ref(&git_root, &ref_name).await {
                tracing::warn!(error = %cleanup_err, ref_name = %ref_name, "timeline ref cleanup failed");
            }
            let _ = state
                .datastore
                .shaver_runs()
                .finish_error(&input.org_id, &generated.run_id, format!("{e}"))
                .await;
            return Err(AppError::Internal(anyhow!(
                "timeline worktree cleanup: {e}"
            )));
        }
        if let Err(e) = publish_track(state, &git_root, &input, generated, ref_name).await {
            return Err(e);
        }
        Ok(())
    }

    async fn run_shaver_in_worktree(
        &self,
        state: &AppState,
        settings: &PartitionSettings,
        git_root: &Path,
        worktree_path: &Path,
        input: &TimelineInput,
    ) -> Result<GeneratedTimeline, AppError> {
        let template = &self.inner.subagents.shaver.template;
        let ctx = subagents::shaver::ShaverContext {
            worktree_path: worktree_path.display().to_string(),
            parent_commit: input.parent_commit_sha.clone(),
            before_tree: input.parent_tree_sha.clone(),
            target_tree: input.target_tree_sha.clone(),
            target_title: input.target_title.clone(),
            target_description: input.target_description.clone(),
        };
        let prompt = subagents::shaver::render_prompt(&ctx, template);
        let run_id = state
            .datastore
            .shaver_runs()
            .start(NewShaverRunInsert {
                org_id: input.org_id.clone(),
                user_id: input.user_id.clone(),
                session_id: input.session_id.clone(),
                target_node_id: input.target_node_id.clone(),
                worktree_path: worktree_path.display().to_string(),
                prompt_text: prompt.clone(),
                started_at: eunomio_core::unix_seconds(),
            })
            .await?;

        let result = self
            .drive_shaver_run(
                state,
                settings,
                git_root,
                worktree_path,
                input,
                &run_id,
                prompt,
            )
            .await;
        let generated = match result {
            Ok(generated) => generated,
            Err(e) => {
                let _ = state
                    .datastore
                    .shaver_runs()
                    .finish_error(&input.org_id, &run_id, e.to_string())
                    .await;
                return Err(e);
            }
        };
        Ok(generated)
    }

    async fn drive_shaver_run(
        &self,
        state: &AppState,
        settings: &PartitionSettings,
        git_root: &Path,
        worktree_path: &Path,
        input: &TimelineInput,
        run_id: &str,
        prompt: String,
    ) -> Result<GeneratedTimeline, AppError> {
        self.inner.quota.check_can_start_run(&input.org_id).await?;
        let cursor_api_key = state
            .keystore
            .get(&input.user_id)
            .await
            .map_err(|e| AppError::Internal(anyhow!("reading cursor api key: {e}")))?
            .ok_or_else(|| AppError::Unrecoverable {
                status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
                code: "cursor_sdk_unavailable".into(),
                message: "Cursor API key not configured".into(),
            })?;
        let model = if settings.shaver.override_model {
            settings.shaver.model.clone()
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
                    run_id: run_id.to_string(),
                    cursor_api_key: Some(cursor_api_key),
                    env: timeline_git_env(),
                },
                tx,
            )
            .await?;

        let mut final_result: Option<String> = None;
        let mut error: Option<String> = None;
        let mut cancelled = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                HelperEvent::Started { .. } => {}
                HelperEvent::SdkMessage { message, .. } => {
                    if let Some(chunk) = crate::cursor_bridge::fold_sdk_event(&message) {
                        if let Err(e) = state
                            .datastore
                            .shaver_runs()
                            .append_transcript_text(&input.org_id, run_id, &chunk)
                            .await
                        {
                            tracing::warn!(error = %e, run_id = %run_id, "persisting shaver transcript_text failed");
                        }
                    }
                }
                HelperEvent::UsageReported { usage, .. } => {
                    if let Err(e) = self.inner.quota.record_usage(&input.org_id, usage).await {
                        tracing::warn!(org_id = %input.org_id, error = %e, "quota record_usage failed");
                    }
                }
                HelperEvent::Finished { result, .. } => final_result = Some(result),
                HelperEvent::Error { message, .. } => error = Some(message),
                HelperEvent::Cancelled { .. } => cancelled = true,
            }
        }
        drop(handle);
        if cancelled {
            return Err(AppError::Internal(anyhow!("shaver cancelled")));
        }
        if let Some(message) = error {
            return Err(AppError::Internal(anyhow!(message)));
        }
        let raw = final_result
            .ok_or_else(|| AppError::Internal(anyhow!("no terminal event from shaver")))?;
        let parsed = subagents::shaver::parse_output(&raw)
            .map_err(|e| AppError::Internal(anyhow!("parsing shaver output: {e}")))?;
        let steps = derive_steps(git_root, &input.parent_commit_sha, &parsed.head_commit).await?;
        validate::validate_track(
            git_root,
            &input.parent_tree_sha,
            &input.parent_commit_sha,
            &input.target_tree_sha,
            &steps,
        )
        .await?;
        Ok(GeneratedTimeline {
            run_id: run_id.to_string(),
            raw_result: raw,
            head_commit: parsed.head_commit,
            steps,
        })
    }
}

fn timeline_git_env() -> BTreeMap<String, String> {
    [
        ("GIT_AUTHOR_NAME", "Eunomio"),
        ("GIT_AUTHOR_EMAIL", "timeline@eunomio.local"),
        ("GIT_COMMITTER_NAME", "Eunomio"),
        ("GIT_COMMITTER_EMAIL", "timeline@eunomio.local"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::timeline_git_env;

    #[test]
    fn timeline_git_env_supplies_commit_identity() {
        let env = timeline_git_env();

        assert_eq!(
            env.get("GIT_AUTHOR_NAME").map(String::as_str),
            Some("Eunomio")
        );
        assert_eq!(
            env.get("GIT_AUTHOR_EMAIL").map(String::as_str),
            Some("timeline@eunomio.local")
        );
        assert_eq!(
            env.get("GIT_COMMITTER_NAME").map(String::as_str),
            Some("Eunomio")
        );
        assert_eq!(
            env.get("GIT_COMMITTER_EMAIL").map(String::as_str),
            Some("timeline@eunomio.local")
        );
    }
}

async fn derive_steps(
    repo: &Path,
    parent_commit: &str,
    head_commit: &str,
) -> Result<Vec<ShavingStep>, AppError> {
    let commits = git::commits_between_linear(repo, parent_commit, head_commit)
        .await
        .map_err(|e| AppError::Internal(anyhow!("timeline ancestry: {e}")))?;
    let mut steps = Vec::with_capacity(commits.len());
    for commit in commits {
        let tree_sha = git::rev_parse(repo, &format!("{commit}^{{tree}}"))
            .await
            .map_err(|e| AppError::Internal(anyhow!("timeline commit tree: {e}")))?;
        let subject = git::commit_subject(repo, &commit)
            .await
            .map_err(|e| AppError::Internal(anyhow!("timeline commit subject: {e}")))?;
        let label = subject
            .lines()
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        steps.push(ShavingStep {
            tree_sha,
            commit_sha: commit,
            label,
        });
    }
    Ok(steps)
}

async fn publish_track(
    state: &AppState,
    git_root: &Path,
    input: &TimelineInput,
    generated: GeneratedTimeline,
    ref_name: String,
) -> Result<(), AppError> {
    let run_id = generated.run_id;
    let raw_result = generated.raw_result;
    let head_commit = generated.head_commit;
    let current = state
        .datastore
        .nodes()
        .target_tree_and_parent(&input.org_id, &input.session_id, &input.target_node_id)
        .await?;
    if current.as_ref().map(|(target_tree, _, parent_tree)| {
        target_tree == &input.target_tree_sha
            && parent_tree.as_deref() == Some(input.parent_tree_sha.as_str())
    }) != Some(true)
    {
        let _ = state
            .datastore
            .shaver_runs()
            .finish_error(
                &input.org_id,
                &run_id,
                "edge changed before timeline publish".into(),
            )
            .await;
        if let Err(cleanup_err) = git::delete_ref(git_root, &ref_name).await {
            tracing::warn!(error = %cleanup_err, ref_name = %ref_name, "timeline ref cleanup failed");
        }
        return Ok(());
    }
    let row = NewShavingTrackInsert {
        org_id: input.org_id.clone(),
        session_id: input.session_id.clone(),
        target_node_id: input.target_node_id.clone(),
        parent_tree_sha: input.parent_tree_sha.clone(),
        head_tree_sha: input.target_tree_sha.clone(),
        steps: generated.steps,
        ref_name: ref_name.clone(),
        created_at: eunomio_core::unix_seconds(),
    };
    if let Err(e) = state.datastore.shaving_tracks().insert(row).await {
        let _ = state
            .datastore
            .shaver_runs()
            .finish_error(&input.org_id, &run_id, e.to_string())
            .await;
        if let Err(cleanup_err) = git::delete_ref(git_root, &ref_name).await {
            tracing::warn!(error = %cleanup_err, ref_name = %ref_name, "timeline ref cleanup failed");
        }
        return Err(e);
    }
    let result_json = serde_json::to_string(&ShaverResult { head_commit })
        .map_err(|e| AppError::Internal(anyhow!("shaver result json: {e}")))?;
    if let Err(e) = state
        .datastore
        .shaver_runs()
        .finish_success(&input.org_id, &run_id, result_json, Some(raw_result))
        .await
    {
        let _ = state
            .datastore
            .shaving_tracks()
            .delete(&input.org_id, &input.session_id, &input.target_node_id)
            .await;
        if let Err(cleanup_err) = git::delete_ref(git_root, &ref_name).await {
            tracing::warn!(error = %cleanup_err, ref_name = %ref_name, "timeline ref cleanup failed");
        }
        return Err(e);
    }
    state.coordinator.emit(
        &input.session_id,
        SseEvent::ShavingReady {
            session_id: input.session_id.clone(),
            target_node_id: input.target_node_id.clone(),
        },
    );
    Ok(())
}

async fn provision_shaver_worktree(
    repo_root: &Path,
    data_dir: &Path,
    session_id: &str,
    parent_commit: &str,
) -> Result<PathBuf> {
    let worktree_path = data_dir
        .join("worktrees")
        .join(session_id)
        .join("shaving-gen")
        .join(Uuid::new_v4().to_string())
        .join("worktree");
    if let Some(parent_dir) = worktree_path.parent() {
        tokio::fs::create_dir_all(parent_dir)
            .await
            .with_context(|| format!("create worktree parent {}", parent_dir.display()))?;
    }
    git::worktree_add(repo_root, &worktree_path, parent_commit).await?;
    Ok(worktree_path)
}

async fn cleanup_shaver_worktree(repo_root: &Path, worktree_path: &Path) {
    if let Err(e) = teardown_shaver_worktree(repo_root, worktree_path).await {
        tracing::warn!(error = %e, worktree = %worktree_path.display(), "timeline worktree cleanup failed");
    }
}

async fn teardown_shaver_worktree(repo_root: &Path, worktree_path: &Path) -> Result<()> {
    worktree::teardown(repo_root, worktree_path).await;
    if let Some(parent_dir) = worktree_path.parent() {
        match tokio::fs::remove_dir_all(parent_dir).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("remove {}", parent_dir.display())),
        }
    }
    Ok(())
}

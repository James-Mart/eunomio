// SPDX-License-Identifier: Apache-2.0

use crate::{AppError, ResolvedPullRequest};
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

static PULL_URL_RE: OnceLock<Regex> = OnceLock::new();

fn pull_url_re() -> &'static Regex {
    PULL_URL_RE.get_or_init(|| {
        Regex::new(r"^https://github\.com/([^/]+)/([^/]+?)(?:\.git)?/pull/(\d+)/?$").unwrap()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPullUrl {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

pub fn parse_github_pull_url(input: &str) -> Result<ParsedPullUrl, AppError> {
    let trimmed = input.trim();
    let caps = pull_url_re().captures(trimmed).ok_or_else(|| {
        AppError::BadRequest(
            "expected a GitHub pull request URL (https://github.com/org/repo/pull/N); \
             for other hosts use the Branch tab"
                .into(),
        )
    })?;
    let number: u64 = caps[3].parse().map_err(|_| {
        AppError::BadRequest("invalid pull request number in URL".into())
    })?;
    Ok(ParsedPullUrl {
        owner: caps[1].to_string(),
        repo: caps[2].to_string(),
        number,
    })
}

#[derive(Debug, Deserialize)]
struct PullResponse {
    head: PullRef,
    base: PullRef,
}

#[derive(Debug, Deserialize)]
struct PullRef {
    #[serde(rename = "ref")]
    ref_name: String,
    repo: PullRepo,
}

#[derive(Debug, Deserialize)]
struct PullRepo {
    full_name: String,
}

fn map_pull_response(
    parsed: &ParsedPullUrl,
    pull: PullResponse,
) -> Result<ResolvedPullRequest, AppError> {
    if pull.head.repo.full_name != pull.base.repo.full_name {
        return Err(AppError::BadRequest(
            "fork pull requests are not supported — use the Branch tab".into(),
        ));
    }
    Ok(ResolvedPullRequest {
        remote_url: format!(
            "https://github.com/{}/{}.git",
            parsed.owner, parsed.repo
        ),
        source_ref: pull.head.ref_name,
        base_ref: pull.base.ref_name,
    })
}

pub async fn resolve_pull_request(url: &str) -> Result<ResolvedPullRequest, AppError> {
    let parsed = parse_github_pull_url(url)?;
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}",
        parsed.owner, parsed.repo, parsed.number
    );

    let client = reqwest::Client::builder()
        .user_agent("eunomio")
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("building HTTP client: {e}")))?;

    let resp = client
        .get(&api_url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("GitHub request failed: {e}")))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::BadRequest(
            "PR not found or not accessible — check the URL or use the Branch tab".into(),
        ));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(AppError::BadRequest(
            "GitHub rate limit exceeded — try again later or use the Branch tab".into(),
        ));
    }
    if !status.is_success() {
        return Err(AppError::BadRequest(format!(
            "GitHub returned HTTP {status}"
        )));
    }

    let pull: PullResponse = resp
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("invalid GitHub response: {e}")))?;

    map_pull_response(&parsed, pull)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_pull_url() {
        let parsed = parse_github_pull_url("https://github.com/gofractally/psibase/pull/1870")
            .unwrap();
        assert_eq!(parsed.owner, "gofractally");
        assert_eq!(parsed.repo, "psibase");
        assert_eq!(parsed.number, 1870);
    }

    #[test]
    fn parse_pull_url_with_trailing_slash() {
        let parsed =
            parse_github_pull_url("https://github.com/org/repo/pull/42/").unwrap();
        assert_eq!(parsed.number, 42);
    }

    #[test]
    fn parse_pull_url_with_git_suffix() {
        let parsed =
            parse_github_pull_url("https://github.com/org/repo.git/pull/7").unwrap();
        assert_eq!(parsed.repo, "repo");
    }

    #[test]
    fn reject_non_github_host() {
        assert!(parse_github_pull_url("https://gitlab.com/org/repo/-/merge_requests/1").is_err());
    }

    #[test]
    fn reject_http_scheme() {
        assert!(parse_github_pull_url("http://github.com/org/repo/pull/1").is_err());
    }

    #[test]
    fn map_same_repo_pull() {
        let parsed = ParsedPullUrl {
            owner: "gofractally".into(),
            repo: "psibase".into(),
            number: 1870,
        };
        let pull = PullResponse {
            head: PullRef {
                ref_name: "relocate".into(),
                repo: PullRepo {
                    full_name: "gofractally/psibase".into(),
                },
            },
            base: PullRef {
                ref_name: "main".into(),
                repo: PullRepo {
                    full_name: "gofractally/psibase".into(),
                },
            },
        };
        let resolved = map_pull_response(&parsed, pull).unwrap();
        assert_eq!(
            resolved.remote_url,
            "https://github.com/gofractally/psibase.git"
        );
        assert_eq!(resolved.source_ref, "relocate");
        assert_eq!(resolved.base_ref, "main");
    }

    #[test]
    fn reject_fork_pull() {
        let parsed = ParsedPullUrl {
            owner: "org".into(),
            repo: "repo".into(),
            number: 1,
        };
        let pull = PullResponse {
            head: PullRef {
                ref_name: "feature".into(),
                repo: PullRepo {
                    full_name: "forker/repo".into(),
                },
            },
            base: PullRef {
                ref_name: "main".into(),
                repo: PullRepo {
                    full_name: "org/repo".into(),
                },
            },
        };
        assert!(map_pull_response(&parsed, pull).is_err());
    }
}

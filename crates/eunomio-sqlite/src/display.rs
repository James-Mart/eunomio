// SPDX-License-Identifier: Apache-2.0

/// Derive a short repo name from a git remote URL (HTTPS, SSH, or SCP-style).
fn repo_name_from_remote_url(url: &str) -> String {
    remote_path_segments(url)
        .last()
        .cloned()
        .unwrap_or_else(|| url.trim().trim_end_matches('/').to_string())
}

/// Derive repo owner from a git remote URL when path is `owner/repo`.
fn repo_owner_from_remote_url(url: &str) -> Option<String> {
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

pub fn repo_display_parts(
    normalized_remote: &str,
    literal_remote: &str,
) -> (Option<String>, String) {
    if normalized_remote.starts_with("local:") {
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

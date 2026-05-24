// SPDX-License-Identifier: Apache-2.0

use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// Reject requests whose `Host` (or `Origin`, when present) names anything
/// other than loopback. Defends against CSRF from arbitrary sites the user
/// has open and against DNS-rebinding reads, since browsers will happily
/// connect to 127.0.0.1 under an attacker-controlled hostname.
///
/// With `--allow-dev-url`, accept `Origin` headers naming `*.trycloudflare.com`.
/// The UI is loaded from the public URL; Vite proxies `/api/*` with loopback
/// `Host` but leaves `Origin` on the trycloudflare subdomain.
pub async fn host_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let host_header = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host_header.map(is_loopback_host).unwrap_or(false) {
        return forbidden_host();
    }
    if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let dev_origin_ok = state.tunnel.allow_dev_url() && origin_is_trycloudflare(origin);
        if !origin_is_loopback(origin) && !dev_origin_ok {
            return forbidden_host();
        }
    }
    next.run(req).await
}

fn forbidden_host() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": "forbidden host", "code": "forbidden_host" })),
    )
        .into_response()
}

fn is_loopback_host(value: &str) -> bool {
    let host = strip_host_port(value);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

fn origin_is_loopback(origin: &str) -> bool {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"));
    rest.map(is_loopback_host).unwrap_or(false)
}

/// Matches origins of the form `https://<sub>.trycloudflare.com`, where
/// `<sub>` is a single non-empty label of ASCII letters, digits, or hyphens
/// (the format cloudflared's Quick Tunnel issues, and the same shape matched
/// by the URL regex in `tunnel/process.rs`). Used only with `--allow-dev-url`.
fn origin_is_trycloudflare(origin: &str) -> bool {
    let Some(rest) = origin.strip_prefix("https://") else {
        return false;
    };
    let host = strip_host_port(rest);
    let Some(sub) = host.strip_suffix(".trycloudflare.com") else {
        return false;
    };
    !sub.is_empty()
        && !sub.contains('.')
        && sub.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn strip_host_port(value: &str) -> &str {
    if value.starts_with('[') {
        if let Some(end) = value.find(']') {
            return &value[..=end];
        }
    }
    match value.rsplit_once(':') {
        Some((host, _)) if !host.is_empty() && !host.contains(':') => host,
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_loopback_hosts() {
        assert!(is_loopback_host("127.0.0.1:3001"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost:5173"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("[::1]:3001"));
        assert!(is_loopback_host("[::1]"));
        assert!(is_loopback_host("::1"));
    }

    #[test]
    fn rejects_non_loopback_hosts() {
        assert!(!is_loopback_host("example.com"));
        assert!(!is_loopback_host("example.com:3001"));
        assert!(!is_loopback_host("evil.com"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("192.168.1.1"));
        assert!(!is_loopback_host(""));
    }

    #[test]
    fn origin_loopback_classification() {
        assert!(origin_is_loopback("http://127.0.0.1:3001"));
        assert!(origin_is_loopback("http://localhost:5173"));
        assert!(origin_is_loopback("https://[::1]:8080"));
        assert!(!origin_is_loopback("http://evil.com"));
        assert!(!origin_is_loopback("evil.com"));
        assert!(!origin_is_loopback("null"));
    }

    #[test]
    fn accepts_trycloudflare_origins() {
        assert!(origin_is_trycloudflare(
            "https://tee-left-stood-ping.trycloudflare.com"
        ));
        assert!(origin_is_trycloudflare("https://abc123.trycloudflare.com"));
        assert!(origin_is_trycloudflare(
            "https://a-b-c-1-2-3.trycloudflare.com"
        ));
    }

    #[test]
    fn rejects_non_trycloudflare_origins() {
        assert!(!origin_is_trycloudflare(
            "http://tee-left-stood-ping.trycloudflare.com"
        ));
        assert!(!origin_is_trycloudflare("https://trycloudflare.com"));
        assert!(!origin_is_trycloudflare("https://.trycloudflare.com"));
        assert!(!origin_is_trycloudflare(
            "https://foo.bar.trycloudflare.com"
        ));
        assert!(!origin_is_trycloudflare(
            "https://attacker-trycloudflare.com"
        ));
        assert!(!origin_is_trycloudflare(
            "https://sub.trycloudflare.com.evil.com"
        ));
        assert!(!origin_is_trycloudflare(
            "https://has_underscore.trycloudflare.com"
        ));
        assert!(!origin_is_trycloudflare("https://has space.trycloudflare.com"));
        assert!(!origin_is_trycloudflare("ftp://sub.trycloudflare.com"));
        assert!(!origin_is_trycloudflare("https://evil.com"));
        assert!(!origin_is_trycloudflare("null"));
    }
}

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
/// In `--dev-tunnel` mode we additionally accept `Origin` headers that name a
/// `*.trycloudflare.com` subdomain. The browser loads the UI from the public
/// cloudflared URL and Vite proxies `/api/*` to the backend with
/// `changeOrigin: true` (which rewrites `Host` to loopback) but leaves the
/// original `Origin` intact. Without this exemption every mutating request
/// through the dev tunnel 403s; with it, the production CSRF/DNS-rebinding
/// defence still applies to every other deployment. The dev tunnel skips the
/// share-token gate by design, so allowing this origin does not weaken any
/// guarantee that wasn't already waived for dev.
pub async fn host_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let host_header = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host_header.map(is_loopback_host).unwrap_or(false) {
        return forbidden_host();
    }
    if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let dev_origin_ok = state.tunnel.dev_mode() && origin_is_dev_tunnel(origin);
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
/// by the URL regex in `tunnel/process.rs`). Used only when `--dev-tunnel`
/// is active.
fn origin_is_dev_tunnel(origin: &str) -> bool {
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
    fn accepts_dev_tunnel_origins() {
        assert!(origin_is_dev_tunnel(
            "https://tee-left-stood-ping.trycloudflare.com"
        ));
        assert!(origin_is_dev_tunnel("https://abc123.trycloudflare.com"));
        assert!(origin_is_dev_tunnel(
            "https://a-b-c-1-2-3.trycloudflare.com"
        ));
    }

    #[test]
    fn rejects_non_dev_tunnel_origins() {
        // http:// is rejected — quick tunnels are always https.
        assert!(!origin_is_dev_tunnel(
            "http://tee-left-stood-ping.trycloudflare.com"
        ));
        // Bare apex is rejected (no subdomain).
        assert!(!origin_is_dev_tunnel("https://trycloudflare.com"));
        assert!(!origin_is_dev_tunnel("https://.trycloudflare.com"));
        // Multi-label subdomain is rejected — quick tunnels are single-label.
        assert!(!origin_is_dev_tunnel(
            "https://foo.bar.trycloudflare.com"
        ));
        // Suffix-spoofing must not match.
        assert!(!origin_is_dev_tunnel(
            "https://attacker-trycloudflare.com"
        ));
        assert!(!origin_is_dev_tunnel(
            "https://sub.trycloudflare.com.evil.com"
        ));
        // Disallowed characters in the label.
        assert!(!origin_is_dev_tunnel(
            "https://has_underscore.trycloudflare.com"
        ));
        assert!(!origin_is_dev_tunnel("https://has space.trycloudflare.com"));
        // Other schemes / shapes.
        assert!(!origin_is_dev_tunnel("ftp://sub.trycloudflare.com"));
        assert!(!origin_is_dev_tunnel("https://evil.com"));
        assert!(!origin_is_dev_tunnel("null"));
    }
}

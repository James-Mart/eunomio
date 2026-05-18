#!/usr/bin/env bash
# Public HTTPS tunnel for the dev UI, gated by HTTP basic auth.
#
#   browser  -> *.trycloudflare.com   (Cloudflare edge)
#            -> cloudflared           (quick tunnel, no account needed)
#            -> caddy :8080           (basic_auth)
#            -> vite  :5173           (dev.sh)
#            -> /api  :3001           (eunomia, via vite proxy)
#
# Run dev.sh in a separate terminal first, with EUNOMIA_TUNNEL=1 so vite's
# HMR client connects via wss://<public-host>:443:
#
#   EUNOMIA_TUNNEL=1 ./dev.sh /path/to/repo
#
# Usage:
#   ./tunnel.sh <user> <password>
#   EUNOMIA_USER=… EUNOMIA_PASS=… ./tunnel.sh
#
# Env:
#   EUNOMIA_TUNNEL_PORT   local caddy port (default 8080)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
PORT="${EUNOMIA_TUNNEL_PORT:-8080}"
USER_NAME="${1:-${EUNOMIA_USER:-}}"
PASSWORD="${2:-${EUNOMIA_PASS:-}}"

if [[ -z "$USER_NAME" || -z "$PASSWORD" ]]; then
  echo "usage: $0 <user> <password>     (or set EUNOMIA_USER + EUNOMIA_PASS)" >&2
  exit 2
fi

mkdir -p "$BIN_DIR"

ensure_caddy() {
  if command -v caddy >/dev/null 2>&1; then return; fi
  echo "[tunnel] downloading caddy → $BIN_DIR/caddy"
  curl -fsSL "https://caddyserver.com/api/download?os=linux&arch=amd64" -o "$BIN_DIR/caddy"
  chmod +x "$BIN_DIR/caddy"
}

ensure_cloudflared() {
  if command -v cloudflared >/dev/null 2>&1; then return; fi
  echo "[tunnel] downloading cloudflared → $BIN_DIR/cloudflared"
  curl -fsSL \
    "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64" \
    -o "$BIN_DIR/cloudflared"
  chmod +x "$BIN_DIR/cloudflared"
}

ensure_caddy
ensure_cloudflared

CADDY_BIN="$(command -v caddy)"
CLOUDFLARED_BIN="$(command -v cloudflared)"

HASH="$("$CADDY_BIN" hash-password --plaintext "$PASSWORD")"

WORK_DIR="$(mktemp -d)"
CADDYFILE="$WORK_DIR/Caddyfile"
cat >"$CADDYFILE" <<EOF
{
  auto_https off
  admin off
  persist_config off
}

:$PORT {
  basic_auth {
    $USER_NAME $HASH
  }
  reverse_proxy 127.0.0.1:5173
}
EOF

pids=()
cleanup() {
  trap - EXIT INT TERM
  for pid in "${pids[@]:-}"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
  wait 2>/dev/null || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

cat <<EOF
[tunnel] caddy        : http://127.0.0.1:$PORT  (basic auth: $USER_NAME / ****)
[tunnel] reverse_proxy: http://127.0.0.1:5173   (vite dev server)
[tunnel] starting caddy + cloudflared…
[tunnel] make sure dev.sh is running with EUNOMIA_TUNNEL=1 in another terminal.

EOF

( "$CADDY_BIN" run --config "$CADDYFILE" --adapter caddyfile 2>&1 | sed -u 's/^/[caddy]      /' ) &
pids+=($!)

# Give caddy a moment to bind before cloudflared starts asking it for traffic.
sleep 1

( "$CLOUDFLARED_BIN" tunnel --no-autoupdate --url "http://localhost:$PORT" 2>&1 \
    | sed -u 's/^/[cloudflared] /' ) &
pids+=($!)

wait -n || true

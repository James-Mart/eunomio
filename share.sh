#!/usr/bin/env bash
# share.sh — full eunomia dev stack + public Cloudflare tunnel + HTTP basic auth.
#
#   browser  --https+wss-->  *.trycloudflare.com
#                  -> cloudflared
#                  -> caddy :8080  (basic_auth)
#                  -> vite  :5173  (HMR)
#                  -> /api -> eunomia :3001  (cargo-watch reloads on .rs change)
#
# Usage:
#   ./share.sh [REPO_ROOT]
#
# Env (all optional):
#   EUNOMIA_USER          basic-auth username (default: eunomia)
#   EUNOMIA_PASS          basic-auth password (skip interactive prompt if set)
#   EUNOMIA_PORT          axum port (default 3001)
#   EUNOMIA_TUNNEL_PORT   caddy port (default 8080)

set -euo pipefail

# ---- config ---------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EUNOMIA_REPO="$SCRIPT_DIR"
BIN_DIR="$HOME/.local/bin"

REPO_ROOT="${1:-$PWD}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
PORT="${EUNOMIA_PORT:-3001}"
TUNNEL_PORT="${EUNOMIA_TUNNEL_PORT:-8080}"
USER_NAME="${EUNOMIA_USER:-eunomia}"

if [[ ! -d "$REPO_ROOT/.git" && ! -f "$REPO_ROOT/.git" ]]; then
  echo "[share] warning: $REPO_ROOT does not look like a git working copy." >&2
fi

# ---- prerequisites --------------------------------------------------------
mkdir -p "$BIN_DIR"
ensure_bin() {
  local name="$1" url="$2"
  if command -v "$name" >/dev/null 2>&1; then return; fi
  echo "[share] downloading $name → $BIN_DIR/$name"
  curl -fsSL "$url" -o "$BIN_DIR/$name"
  chmod +x "$BIN_DIR/$name"
}
ensure_bin caddy "https://caddyserver.com/api/download?os=linux&arch=amd64"
ensure_bin cloudflared \
  "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64"

if ! command -v cargo-watch >/dev/null 2>&1; then
  echo "[share] cargo-watch not installed; backend won't auto-reload."
  echo "[share]   install:  cargo install cargo-watch --locked"
fi

if [[ ! -d "$EUNOMIA_REPO/frontend/node_modules" ]]; then
  echo "[share] installing frontend deps (one-time)…"
  ( cd "$EUNOMIA_REPO/frontend" && npm install )
fi

# ---- password prompt ------------------------------------------------------
if [[ -z "${EUNOMIA_PASS:-}" ]]; then
  printf '[share] basic-auth user: %q. Set a password (input hidden):\n' "$USER_NAME"
  IFS= read -rsp "  password: " EUNOMIA_PASS; echo
  IFS= read -rsp "  confirm:  " confirm; echo
  if [[ "$EUNOMIA_PASS" != "$confirm" ]]; then
    echo "[share] passwords do not match" >&2
    exit 1
  fi
  unset confirm
fi
[[ -z "$EUNOMIA_PASS" ]] && { echo "[share] empty password" >&2; exit 1; }

CADDY_BIN="$(command -v caddy)"
CLOUDFLARED_BIN="$(command -v cloudflared)"

HASH="$("$CADDY_BIN" hash-password --plaintext "$EUNOMIA_PASS")"
unset EUNOMIA_PASS

WORK_DIR="$(mktemp -d)"
CADDYFILE="$WORK_DIR/Caddyfile"
cat >"$CADDYFILE" <<EOF
{
  auto_https off
  admin off
  persist_config off
}

:$TUNNEL_PORT {
  basic_auth {
    $USER_NAME $HASH
  }
  reverse_proxy 127.0.0.1:5173
}
EOF
unset HASH

# ---- backend command ------------------------------------------------------
if command -v cargo-watch >/dev/null 2>&1; then
  backend_cmd=(
    cargo watch -q
      -w "$EUNOMIA_REPO/backend/src"
      -w "$EUNOMIA_REPO/backend/Cargo.toml"
      -x "run --manifest-path $EUNOMIA_REPO/backend/Cargo.toml -- serve --port $PORT --no-open"
  )
else
  backend_cmd=(
    cargo run --manifest-path "$EUNOMIA_REPO/backend/Cargo.toml" --
      serve --port "$PORT" --no-open
  )
fi

# ---- cleanup --------------------------------------------------------------
kill_tree() {
  local pid="$1" sig="${2:-TERM}" child
  for child in $(pgrep -P "$pid" 2>/dev/null || true); do
    kill_tree "$child" "$sig"
  done
  kill -"$sig" "$pid" 2>/dev/null || true
}
pids=()
cleanup() {
  trap - EXIT INT TERM
  for pid in "${pids[@]:-}"; do [[ -n "$pid" ]] && kill_tree "$pid" TERM; done
  sleep 0.3
  for pid in "${pids[@]:-}"; do [[ -n "$pid" ]] && kill_tree "$pid" KILL; done
  wait 2>/dev/null || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

wait_port() {
  local port="$1" tries="${2:-30}" i
  for ((i=0; i<tries; i++)); do
    if (echo > /dev/tcp/127.0.0.1/"$port") >/dev/null 2>&1; then return 0; fi
    if curl -s -o /dev/null --max-time 1 "http://127.0.0.1:$port" 2>/dev/null; then return 0; fi
    if curl -s -o /dev/null --max-time 1 "http://[::1]:$port" 2>/dev/null; then return 0; fi
    sleep 0.5
  done
  return 1
}

cat <<EOF
[share] eunomia repo : $EUNOMIA_REPO
[share] REPO_ROOT    : $REPO_ROOT
[share] backend      : http://localhost:$PORT
[share] frontend     : http://localhost:5173
[share] caddy        : http://localhost:$TUNNEL_PORT  (basic_auth user=$USER_NAME)
[share] cloudflared  : starting…

EOF

# ---- launch ---------------------------------------------------------------
( cd "$REPO_ROOT" && "${backend_cmd[@]}" 2>&1 | sed -u 's/^/[backend]    /' ) &
pids+=($!)

(
  cd "$EUNOMIA_REPO/frontend"
  EUNOMIA_TUNNEL=1 npm run dev 2>&1 | sed -u 's/^/[frontend]   /'
) &
pids+=($!)

if ! wait_port 5173 60; then
  echo "[share] vite did not bind :5173 in time" >&2
  exit 1
fi

( "$CADDY_BIN" run --config "$CADDYFILE" --adapter caddyfile 2>&1 \
    | sed -u 's/^/[caddy]      /' ) &
pids+=($!)

if ! wait_port "$TUNNEL_PORT" 20; then
  echo "[share] caddy did not bind :$TUNNEL_PORT in time" >&2
  exit 1
fi

URL_FILE="$WORK_DIR/public-url"
(
  attempt=0
  while (( attempt < 8 )); do
    attempt=$((attempt + 1))
    if (( attempt > 1 )); then
      echo "[share] cloudflared exited without URL (Cloudflare allocator flake); retry $attempt/8…" >&2
      sleep 2
    fi
    "$CLOUDFLARED_BIN" tunnel --no-autoupdate --url "http://localhost:$TUNNEL_PORT" 2>&1
    sleep 1
    [[ -s "$URL_FILE" ]] && break
  done
  if [[ ! -s "$URL_FILE" ]]; then
    echo "[share] gave up after $attempt attempts. Cloudflare's QuickTunnel allocator is" >&2
    echo "[share] returning 500s right now. Try ngrok (ngrok http $TUNNEL_PORT) instead." >&2
  fi
) 2>&1 | (
  got_url=""
  while IFS= read -r line; do
    printf '[cloudflared] %s\n' "$line"
    if [[ -z "$got_url" && "$line" =~ (https://[a-zA-Z0-9-]+\.trycloudflare\.com) ]]; then
      got_url="${BASH_REMATCH[1]}"
      printf '%s\n' "$got_url" > "$URL_FILE"
      printf '\n============================================================\n'
      printf '  PUBLIC URL : %s\n' "$got_url"
      printf '  USERNAME   : %s\n' "$USER_NAME"
      printf '  PASSWORD   : (the one you typed)\n'
      printf '============================================================\n\n'
    fi
  done
) &
pids+=($!)

wait -n || true

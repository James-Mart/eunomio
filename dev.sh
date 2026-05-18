#!/usr/bin/env bash
# Launch eunomia in dev mode: vite (HMR) on :5173, axum backend on :3001.
# Usage: ./dev.sh [REPO_ROOT]
#   REPO_ROOT defaults to $PWD; whatever git repo the script ends up running
#   from is the one the backend captures as REPO_ROOT at startup.
# Env:
#   EUNOMIA_PORT   backend port (default 3001)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EUNOMIA_REPO="$SCRIPT_DIR"
PORT="${EUNOMIA_PORT:-3001}"
REPO_ROOT="${1:-$PWD}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

if [[ ! -d "$REPO_ROOT/.git" && ! -f "$REPO_ROOT/.git" ]]; then
  echo "[dev] warning: $REPO_ROOT does not look like a git repo; backend git ops will fail." >&2
fi

if [[ ! -d "$EUNOMIA_REPO/frontend/node_modules" ]]; then
  echo "[dev] installing frontend deps (one-time)…"
  ( cd "$EUNOMIA_REPO/frontend" && npm install )
fi

if command -v cargo-watch >/dev/null 2>&1; then
  backend_cmd=(
    cargo watch -q
      -w "$EUNOMIA_REPO/backend/src"
      -w "$EUNOMIA_REPO/backend/Cargo.toml"
      -x "run --manifest-path $EUNOMIA_REPO/backend/Cargo.toml -- serve --port $PORT --no-open"
  )
else
  echo "[dev] cargo-watch not installed; backend won't auto-reload on Rust changes."
  echo "[dev]   install with:  cargo install cargo-watch"
  backend_cmd=(
    cargo run --manifest-path "$EUNOMIA_REPO/backend/Cargo.toml" --
      serve --port "$PORT" --no-open
  )
fi

pids=()
cleanup() {
  trap - EXIT INT TERM
  for pid in "${pids[@]:-}"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

cat <<EOF
[dev] eunomia repo : $EUNOMIA_REPO
[dev] REPO_ROOT    : $REPO_ROOT
[dev] backend      : http://localhost:$PORT  (api only in dev mode)
[dev] frontend     : http://localhost:5173   (open this one)
EOF

cd "$REPO_ROOT"
( "${backend_cmd[@]}" 2>&1 | sed -u 's/^/[backend]  /' ) &
pids+=($!)

( cd "$EUNOMIA_REPO/frontend" && npm run dev 2>&1 | sed -u 's/^/[frontend] /' ) &
pids+=($!)

wait -n || true

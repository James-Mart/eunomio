#!/usr/bin/env bash
# Launch eunomia in dev mode: vite (HMR) on :5173, axum backend on :3001.
# Usage: ./dev.sh
# Env:
#   EUNOMIA_PORT   backend port (default 3001)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EUNOMIA_REPO="$SCRIPT_DIR"
PORT="${EUNOMIA_PORT:-3001}"

if [[ ! -d "$EUNOMIA_REPO/frontend/node_modules" ]]; then
  echo "[dev] installing frontend deps (one-time)…"
  ( cd "$EUNOMIA_REPO/frontend" && npm install )
fi

if [[ ! -d "$EUNOMIA_REPO/helper/node_modules" ]]; then
  echo "[dev] installing helper deps (one-time)…"
  ( cd "$EUNOMIA_REPO/helper" && npm install )
fi

if command -v cargo-watch >/dev/null 2>&1; then
  backend_cmd=(
    cargo watch -q
      -w "$EUNOMIA_REPO/backend/src"
      -w "$EUNOMIA_REPO/backend/Cargo.toml"
      -x "run --manifest-path $EUNOMIA_REPO/backend/Cargo.toml -- --port $PORT"
  )
else
  echo "[dev] cargo-watch not installed; backend won't auto-reload on Rust changes."
  echo "[dev]   install with:  cargo install cargo-watch"
  backend_cmd=(
    cargo run --manifest-path "$EUNOMIA_REPO/backend/Cargo.toml" --
      --port "$PORT"
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
[dev] backend      : http://localhost:$PORT  (api only in dev mode)
[dev] frontend     : http://localhost:5173   (open this one)
EOF

cd "$EUNOMIA_REPO"
( "${backend_cmd[@]}" 2>&1 | sed -u 's/^/[backend]  /' ) &
pids+=($!)

( cd "$EUNOMIA_REPO/frontend" && npm run dev 2>&1 | sed -u 's/^/[frontend] /' ) &
pids+=($!)

wait -n || true

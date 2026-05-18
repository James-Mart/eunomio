#!/usr/bin/env bash
# Start a Cursor Agent private cloud worker for this repo (display name: eunomia).
# Requires the `cursor`/`agent` CLI.
# This script intentionally uses the "My Machines" flow (not self-hosted pool).
# Auth modes:
#   1) Default: user login (recommended) via `agent login`
#   2) Explicit API key (fallback): ./start-worker.sh --api-key <key>
#      or EUNOMIA_WORKER_API_KEY=<key> ./start-worker.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
API_KEY="${EUNOMIA_WORKER_API_KEY:-}"

usage() {
  cat <<'EOF'
Usage:
  ./start-worker.sh
  ./start-worker.sh --api-key <key>

Options:
  --api-key <key>   Explicit API key for worker auth (fallback mode)
  -h, --help        Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --api-key)
      if [[ $# -lt 2 || -z "${2:-}" ]]; then
        echo "[start-worker] --api-key requires a non-empty value." >&2
        exit 2
      fi
      API_KEY="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[start-worker] unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -n "${CURSOR_API_KEY:-}" ]]; then
  echo "[start-worker] CURSOR_API_KEY is set; ignoring it for My Machines auth." >&2
  echo "[start-worker] If this keeps happening, remove CURSOR_API_KEY from your shell profile." >&2
fi

# Default path: force user auth so browser account and worker identity stay aligned.
if [[ -z "$API_KEY" ]] && ! env -u CURSOR_API_KEY agent whoami >/dev/null 2>&1; then
  echo "[start-worker] Authentication required for My Machines worker mode." >&2
  echo "[start-worker] Run: agent login" >&2
  echo "[start-worker] Or pass a key explicitly: ./start-worker.sh --api-key <key>" >&2
  exit 1
fi

if [[ -n "$API_KEY" ]]; then
  echo "[start-worker] Starting with explicit API key auth." >&2
  echo "[start-worker] Ensure this key belongs to the same account you use on cursor.com." >&2
  exec env -u CURSOR_API_KEY cursor agent --api-key "$API_KEY" worker \
    --worker-dir "$SCRIPT_DIR" \
    start \
    --name eunomia
fi

exec env -u CURSOR_API_KEY cursor agent worker \
  --worker-dir "$SCRIPT_DIR" \
  start \
  --name eunomia

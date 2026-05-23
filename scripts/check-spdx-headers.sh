#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MISSING=0

check_file() {
  local f="$1"
  if ! head -n 5 "$f" | grep -q 'SPDX-License-Identifier: Apache-2.0'; then
    echo "missing SPDX header: $f"
    MISSING=1
  fi
}

while IFS= read -r -d '' f; do check_file "$f"; done < <(find "$ROOT/crates" -name '*.rs' -print0)
while IFS= read -r -d '' f; do check_file "$f"; done < <(find "$ROOT/frontend/src" \( -name '*.ts' -o -name '*.tsx' -o -name '*.css' \) -print0 2>/dev/null || true)
while IFS= read -r -d '' f; do check_file "$f"; done < <(find "$ROOT/helper/src" \( -name '*.ts' -o -name '*.mjs' \) -print0 2>/dev/null || true)
while IFS= read -r -d '' f; do check_file "$f"; done < <(find "$ROOT/scripts" \( -name '*.mjs' -o -name '*.sh' \) -print0)

if [ "$MISSING" -ne 0 ]; then
  echo "SPDX header check failed"
  exit 1
fi
echo "All checked files have Apache-2.0 SPDX headers"

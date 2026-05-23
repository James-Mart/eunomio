#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

add_rust() {
  local f="$1"
  if head -n 5 "$f" | grep -q 'SPDX-License-Identifier'; then return; fi
  tmp="$(mktemp)"
  printf '%s\n' '// SPDX-License-Identifier: Apache-2.0' '' > "$tmp"
  cat "$f" >> "$tmp"
  mv "$tmp" "$f"
}

add_block() {
  local f="$1"
  if head -n 5 "$f" | grep -q 'SPDX-License-Identifier'; then return; fi
  tmp="$(mktemp)"
  printf '%s\n' '/* SPDX-License-Identifier: Apache-2.0 */' '' > "$tmp"
  cat "$f" >> "$tmp"
  mv "$tmp" "$f"
}

while IFS= read -r -d '' f; do add_rust "$f"; done < <(find "$ROOT/crates" -name '*.rs' -print0)
while IFS= read -r -d '' f; do add_block "$f"; done < <(find "$ROOT/frontend/src" \( -name '*.ts' -o -name '*.tsx' -o -name '*.css' \) -print0 2>/dev/null || true)
while IFS= read -r -d '' f; do add_block "$f"; done < <(find "$ROOT/helper/src" \( -name '*.ts' -o -name '*.mjs' \) -print0 2>/dev/null || true)
while IFS= read -r -d '' f; do
  # shell scripts keep #! first; skip if already has spdx on line 2
  if head -n 3 "$f" | grep -q 'SPDX-License-Identifier'; then continue; fi
  if head -n 1 "$f" | grep -q '^#!'; then
    tmp="$(mktemp)"
    head -n 1 "$f" > "$tmp"
    echo '# SPDX-License-Identifier: Apache-2.0' >> "$tmp"
    tail -n +2 "$f" >> "$tmp"
    mv "$tmp" "$f"
  else
    add_block "$f"
  fi
done < <(find "$ROOT/scripts" \( -name '*.mjs' -o -name '*.sh' \) -print0)

echo "SPDX headers applied"

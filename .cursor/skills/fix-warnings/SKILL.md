---
name: fix-warnings
description: Fix compiler, linter, and build warnings by addressing root causes — not suppressing diagnostics. Use when the user asks to fix warnings, clean up build output, or mentions unused imports, dead code, rustc/clippy/eslint warnings.
disable-model-invocation: true
---

# Fix Warnings

Warnings are signals — incomplete refactors, dead code, wrong abstractions, latent bugs — not noise to silence. The build should be clean because the code is correct.

## Rules

- Fix **all** warnings from the relevant build/lint pass.
- **Never suppress by default** (`#[allow]`, `eslint-disable`, `@ts-ignore`) unless a documented false positive.
- **Never delete or prefix `_` blindly** — unused symbols often mean missing wiring.
- Re-run the same build/lint until output is clean.

## Workflow

```
- [ ] Collect full warning output (not a partial paste)
- [ ] Group by kind — see [reference.md](reference.md)
- [ ] Per warning: explain in one sentence why it exists; read callers/context first
- [ ] Apply smallest **correct** fix (prefer wiring > remove obsolete > refactor > migrate deprecated)
- [ ] Re-run build/lint; repeat until zero warnings
```

**Collect:** run the command that produced the warnings. Rust: `cargo build --workspace`, `cargo clippy --workspace --all-targets`. Frontend: project lint/dev script.

**Investigate before editing:** Was this symbol supposed to be used? Is the behavior obsolete or unfinished? Does nearby code already show the right pattern?

**Fix priority:** (1) complete intended behavior, (2) remove confirmed-dead code, (3) tighten structure/types, (4) migrate deprecated APIs.

**Avoid:** `_` prefixes, wildcard import deletes without reading call sites, broad lint disables, drive-by refactors.

**Unblockable warnings:** state the product/design choice needed; do not suppress silently.

## Example

Unused `use eunomio_core::unix_seconds` — **wrong:** delete the import. **right:** check whether a timestamp call was dropped in refactor; restore it or remove import *and* related dead code once confirmed obsolete.

## Report

Brief summary: warning counts by kind, root causes, changes by theme, verification command.

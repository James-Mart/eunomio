# Warning classification reference

Use when deciding how to investigate a warning — not as a cheat sheet for silencing it.

| Kind | Often means | Wrong fix | Right direction |
|---|---|---|---|
| Unused import | Incomplete refactor, speculative import | Delete without reading call sites | Wire usage or swap `*` for explicit imports |
| Unused var / mut | Unfinished logic, wrong binding | `_` prefix or delete | Wire value, fix algorithm, remove dead branch |
| Dead / unreachable code | Stub, wrong condition, obsolete path | Delete without context | Implement, fix control flow, remove obsolete feature |
| Deprecated API | Technical debt | Suppress | Migrate to replacement |
| Type mismatch | Wrong model, stale API | Cast to silence | Fix types at source |
| Clippy style | Sometimes deeper smell | Blind `clippy --fix` | Fix if clearer; investigate if it hides design issue |

### Rust notes

- `use foo::*` — restore usage or use explicit imports.
- Unused timestamps/IDs — check fields and logs dropped in refactor.
- Unused `mut` — fix algorithm; drop `mut` only when mutation is truly unnecessary.

### TypeScript notes

- Same investigation as unused vars in Rust.
- Do not disable `@typescript-eslint/no-unused-vars` file-wide.

### Suppression (rare)

Only with a comment explaining why the lint is wrong:

```rust
// Verified in tests/ffi_layout.rs — bindgen layout differs from clippy's view
#[allow(clippy::unnecessary_cast)]
```

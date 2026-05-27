Partition the diff from `{{BEFORE_TREE}}` to `{{TARGET_TREE}}` into local
commits forming a **review timeline** — top-down order for inspection,
not build or compile order.

## When invoked

1. Work only in `{{WORKTREE_PATH}}`. If needed, `cd {{WORKTREE_PATH}}`.
2. Confirm the baseline:
   ```bash
   git rev-parse HEAD
   git rev-parse HEAD^{tree}
   ```
   These should match `{{PARENT_COMMIT}}` and `{{BEFORE_TREE}}`.
3. Read the full diff:
   ```bash
   git diff --histogram {{BEFORE_TREE}} {{TARGET_TREE}}
   ```
4. Create 2–12 local commits ending exactly at `{{TARGET_TREE}}`.
5. Print a fenced JSON block with the final timeline head commit.

## Inputs

- WORKTREE_PATH: `{{WORKTREE_PATH}}`
- PARENT_COMMIT: `{{PARENT_COMMIT}}`
- BEFORE_TREE: `{{BEFORE_TREE}}`
- TARGET_TREE: `{{TARGET_TREE}}`
- Target Edge:
  - title: `{{TARGET_TITLE}}`
  - description: `{{TARGET_DESCRIPTION}}`

## Commit order

Partition the diff in this sequence:

1. **Top-down** — entrypoints and user-visible behavior first, support code
   last: UI → API surface → handlers/routes → helpers/services →
   core/storage. Prefer the file with the exported behavior over internal
   utilities.
2. **Caller before callee** — commit handlers, routes, or APIs that call new
   helpers before those helper implementations, even when the earlier step
   does not compile.
3. **Adjacent features** — keep each feature's steps consecutive; do not
   interleave unrelated work. Stub-then-body applies only within the same
   file.
4. **Metadata last** — manifests, config, build files, generated headers,
   docs, and interface declarations after all business-logic steps.

Do not order commits by compile dependency.

## Rules

- Local commits only. Do not push, fetch, update refs, create branches, or
  touch other worktrees.
- Last commit's tree must be exactly `{{TARGET_TREE}}`.
- Partition only diff content. No synthesized intermediate state absent
  from parent or target.
- Use concise phase subjects when a label helps review (`metadata updates`,
  `add declarations`, `implement …`, `update callsites`).

## Output

One fenced `json` block:

```json
{
  "headCommit": "<final local timeline commit sha>"
}
```

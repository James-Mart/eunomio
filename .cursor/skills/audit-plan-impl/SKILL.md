---
name: audit-plan-impl
description: Compares a previously implemented plan against the actual codebase and reports every deviation, no matter how minor. Use when the user provides a plan and wants an implementation audit, plan-vs-reality check, deviation report, or verification that implementation matches the plan.
disable-model-invocation: true
---

# Audit Plan Implementation

Post-implementation audit only — not for plan authoring, cleanup, or grilling. Use `/plan-cleanup` or `/grill-with-docs` before implementation; use this skill after.

Systematically compare a plan to what was implemented. Go slowly. Miss nothing.

## Inputs

The user supplies the plan — attached, pasted, or as a file path. If the plan is missing or ambiguous, ask once for it before proceeding.

Treat the plan as the source of truth for *intent*. The codebase is the source of truth for *reality*.

## Workflow

Copy this checklist and track progress:

```
Audit progress:
- [ ] Step 1: Parse plan into checkable items
- [ ] Step 2: Map each item to code locations
- [ ] Step 3: Verify each item against the codebase
- [ ] Step 4: Scan for unplanned changes in plan scope
- [ ] Step 5: Write deviation report
- [ ] Step 6: Validate completeness
```

### Step 1 — Parse the plan into checkable items

Read the entire plan before touching the codebase. Extract every checkable claim into an ordered list. Include:

- Files to create, modify, or delete
- Functions, types, traits, modules, CLI flags, config keys
- Behavioral requirements and acceptance criteria
- Test plan items and verification steps
- Documentation or ADR updates mentioned
- Explicit out-of-scope or "do not" constraints
- Ordering or dependency requirements between steps

One bullet in the plan may yield multiple checkable items. Do not merge items — split until each item is independently verifiable.

Do not proceed to Step 2 until every plan section has been decomposed into the item list.

### Step 2 — Map items to code

For each item, identify where implementation should live. Use search and file reads — do not guess.

If an item has no plausible location in the codebase, mark it **unimplemented** (`omitted` deviation) and continue.

### Step 3 — Verify each item

Work through the list **in plan order**, one item at a time. For each item:

1. Read the relevant code (and tests, if the plan mentions them).
2. Decide: **match** or **deviation**.
3. Record evidence: file paths and line ranges when applicable.

A **match** means the implementation satisfies the plan item as written — not merely "something similar exists."

If the planned artifact is absent from the codebase, record an **`omitted`** deviation.

A **deviation** is any difference, including:

- Omitted — planned work absent
- Partial — started but incomplete vs plan
- Changed — done differently (API shape, naming, location, algorithm, error handling)
- Added — extra behavior or files not in the plan but in the plan's scope area
- Constraint violated — plan said "do not X" and X was done
- Test gap — plan required a test or verification step that is missing or weaker

When unsure, treat it as a deviation and note the ambiguity.

Do not skip "minor" items. Naming differences, missing doc comments, alternate file paths, and partial implementations are all deviations.

### Step 4 — Scan for unplanned changes in scope

After the item-by-item pass, briefly scan git diff or files touched during implementation (if available) for changes within the plan's stated scope that the plan did not mention. Record each as an **added** deviation.

If git history is unavailable, skip this scan and note it in the report **Notes** section — do not treat missing diff as a deviation.

Skip unrelated repo changes outside the plan's scope.

### Step 5 — Write the report

Produce a succinct report using the template below. List **only deviations** — do not enumerate matched items unless the user asks.

### Step 6 — Validate completeness

Before submitting the report, confirm:

- Every parsed plan item was checked (parsed count = checked count).
- Every plan section appears in the audit trail (even if zero deviations).
- Every deviation row cites evidence (path, symbol, or behavior checked).
- Items that could not be verified are listed in **Notes**, not silently dropped.

Do not submit until this reconciliation passes.

## Error handling

| Situation | Action |
|-----------|--------|
| Plan missing | Ask once; stop if still missing |
| Plan too vague to parse | List ambiguities in **Notes**; audit only verifiable items |
| Item location unclear | Search codebase; if still unclear, record in **Notes** as unverified |
| Git diff unavailable | Audit from codebase only; note in **Notes** |
| Plan scope vs repo changes unclear | Prefer plan's stated scope; note boundary ambiguity in **Notes** |

## Report template

```markdown
# Plan implementation audit

**Plan:** [title or one-line description]
**Scope reviewed:** [files/areas examined]
**Summary:** [N] plan items checked · [M] deviations found

## Deviations

### [Plan section or step reference]

| # | Plan said | Implemented | Type |
|---|-----------|-------------|------|
| 1 | … | … | omitted / partial / changed / added / constraint violated / test gap |

[Repeat per section. Use one row per deviation. Keep cells short — cite paths, symbols, or behaviors, not essays.]

## Notes

- [Optional: ambiguities in the plan, items that could not be verified, or suggested follow-ups]
```

**Type** values (pick one per row):

- `omitted` — not done, or planned artifact not found in codebase
- `partial` — incomplete
- `changed` — done differently
- `added` — not in plan
- `constraint violated` — explicit "do not" ignored
- `test gap` — missing or insufficient test/verification

If there are zero deviations, say so explicitly:

```markdown
# Plan implementation audit

**Plan:** …
**Summary:** [N] plan items checked · 0 deviations found

Implementation matches the plan. No deviations identified.
```

## Example

**Plan excerpt:**

> Step 2: Add `AuthProvider` trait in `crates/eunomio-core/src/traits/auth_provider.rs` with method `fn authenticate(&self, token: &str) -> Result<User>`.
> Step 3: Do not add migration shims.

**Audit output (excerpt):**

```markdown
# Plan implementation audit

**Plan:** Auth provider trait extraction
**Scope reviewed:** crates/eunomio-core/src/traits/, crates/eunomio-auth-local/
**Summary:** 8 plan items checked · 2 deviations found

## Deviations

### Step 2 — AuthProvider trait

| # | Plan said | Implemented | Type |
|---|-----------|-------------|------|
| 1 | Method `authenticate(&self, token: &str)` | Method `verify_token(&self, creds: &Credentials)` in `auth_provider.rs:24` | changed |

### Step 3 — Constraints

| # | Plan said | Implemented | Type |
|---|-----------|-------------|------|
| 1 | Do not add migration shims | `legacy_auth_adapter.rs` wraps old API in `eunomio-auth-local/` | constraint violated |
```

## Rules

- **Thorough over fast.** Read code; do not infer from plan wording alone.
- **Plan order.** Verify in the same order the plan presents work.
- **Evidence required.** Every deviation row must cite what you checked.
- **Succinct output.** The report lists deviations only; no narration of matched work.
- **No fixes unless asked.** This skill audits; it does not implement or rewrite the plan.

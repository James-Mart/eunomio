# Wire `npm run check:license` into CI

**Status:** Deferred

## Summary

Add a CI workflow step that runs `npm run check:license` to catch FSL or other non-OSS license headers leaking into the open-source tree.

## Why deferred

CI infrastructure for the repo is not in place yet; this guard should land when a workflow exists rather than as a one-off script only runnable locally.

## Preconditions

- A CI pipeline (GitHub Actions or equivalent) that runs on pull requests and `main`.

## Implementation notes

- Run from the repo root (or `frontend/` if that is where the script is defined — confirm `package.json` location).
- Fail the job on any reported violation; no allowlist without explicit review.

## Acceptance criteria

- [ ] CI runs `npm run check:license` on every PR and default branch push.
- [ ] A deliberate FSL header in a tracked file fails the job.
- [ ] Document the check in contributor-facing docs if not already mentioned.

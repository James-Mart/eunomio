# Shavings

**Status:** Superseded by implemented Timeline docs.

Current glossary and implementation notes live in [`../CONTEXT.md`](../CONTEXT.md). User-facing Timeline behavior is documented in [`../docs/content/docs/using-eunomio/timeline.mdx`](../docs/content/docs/using-eunomio/timeline.mdx). This file is retained only as the original idea sketch and can be removed once no longer useful.

## Summary

A **shaving** is a hidden, fine-grained implementation step inside a slice. After the main partition loop produces reviewable slices, a secondary partition loop may run per slice to build a **shaving track**: an internal sequence of small intermediate commits used only for timeline-style diff playback—not canonical review objects.

**Slices answer:** What are the meaningful review units?

**Shavings answer:** How could this slice have been implemented step by step?

## Overview

After the main partition loop has split an original diff into reviewable slices, Eunomio may run a **secondary partition loop** on each slice. Output is a shaving track attached to that slice for progressive inspection without polluting the session graph or PR surface.

## Purpose

Shavings help the reviewer progressively inspect a slice without turning every tiny step into a canonical review object.

## Model

Given a slice:

```
A → B
```

Eunomio may generate:

```
A → shaving₁ → shaving₂ → shaving₃ → B
```

The shaving track is **attached to the slice**, not inserted into the main session chain.

## Invariants

A shaving track must satisfy:

```
track_base_tree == slice_parent_tree
track_head_tree   == slice_target_tree
```

and:

```
combined_shaving_diff == slice_diff
```

Additional rules:

- Shavings may be **non-compilable**.
- Shavings are **not** PR units.
- Shavings are **not** canonical session graph nodes.
- Shavings must **not** affect the final tree or main partition chain.

## Intended granularity

Shavings should represent small implementation phases, such as:

- metadata updates
- declarations
- new implementations
- call-site updates
- tests/docs
- final cleanup

They are optimized for **progressive review**, not semantic independence.

## Storage

Internally, shavings may be git-backed commits or trees.

They should live on hidden refs or slice-local tracks, for example:

```
refs/eunomio/shavings/<slice_id>
```

Each slice may reference its shaving track:

```
slice {
  id
  parent
  target
  shaving_track_id?
}
```

## UI behavior

The UI still presents **only slices** as primary objects.

When a slice is selected, its diff viewer may expose a **timeline control** backed by the shaving track.

Timeline positions map to intermediate trees:

| Position | Tree |
|----------|------|
| 0% | A (slice parent) |
| 25% | shaving₁ |
| 50% | shaving₂ |
| 75% | shaving₃ |
| 100% | B (slice target) |

The user experiences this as **progressive diff playback**, not as separate commits.

## User interaction

Users may:

- scrub through the slice timeline
- jump between implementation phases
- compare any shaving point against the slice parent
- return to the full slice diff

They should **not** need to understand or manage shavings directly.

## Relationship to other features

| Concept | Role |
|---------|------|
| Slices | Canonical review partitions on the session chain |
| Shavings | Hidden implementation playback inside a slice |
| [PR cohesion analysis](./pr-cohesion-analysis.md) | Groups/reorders slices for review plans; shavings stay out of PR proposals |

## Acceptance criteria

- [ ] Secondary partition on a slice can produce a shaving track satisfying tree and diff invariants.
- [ ] Shaving tracks are stored off the main chain (e.g. `refs/eunomio/shavings/<slice_id>`) and optional `shaving_track_id` on slice records.
- [ ] Main session graph and final tree are unchanged by shaving generation.
- [ ] Diff viewer timeline scrubs intermediate trees; full slice diff remains the default end state.
- [ ] Shavings are not exposed as PR units or canonical graph nodes in the UI.
- [ ] Non-compilable intermediate states are allowed on the track.

## Open questions

- When to run the secondary partition loop (always, on demand, or heuristics).
- Cache invalidation when a slice is regenerated or replayed.
- Whether timeline positions are discrete steps only or continuous interpolation between trees.

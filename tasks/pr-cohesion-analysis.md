# PR cohesion analysis and reordering

**Status:** Spec

## Summary

After a diff has been partitioned into a chain of slice commits, Eunomio runs an analysis pass that decides whether slices form one cohesive PR, can be split into multiple reviewable PRs, or should be reordered. Output is a review plan—dependency constraints, reordered sequences, and proposed PR groupings—validated mechanically with git and explained to the user. Proposals never mutate the canonical chain until accepted.

**Partitions answer:** What are the smallest understandable steps?

**PR cohesion analysis answers:** Which steps belong together as clean, reviewable PRs?

## Overview

Given a partitioned change set, the system:

1. Determines whether slices form **one cohesive PR**.
2. Determines whether they can be **reorganized into multiple reviewable PRs**.
3. Determines whether **slice ordering** should change.

The goal is to transform a partitioned change set into a **coherent review plan**.

## Core model

Given:

```
base → slice₁ → slice₂ → … → final
```

Each slice is a review atom:

```
patchᵢ = diff(parent(sliceᵢ), sliceᵢ)
```

The system analyzes relationships between slices and produces:

- dependency constraints
- reordered commit sequences
- proposed PR groupings

## Goals

The feature should:

- identify semantically cohesive groups of slices
- detect ordering dependencies between slices
- propose stacked or independent PRs
- validate reorderings mechanically using git
- preserve the final tree state
- explain all grouping and ordering decisions

## Synthetic change constraint

A proposed PR must **not** contain synthetic changes.

Partitions may temporarily introduce synthetic intermediate states during slice generation, but **finalized PR groupings** must resolve into clean partitions of the original diff.

Requirements:

- `Union(all proposed PR diffs) == original diff`
- No PR may contain changes that exist only for explanatory or synthetic partitioning purposes.

Synthetic commits or edges may exist internally during analysis, but exported/reviewed PRs must correspond to real semantic changes.

## Dependency analysis

The system infers relationships such as:

- slice B depends on slice A
- slice C is mechanically related to slice A
- slice D can stand alone

Signals may include:

- shared files
- symbol dependencies
- API usage
- rename/move relationships
- test/doc coupling
- cherry-pick replayability
- build/test validation

The output is a **directed dependency graph**.

## Reordering

The system may compute alternative valid slice orderings that preserve dependency constraints.

Preferred ordering heuristics include:

- mechanical refactors before behavior changes
- interfaces before implementations
- core logic before consumers
- cleanup after semantic changes

All reorderings must preserve the final tree:

```
reordered_final_tree == original_final_tree
```

## PR grouping

Slices may be clustered into PR candidates.

Example:

| PR | Content |
|----|---------|
| PR 1 | Mechanical interface extraction |
| PR 2 | Behavioral storage changes |
| PR 3 | UI and test updates |

Each proposal should include **rationale** and **dependency information** (stacked vs independent).

## Validation

All proposals must be mechanically validated using git operations such as:

- `git cherry-pick`
- `git merge-tree`
- `git range-diff`

Optional validation:

- builds
- tests
- static analysis

**Git validation is authoritative over agent reasoning.**

## User flow

The feature presents proposals **without mutating the canonical chain**.

The user may:

- accept proposal
- modify grouping
- force ordering
- merge/split groups
- rerun analysis

Only **accepted** proposals modify the session graph.

## Efficiency

Slices are git-backed immutable objects. The system can cheaply:

- diff slices
- replay reorderings
- validate stacks
- cache analysis by commit/tree OID

## Acceptance criteria

- [ ] Analysis produces a dependency graph over slices with explained edges.
- [ ] Reordering proposals satisfy dependency constraints and `reordered_final_tree == original_final_tree`.
- [ ] PR groupings satisfy the synthetic change constraint (`Union(PRs) == original diff`, no synthetic-only PR content).
- [ ] Every proposal is validated via git (cherry-pick / merge-tree / range-diff); failures block presentation or are surfaced clearly.
- [ ] UI/API exposes proposals without changing the canonical chain until user acceptance.
- [ ] User can accept, edit grouping/order, merge/split, and rerun analysis; accepted state updates the session graph.
- [ ] Rationale is shown for each grouping and ordering decision.
- [ ] Analysis results are cacheable keyed by commit/tree OID.

## Open questions

- Exact signal weighting and when to invoke optional build/test/static validation.
- How stacked PR metadata (base branch, merge order) is represented in the session graph vs export.
- Interaction with existing partition/candidate views in the session UI.

# Eunomia

A standalone tool for turning a noisy "ref A → ref B" diff into a clean, reviewable commit history by exploring a graph of synthesized commits.

## Language

**Session**:
One Eunomia working context, scoped to a specific `(baseRef, sourceRef)` pair against a specific REPO_ROOT.

**REPO_ROOT**:
The user's git repository. Captured as the server process's current working directory at startup; every git operation runs against this repo.
_Avoid_: target repo, host repo.

**State directory**:
`~/.eunomia/` by default. Holds the global SQLite database and every per-session synthesis worktree. Shared across all repos a user runs Eunomia against; Sessions are partitioned by REPO_ROOT via the `repo_root` column.

**Synthesis worktree**:
The detached git worktree owned by a Session, living at `~/.eunomia/worktrees/<sessionId>/synthesis/`. The only place subagents are ever allowed to write (subagents are out of MVP scope). Reset between Partitions.

**Node** (a.k.a. virtual Node):
A point in a Session's graph. Holds a full cumulative tree state, a synthesized commit SHA pointing at that tree, exactly one parent Node (except the seed `base`), and a Title.

**Edge**:
The diff between a Node and its parent. Edges are derived, not stored.

**base** / **final**:
The two seed Nodes every Session starts with. `base` corresponds to `merge-base(baseRef, sourceRef)^{tree}`; `final` corresponds to `sourceRef^{tree}` parented on `base`. These are Node IDs in code only by accident — the canonical IDs are UUIDs and `base`/`final` are default Titles.

**Title**:
The display name shown for a Node in the UI. Also used verbatim as the commit subject when a branch is later created from any path that walks through this Node. Default for seed Nodes is `"base"` / `"final"`; editable in the UI.
_Avoid_: nickname, label, name.

**Partition** _(deferred — out of MVP scope)_:
The single primitive for adding intermediate Nodes between an existing Node and its parent. Reserved here so the term doesn't get re-coined later.

## Relationships

- A **Session** has exactly one **REPO_ROOT** and exactly one **synthesis worktree**.
- A **Session** starts with exactly two **Nodes**: **base** and **final**.
- Every non-`base` **Node** has exactly one parent **Node**.
- A **Node**'s **Title** becomes the commit subject of the corresponding commit in any branch created from a path through it.

## Flagged ambiguities

- "nickname" (used colloquially) is the same thing as **Title** — resolved: canonical term is **Title**.

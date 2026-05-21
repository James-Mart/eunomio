# Eunomia

A standalone tool for turning a noisy "ref A → ref B" diff into a clean, reviewable commit history by exploring a graph of synthesized commits.

## Language

**Session**:
One Eunomia working context, scoped to a specific `(baseRef, sourceRef)` pair against a specific REPO_ROOT.

**REPO_ROOT**:
The user's git repository. Captured as the server process's current working directory at startup; every git operation runs against this repo.
_Avoid_: target repo, host repo.

**State directory**:
`~/.eunomia/` by default. Holds the SQLite database and every Partition's worktree. Shared across all repos a user runs Eunomia against; Sessions are partitioned by REPO_ROOT.

**Partition worktree**:
The detached git worktree owned by a single pending Partition. The only place subagents are ever allowed to write. Each Partition has its own, so Constructors from different Partitions are mutually isolated.
_Avoid_: synthesis worktree (legacy term).

**Node** (a.k.a. virtual Node):
A point in a Session's graph. Holds a cumulative tree state, a synthesized commit SHA pointing at that tree, exactly one parent Node (except the seed `base`), and a Title.

**Edge**:
The diff between a Node and its parent. Edges are derived, not stored. Identified by their target Node (every non-`base` Node has exactly one incoming Edge).

**Diff**:
The textual output of `git diff` between two trees — the rendered hunks. The domain entity that has a diff is the **Edge**, not the diff itself; an Edge is identified by its target Node, a diff is just bytes. The API exposes diffs both as part of an Edge payload and as a standalone tree-to-tree comparison.
_Avoid_: using "diff" as a synonym for **Edge** (the domain entity) — say "the Edge's diff", not "the diff between two nodes" when you mean the entity. Calling actual `git diff` output a `Diff` (struct, field, variable) is fine.

**base** / **final**:
The two seed Nodes every Session starts with. `base` is the merge-base tree of `baseRef` and `sourceRef`; `final` is the `sourceRef` tree parented on `base`. The strings `"base"` and `"final"` are their default Titles _and_ their Position labels — the words appear in two unrelated places, which is convenient but not load-bearing.

**Title**:
A descriptive string attached to each Node, used verbatim as the commit subject when a branch is later created from any path through that Node. Shown in the UI when the user selects a Node, not on the Node's graph card. Default for seed Nodes is `"base"` / `"final"`; rewritten on every Acceptance of a Partition on the Node's Edge; user-editable in the UI otherwise. See also **Description** for the paired non-editable string.
_Avoid_: nickname, name. (See **Position label** for the on-graph string.)

**Description**:
A second descriptive string attached to each Node, paired with its Title. Set by the Planner's `description` for the matching edge: the new Slice gets `edges[0].description` and the renamed target gets `edges[1].description`, written at Acceptance alongside the two Title rewrites. Not user-editable. Empty for seed Nodes (`base`, `final`). Hidden in the UI when empty.
_Avoid_: subtitle, blurb.

**Position label**:
The short identifier the graph view uses to render a Node: `base` for the seed base Node, `final` for the seed final Node (which remains the tail of the chain by construction — Partitions insert Slices before it, never replace it), and `1, 2, 3, …` for intermediates assigned by their distance from `base` in the active chain. Recomputed at render time; not stored. Distinct from a Node's **Title**.
_Avoid_: number, index, slot.

**Partition**:
The single primitive for splitting one Edge into two, by inserting a new **Slice** Node between the target Node and its prior parent and reparenting the target onto the Slice.

**Pending Partition**:
A Partition that exists as a row in the `partitions` table — i.e. has been Begun but neither Accepted nor Abandoned yet. Pending Partitions own a Partition worktree, may have an in-flight subagent Run, and may sit at a Review gate. Many can coexist in a Session at once.

**Slice**:
The single new Node a Partition adds. Built by the Constructor; parented on the prior parent of the target Node. Its Title comes from the Planner's description of the slice Edge.
_Avoid_: intermediate, sub-edge.

**Strategy**:
One of three slicing modes a Partition uses — `Synthetic`, `Vertical`, or `Horizontal`. The Planner chooses the strategy on its first run (always starting from `auto`); the user can override the strategy for a specific Partition at the Plan Review gate when asking for a re-plan. The strategy frames the Constructor's scope rules and the slice/leftover boundary.

**Synthetic**:
A **Strategy** whose **Slice** tree contains a **synthesized intermediate** — content present in neither BeforeTree nor TargetTree — chosen so the Slice applies exactly one **Theme** without applying any other in the diff. Prefer **Vertical** or **Horizontal** when the Theme is already a literal hunk-subset of TargetTree — a Synthetic slice should only be used when synthesis is required.
_Avoid_: semantic (legacy name).

**Vertical**:
A **Strategy** whose **Slice** is a literal subset of the diff's hunks that cuts through every architectural layer the diff touches, so the Slice and the leftover are each independently working code. No synthesized intermediate — every line in the Slice's tree appears in BeforeTree or TargetTree.

**Horizontal**:
A **Strategy** whose **Slice** is a literal subset of the diff's hunks confined to one architectural layer (types, schema, service, UI, etc.); the leftover owns every other layer in foundation order. No synthesized intermediate — every line in the Slice's tree appears in BeforeTree or TargetTree.

**Synthesized content**:
The diff-view rendering concept: word-level marks on an Edge's diff showing content that differs from the Edge's **Reference pair** — parent-side removals relative to the pair's before tree (`synthetic−`) and child-side additions relative to the pair's after tree (`synthetic+`). In **Canonical view** every Edge uses `(base.tree, final.tree)` as its Reference pair; in **Candidate view** both candidate Edges use the Partition's `(BeforeTree, TargetTree)`. **`synthetic~`** denotes content present in both reference trees but absent from the Edge's parent/child trees; it is a glossary term only and is not rendered in the UI.
_Avoid_: defining synthesized content relative to `final.tree` alone; "transient", "synthetic content" (collides with the **Synthetic** Strategy).

**Reference pair**:
The two trees an Edge's synthesized marks are computed against: `(beforeRef, afterRef)`. Canonical Edges default to `(base.tree, final.tree)`; candidate Edges use the Partition's `(BeforeTree, TargetTree)`.

**Canonical view**:
The default graph view showing the accepted Node chain (`base → 1 → … → final`). Each Edge's synthesized marks use Reference pair `(base.tree, final.tree)`.

**Original view**:
A two-Node graph view (`base → final`) showing the seed diff before any Partitions. Selecting **final** shows the `base→final` diff with no synthesized marks; selecting **base** shows an empty diff pane.

**Theme**:
One coherent cluster of changes inside a diff — a feature, a refactor, a bug fix, a layer rewrite — that could be reviewed, described, or reverted on its own. Produced by the Surveyor as the `themes[]` list inside a ChangeSurvey. Themes are the candidate set the Planner draws from when it chooses the `Synthetic` strategy; under `Vertical` or `Horizontal` the planner uses them as supporting context but slices along different axes.
_Avoid_: concern (carries a negative valence), topic, item.

**Coordinator**:
The orchestrator that schedules and supervises Surveyor / Planner / Constructor Runs for a Partition. Not itself a subagent.

**Constructor**:
The subagent that writes to a Partition's worktree to build the Slice the Planner identified. The only writable subagent. Returns either `OK` or `BLOCKED: <reason>` on a single line.

**Partition settings**:
User-global configuration that applies to every Partition across every Session for this user. Stored as a single JSON file under the **State directory** (`~/.eunomia/settings.json`), not on the `sessions` row. Structured by subagent role (Surveyor / Planner / Constructor) plus Coordinator. The Coordinator owns the three HITL flags (`afterSurvey`, `afterPlanning`, `afterConstruct`) and the default model that applies to every subagent unless overridden on the subagent's own tab.

**Phase**:
A stage of a Partition — `Survey`, `Plan`, or `Construct`. The Coordinator drives the Partition through phases in this fixed order. Phases are the granularity at which Review gates apply.

**Review gate**:
A Coordinator-controlled halt at a Phase boundary, governed by the matching `humanInTheLoop.*` flag in Partition settings. All three (`afterSurvey`, `afterPlanning`, `afterConstruct`) exist and default to ON. The Construct gate is where Acceptance happens.

**Acceptance**:
The terminal-success outcome of a Partition: inserts the new Slice Node, reparents the target Node onto the Slice, rewrites the target's Title, removes the Partition's worktree, and auto-Abandons every other Partition pending on the same target. Triggered by the user at the Construct Review gate, or automatically when `afterConstruct` is off.

**Candidate view**:
A graph-view mode the user enters via a dropdown when one or more Partitions are pending Acceptance. Renders a 3-Node mini-graph (the target's prior parent, the candidate Slice, and the renamed target) so the user can inspect the proposed graph state — including selecting Nodes to view their incoming-edge diffs — before Accepting.

**Sibling Partitions**:
Two or more pending Partitions sharing the same target Node. Allowed by design — the user can run alternative Partitions (e.g. one Vertical and one Horizontal) on the same target and compare. At most one of them has an actively-executing phase at a moment. Accepting any one of them auto-Abandons the others.

**Indivisible verdict**:
A Planner output declaring that the diff between a Partition's BeforeTree and TargetTree is already a single cohesive change and should not be split further. Serialised as `{ outcome: "indivisible", rationale: "…" }` on the Planner's JSON output, parallel to the Constructor's `BLOCKED` outcome. Terminates a branch of the **Auto fan-out** loop. Governed by `humanInTheLoop.afterIndivisible` in Partition settings (default on): when on, the Partition parks at the Plan Review gate for the user to confirm or push back; when off, the Partition is auto-Abandoned without surfacing the gate.

**Auto fan-out**:
A Coordinator-driven loop that turns one user-initiated Begin Partition into a binary tree of Partitions: each Acceptance auto-Begins two new Partitions, one targeting the newly inserted Slice's incoming Edge and one targeting the renamed-target's incoming Edge. Configured by `coordinator.maxIterations` in Partition settings: `{ kind: "count", count: N }` caps the tree depth at N (count=1 disables fan-out entirely, matching the pre-feature behaviour); `{ kind: "auto" }` removes the depth cap. Branches terminate naturally on an **Indivisible verdict**, a Constructor `BLOCKED`, a Run error, a user Abandon, or the depth budget reaching zero. Orthogonal to the HITL flags — each Phase still respects its own `afterX` flag, so a user can run Auto fan-out with HITL on and review every gate in the tree manually.

## Relationships

- A **Session** has exactly one **REPO_ROOT** and starts with exactly two **Nodes**: **base** and **final**.
- Every non-`base` **Node** has exactly one parent **Node**. The canonical graph is therefore a single linear chain `base → … → final`.
- Each pending **Partition** owns exactly one **Partition worktree** for the duration of its existence.
- A **Partition** row exists only between Begin and a terminal action (Acceptance or Abandon). The Slice it produced and the rewrite of the target Node persist after Acceptance; Run rows live alongside the Partition row and are deleted with it at the terminal action.
- Many **Partitions** can be pending in a Session at any moment, including **Sibling Partitions** on the same target.

## Flagged ambiguities

- "nickname" (used colloquially) is the same thing as **Title** — resolved: canonical term is **Title**.
- "executor" (used colloquially) is the same thing as **Constructor** — resolved: canonical term is **Constructor**.
- "lifecycle" was previously used as a domain noun for the per-Partition flow — resolved: **Partition** is the entity. "Lifecycle" must not be used as a substitute for "Partition" (e.g. "begin a Lifecycle", "Lifecycle row"). It remains a legitimate _descriptor_ of a Partition's flow through its Phases — both in prose ("the Partition's lifecycle") and as a qualified identifier for things that describe that flow (e.g. the `Lifecycle` snapshot type, `LifecycleStepper` UI widget, `usePartitionLifecycle` hook).
- "concern" was previously the name for a survey item — resolved: canonical term is **Theme**. "Concern" is avoided because it carries a negative valence (the survey describes what's in the diff, neutrally; it does not flag worries).
- "user concern" was previously a separate upfront input channel to the Surveyor and Planner — resolved: collapsed into `user_feedback`, supplied only at Review gates on re-runs. There is no upfront user-supplied prose on a Partition.

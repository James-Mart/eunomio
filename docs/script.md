# Eunomio Overview Video — Script

Product intro / trailer for the docs site. Explains what eunomio is and what makes it different in under 90 seconds.

**Target duration:** ~85s  
**Tone:** Confident, developer-native, plainspoken — no hype, no "AI magic"  
**Format:** Voiceover + motion graphics (no UI screen recording required for v1)  
**Aspect ratio:** 16:9 (1920×1080), also crops cleanly to 1:1 for social if needed

---

## What this video must land

By the end, a viewer should understand:

1. **The problem** — one big diff is hard to review and impossible to revert in pieces.
2. **The promise** — same final code, a clean commit history you can explore and ship.
3. **The mechanism** — AI subagents split the diff one commit at a time; you review at every step.
4. **The differentiators** — git-native worktrees, human-in-the-loop gates, virtual graph → real branch.

---

## Scene breakdown

### Scene 1 — The problem · 0:00–0:12

**Visual:** A single oversized diff pane — dozens of mixed hunks (refactor, feature, config, tests). A reviewer cursor hesitates. Fade to a git log with one giant commit: `WIP: everything`.

**On-screen text:** `One branch. One diff. One commit.`

**VO:**

> You finished the feature. The code is right.
> But the history is a mess — one noisy diff, one commit nobody wants to review.

---

### Scene 2 — The pitch · 0:12–0:22

**Visual:** Title card: **eunomio**. Subtitle animates in beneath. Background resolves to a minimal linear graph: `base → final`, two nodes connected by a thick edge labeled with diff size.

**On-screen text:** `Turn a noisy ref diff into a clean, reviewable commit history.`

**VO:**

> Eunomio turns a ref-to-ref diff into a clean, reviewable commit history —
> without you hand-crafting every commit.

---

### Scene 3 — The session · 0:22–0:32

**Visual:** Zoom into the graph. Label the nodes: `base` (merge-base tree) and `final` (your branch tip). Emphasize: **same final code**, bad history. Split layout: graph left, diff right.

**On-screen text:** `base → final` · `Same code. Better history.`

**VO:**

> You start with two points: where you branched, and where you landed.
> Same final code — just organized into commits that make sense.

---

### Scene 4 — The partition · 0:32–0:48

**Visual:** User clicks **Begin partition** on the `final` edge. Three subagent cards animate in sequence along a horizontal pipeline:

| Surveyor | Planner | Constructor |
|----------|---------|-------------|
| reads the diff | picks how to cut | writes one commit |

Each phase pauses at a **Review gate** (pause icon, green outline). Feedback loop arrow on re-run. Worktree badge appears beside Constructor: `isolated git worktree`.

**On-screen text:** `Survey → Plan → Construct` · `You review at every step.`

**VO:**

> Eunomio splits the diff one commit at a time.
> Three specialized agents survey the change, plan the cut, and construct the slice —
> each in an isolated git worktree.
> You review at every step. Accept, give feedback, or abandon. You're steering, not watching.

---

### Scene 5 — Graph grows · 0:48–0:58

**Visual:** Accept animation — the partition primitive: `base → final` becomes `base → 1 → final`. The edge between `1` and `final` shrinks (smaller diff). Repeat once more: `base → 1 → 2 → final`. Each new node gets a small, readable diff in the diff pane.

**On-screen text:** `One partition. One new commit.`

**VO:**

> Each accepted split inserts a new commit into the chain.
> The graph grows. The diffs shrink.
> Every edge becomes something you can actually review.

---

### Scene 6 — What makes it different · 0:58–0:72

**Visual:** Three callouts animate in, staggered:

1. **Git-native** — worktree icon, `git diff`, virtual commit nodes
2. **Human-in-the-loop** — review gates default ON
3. **Explore before you ship** — cursor selects node `2`; ghost branch path highlights `base → 1 → 2`

**On-screen text (staggered):**
- `Real git. Real worktrees.`
- `Gates on by default.`
- `Branch from any point.`

**VO:**

> It's git-native — real worktrees, real diffs, real commits when you're ready.
> Review gates are on by default, so automation never runs ahead of you.
> And the graph is explorable: pick any point, spin off a branch, open a PR.

---

### Scene 7 — Outcome · 0:72–0:82

**Visual:** Canonical chain `base → 1 → 2 → 3 → final`. Morph to a clean git log with readable subjects. PR icon appears. Final tree hash matches throughout (subtle checkmark on `final`).

**On-screen text:** `Same final code. Commits worth reviewing.`

**VO:**

> Same final code.
> A commit history worth reviewing.
> Eunomio is the orchestration layer between your WIP branch and a PR you're proud to open.

---

### Scene 8 — Close · 0:82–0:85

**Visual:** Logo lockup. URL fades in.

**On-screen text:** `eunomio` · `Read the docs →`

**VO:**

> Eunomio.

*(Optional: no VO on close — let the title card breathe.)*

---

## Full voiceover (continuous)

> You finished the feature. The code is right.
> But the history is a mess — one noisy diff, one commit nobody wants to review.
>
> Eunomio turns a ref-to-ref diff into a clean, reviewable commit history —
> without you hand-crafting every commit.
>
> You start with two points: where you branched, and where you landed.
> Same final code — just organized into commits that make sense.
>
> Eunomio splits the diff one commit at a time.
> Three specialized agents survey the change, plan the cut, and construct the slice —
> each in an isolated git worktree.
> You review at every step. Accept, give feedback, or abandon. You're steering, not watching.
>
> Each accepted split inserts a new commit into the chain.
> The graph grows. The diffs shrink.
> Every edge becomes something you can actually review.
>
> It's git-native — real worktrees, real diffs, real commits when you're ready.
> Review gates are on by default, so automation never runs ahead of you.
> And the graph is explorable: pick any point, spin off a branch, open a PR.
>
> Same final code.
> A commit history worth reviewing.
> Eunomio is the orchestration layer between your WIP branch and a PR you're proud to open.
>
> Eunomio.

**Word count:** ~195 · **Estimated read time at moderate pace:** ~80–85s

---

## Deliberately omitted (save for follow-up videos)

| Topic | Why omitted |
|-------|-------------|
| Synthetic / Vertical / Horizontal strategies | Too much for a trailer; belongs in a strategies clip |
| Candidate vs Canonical vs Original views | Preview-lens concept needs its own 40s beat |
| Parallel partitions / auto fan-out | Power-user; confuses the first impression |
| Timeline / Shavings | Review aid, not core value prop |
| Cursor SDK / Rust architecture | Implementation detail |
| Security / tunnel sharing | Ops audience, not intro |

---

## Production notes

- **Music:** Minimal, low-tempo electronic or muted piano — never overpower VO.
- **SFX:** Subtle UI clicks on Accept / Begin; soft whoosh on graph mutations.
- **Captions:** Burn in or ship as WebVTT alongside embed — many docs readers watch muted.
- **Brand:** Dark neutral background (`#0A241B` family), green accent on progress/acceptance (`#0FBF3E`), monospace for git labels, Mona Sans for titles (match docs site).
- **Remotion mapping:** Each scene above = one `<Sequence>` or nested composition. Reusable components: `CommitChain`, `DiffPane`, `SubagentPipeline`, `HITLGate`, `AcceptMutation`.

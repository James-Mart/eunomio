# Eunomio Overview Video — Script

Product intro / trailer for the docs site. Frames eunomio as a **third option** for code review in the AI era — not full manual review at impossible scale, and not delegated AI review that skips senior judgment.

**Target duration:** ~100s (trimmable to ~75s by cutting Scene 4 citation card and shortening Scene 8)  
**Tone:** Direct, senior-engineer plainspoken — problem-first, no "AI magic"  
**Format:** Voiceover + motion graphics  
**Aspect ratio:** 16:9 (1920×1080)

---

## Narrative thesis

Code review is broken because **git commits serve the author, not the reviewer**. The person who eventually receives the PR essentially has to absorb the **entire diff at once**. We're all now familiar with the concept of progressive disclosure and protecting the context-window of our agents, well, it turns out the same principles apply to humans. 

Research backs this up: review effectiveness **falls as change size grows**. Reviewers just rubber-stamp it because it's too overwhelming to validate with the optimal level of depth.

This has been a problem for a long time, but the AI era makes this urgent. Code is being produced faster than ever. Teams face two bad options:

1. **Review everything properly** — correct, but doesn't scale; nobody wants review to be their full-time job.
2. **Delegate review to AI** — useful for bugs and style, but not yet for the senior question: _Should this change exist? Are the assumptions right? Do the abstractions fit the strategy?_

**Eunomio is a third option:** full **human-in-the-loop** code review, with AI used to **modernize the review process itself** — not to replace the reviewer.

---

## What this video must land

By the end, a viewer should understand:

1. **Why review breaks** — commits are author checkpoints; big diffs overwhelm human attention.
2. **Why it matters now** — higher code velocity + unchanged review habits = quality risk.
3. **Why AI review isn't enough** — senior judgment is about assumptions and architecture, not just correctness.
4. **What eunomio is** — AI helps _structure_ review for humans: smaller diffs, step by step, you stay in control.

---

## References (for VO attribution, on-screen citations, or docs footnotes)

The term **"lazy review"** is not a standard name in the literature. The closest established concepts:

| Concept                           | What it means                                                                                                                            | Citation                                                                                                                                                                                                            |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Defect density falloff**        | Defects found per 1,000 LOC drops sharply once a review exceeds ~200 lines; 400 LOC is treated as a practical maximum                    | Jason Cohen, _Best Kept Secrets of Peer Code Review_ — Cisco Systems case study (SmartBear, 2006). [PDF](https://static1.smartbear.co/support/media/resources/cc/episode_4_thelargestcasestudyofcodereviewever.pdf) |
| **Light review of large changes** | Beyond ~180 SLOC, immediate-acceptance rates rise again — a "bathtub curve" suggesting reviewers may treat large changes less thoroughly | Kudrjavets, Kumar, Nagappan & Rastogi, _Mining Code Review Data to Understand Waiting Times Between Acceptance and Merging_ (2022). [arXiv:2203.05048](https://arxiv.org/abs/2203.05048)                            |
| **Rubber-stamping**               | Large PRs receive far fewer comments per line; approvals without substantive scrutiny                                                    | Industry analyses of GitHub PR data (e.g. CodePulse rubber-stamp studies, 2024–2025)                                                                                                                                |
| **Small changes as practice**     | Modern review converges on lightweight, iterative review of small changes; Google median CL ≈ 24 lines changed                           | Rigby & Bird, _Convergent Contemporary Software Peer Review Practices_ (ESEC/FSE 2013); Sadowski et al., _Modern Code Review: A Case Study at Google_ (ICSE-SEIP 2018)                                              |
| **Progressive disclosure**        | Show essentials first; reveal detail on demand — reduces cognitive load                                                                  | Nielsen Norman Group, [Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/) (HCI); adopted in agent/context-window design                                                              |

**Suggested on-screen callout (Scene 4):** _"Review effectiveness drops as change size grows — Cisco study, 2006; Kudrjavets et al., 2022"_

---

## Scene breakdown

### Scene 1 — Commits for the author · 0:00–0:14

**Visual:** Developer timeline — quick commits flash by: `wip`, `fix tests`, `actually fix`, `refactor maybe`, `ok this time`. They feel productive. Cut to reviewer opening one PR — single diff, enormous.

**On-screen text:** `Your commits. Their problem.`

**VO:**

> For most of us, commits are checkpoints — save points on _your_ path through a feature.
> They were never really designed for the person who has to review them.
> You ship the branch. They get the whole diff.

---

### Scene 2 — What commit-by-commit would reveal · 0:14–0:26

**Visual:** Same history rewound commit-by-commit: a wrong abstraction, a revert, a pivot in approach — each visible for a moment, then buried in the squash. Ghost overlays show "this never made it to the PR description."

**On-screen text:** `The story is in the history. The review isn't.`

**VO:**

> If someone reviewed you commit by commit, they'd see the false starts —
> the strategy changes, the mistakes you fixed along the way.
> But that's not what code review looks like. It's one wall of changes, all at once.

---

### Scene 3 — Progressive disclosure · 0:26–0:38

**Visual:** A diff pane tries to show everything — files, layers, concerns — labels overflow. Split: left side "everything at once" (overloaded), right side "one edge at a time" (readable chunk with room to think). Optional subtitle: _progressive disclosure_.

**On-screen text:** `Progressive disclosure` · `Reveal what matters now.`

**VO:**

> We learned from working with AI agents: don't dump the whole context — disclose what matters now, add more when you need it.
> Progressive disclosure.
> Humans need the same thing. A giant diff doesn't make you thorough. It overwhelms you.

---

### Scene 4 — The size problem · 0:38–0:50

**Visual:** Simple chart — defect-finding effectiveness vs. lines under review; cliff after ~200 LOC. Optional small citation line at bottom. Reviewer cursor accelerates through lines — skim, not read. `LGTM` appears on a 1,200-line PR.

**On-screen text:** `Past ~200 lines, review quality drops.` · _(citation card)_

**VO:**

> And we know what happens next. Study after study shows review quality falls as the change gets bigger —
> reviewers skim, rubber-stamp, or give up.
> The harder the change, the _less_ attention it often gets.

---

### Scene 5 — The AI era · 0:50–0:58

**Visual:** Code output rate graph rising steeply; review queue grows in parallel; same number of human reviewers. Tension, not panic.

**On-screen text:** `More code. Same reviewers.`

**VO:**

> Now code is being produced faster than ever.
> If we don't change how review works, we risk drowning — and calling it "ship velocity."

---

### Scene 6 — Two bad options · 0:58–1:12

**Visual:** Split screen — two paths:

**Path A — Review everything:** Reviewer buried in diffs, clock spinning, "correct but impossible."

**Path B — AI review:** Bot finds typos and bugs (green checks) but misses architecture diagram — wrong abstraction, wrong dependency direction. Label: _"Does it work?" ≠ "Should it exist?"_

**On-screen text:** `Review everything` · `Delegate to AI` · `Both break down.`

**VO:**

> You're stuck with two bad options.
> Review everything properly — correct, but nobody has that kind of time.
> Or delegate review to AI — fine for bugs, not for the senior question:
> Should this feature exist? Are the assumptions right? Do these abstractions fit where we're going?

---

### Scene 7 — The third option · 1:12–1:22

**Visual:** Title card **eunomio**. Third path opens between the two failed paths. Tagline beneath.

**On-screen text:** `Human review. Modernized.` · `Not AI code review. AI for code review.`

**VO:**

> Eunomio is a third option.
> It's not AI doing your code review.
> It's AI helping _you_ do human code review — full human in the loop, with the process rebuilt for how review actually works.

---

### Scene 8 — How it works · 1:22–1:38

**Visual:** Session graph `base → final`. **Begin partition** on the big edge. Surveyor → Planner → Constructor pipeline with **Review gates** (pause icons). Accept: `base → 1 → final` — smaller diff on the new edge. Repeat once. Human figure at each gate — steering, not spectating.

**On-screen text:** `Split the diff. Review each piece. You decide.`

**VO:**

> You point eunomio at a ref-to-ref diff.
> Agents survey it, plan how to split it, and construct one commit at a time — in isolated git worktrees.
> You review at every step. Accept, push back, or abandon.
> The diff shrinks. The history becomes something a human can actually review — and defend.

---

### Scene 9 — Close · 1:38–1:42

**Visual:** Clean commit chain morphs to PR. Logo lockup.

**On-screen text:** `eunomio` · `Read the docs →`

**VO:**

> Same final code. A review process worth trusting.
> Eunomio.

---

## Full voiceover (continuous)

> For most of us, commits are checkpoints — save points on _your_ path through a feature.
> They were never really designed for the person who has to review them.
> You ship the branch. They get the whole diff.
>
> If someone reviewed you commit by commit, they'd see the false starts —
> the strategy changes, the mistakes you fixed along the way.
> But that's not what code review looks like. It's one wall of changes, all at once.
>
> We learned from working with AI agents: don't dump the whole context — disclose what matters now, add more when you need it.
> Progressive disclosure.
> Humans need the same thing. A giant diff doesn't make you thorough. It overwhelms you.
>
> And we know what happens next. Study after study shows review quality falls as the change gets bigger —
> reviewers skim, rubber-stamp, or give up.
> The harder the change, the _less_ attention it often gets.
>
> Now code is being produced faster than ever.
> If we don't change how review works, we risk drowning — and calling it "ship velocity."
>
> You're stuck with two bad options.
> Review everything properly — correct, but nobody has that kind of time.
> Or delegate review to AI — fine for bugs, not for the senior question:
> Should this feature exist? Are the assumptions right? Do these abstractions fit where we're going?
>
> Eunomio is a third option.
> It's not AI doing your code review.
> It's AI helping _you_ do human code review — full human in the loop, with the process rebuilt for how review actually works.
>
> You point eunomio at a ref-to-ref diff.
> Agents survey it, plan how to split it, and construct one commit at a time — in isolated git worktrees.
> You review at every step. Accept, push back, or abandon.
> The diff shrinks. The history becomes something a human can actually review — and defend.
>
> Same final code. A review process worth trusting.
> Eunomio.

**Word count:** ~280 · **Estimated read time at moderate pace:** ~95–105s

---

## Trimming guide (if targeting ~75s)

| Cut or compress                                  | Saves |
| ------------------------------------------------ | ----- |
| Scene 4 citation card → single line VO, no chart | ~8s   |
| Scene 2 → one sentence, faster visual montage    | ~6s   |
| Scene 8 → one accept animation, not two          | ~8s   |
| Scene 5 → merge into Scene 6 opening             | ~5s   |

---

## Deliberately omitted (save for follow-up videos)

| Topic                                             | Why omitted                                         |
| ------------------------------------------------- | --------------------------------------------------- |
| How partitions mutate the graph (slice insertion) | Mechanism detail; this video sells _why_, not _how_ |
| Synthetic / Vertical / Horizontal strategies      | Belongs in a strategies clip                        |
| Candidate vs Canonical views                      | Preview-lens concept needs its own beat             |
| Branch from any node                              | Outcome detail; implied by "reviewable history"     |
| Timeline / Shavings                               | Review aid, not core pitch                          |
| Cursor SDK / architecture                         | Implementation detail                               |

---

## Production notes

- **Music:** Tense but restrained in Scenes 1–6; opens up at Scene 7. Never overpower VO.
- **Captions:** Required — many viewers will watch muted in docs.
- **Brand:** Dark neutral background (`#0A241B` family), green on acceptance/HITL gates (`#0FBF3E`), red/amber for "two bad options" paths, Mona Sans + monospace git labels.
- **Remotion mapping:** Scenes 1–7 are mostly narrative motion graphics; Scene 8 reuses product-adjacent components (`CommitChain`, `SubagentPipeline`, `HITLGate`, `AcceptMutation`) from the prior script iteration.

---
name: tune-subagent-prompt
description: Orchestrate survey subagent prompt tuning via eunomia subagent-run, prompt-analyst critique, and tuning-log.json. Kickoff requires kind, targetNodeId, and hitl (true|false).
---

# Tune subagent prompt

Iteratively tune an Eunomia subagent prompt for **survey** runs (`kind: survey`). API and CLI support all kinds; this skill workflow is survey-only in v1.

## Kickoff parameters

| Param | Required | Notes |
| --- | --- | --- |
| `kind` | yes | Must be `survey` for this skill |
| `targetNodeId` | yes | Copy from the node Info tab (canonical view) |
| `hitl` | yes | `true` = pause after each iteration verdict for human confirmation; `false` = apply verdict automatically |

## Two HITL concepts

- **Partition HITL** (Eunomia settings): always enable survey HITL in Step 0 so the partition parks at `awaiting_review` for re-runs.
- **Skill HITL** (`hitl` kickoff flag): when `true`, stop after each orchestrator verdict and ask the human to confirm or override before acting.

## Prerequisites

- Eunomia server running (default `http://127.0.0.1:3001`).
- Repo root is the Eunomia project (for embedded prompts and CLI).

## Step 0 — begin partition

1. `GET /api/nodes/:targetNodeId/session` → `sessionId`.
2. `PATCH /api/partition-settings` — enable survey HITL for this session/target.
3. `POST /api/sessions/:sessionId/edges/:targetNodeId/partition` — begin a fresh partition.
4. Poll partition / SSE until survey phase is `awaiting_review`.
5. Create `.cursor/tuning-log.json`:

```json
{
  "kind": "survey",
  "targetNodeId": "<uuid>",
  "sessionId": "<uuid>",
  "partitionId": 0,
  "hitl": false,
  "iteration": 0,
  "currentPrompt": null,
  "iterations": []
}
```

Set `partitionId`, `hitl`, and seed `currentPrompt` from `GET /api/subagent-prompts` → `surveyor`.

## Loop

1. Write `currentPrompt` to a temp file.
2. Run CLI (no `userFeedback`):

```bash
eunomia subagent-run \
  --base-url http://127.0.0.1:3001 \
  --partition-id <partitionId> \
  --kind survey \
  --prompt-file /path/to/prompt.md
```

3. Invoke **prompt-analyst** subagent with the prompt template + transcript JSON.
4. Form an orchestrator **verdict**:
   - Apply or reject the analyst's proposed revision
   - Continue or stop the loop
   - Rationale referencing prior `tuning-log.json` entries
5. **If `hitl: true`:** present to the human:
   - Analyst summary (critique + proposed revision)
   - Orchestrator verdict (apply/reject, continue/stop)
   - Rationale vs log history
   - Ask for confirm or override before proceeding
6. **If `hitl: false`:** apply verdict automatically.
7. Update `tuning-log.json` (`iteration`, `iterations[]`, `currentPrompt` if accepted).
8. Repeat or exit.

## Auto-stop rules

Propose stop (human confirms when `hitl: true`) when any of:

- 10 iterations reached
- 2 consecutive rejected proposals
- Analyst reports no material issues

## API reference

| Endpoint | Use |
| --- | --- |
| `GET /api/nodes/:nodeId/session` | Resolve `sessionId` from node UUID |
| `GET /api/subagent-prompts` | Default `{ surveyor, planner, constructor }` bodies |
| `POST /api/partitions/:id/runs` | `{ "kind": "survey", "promptOverride": "..." }` — omit/`""` uses embedded default |
| `GET /api/partitions/:id/runs/:runId/transcript` | Transcript for analyst |

## Out of scope

- Plan or construct tuning workflows (deferred)
- Persisting overrides in settings or UI
- Auto-committing to `subagents/*.md`
- Reusing existing partitions — always begin fresh from `targetNodeId`

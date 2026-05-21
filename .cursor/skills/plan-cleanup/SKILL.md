---
name: plan-cleanup
description: Manually tag this skill after an agent has produced a plan so the assistant considers your standard planning requirements before finalizing a plan.
---

# Plan cleanup

## Step 1 - Review

Review the plan and ensure it plans to implement something idiomatic/conventional. Consider good abstraction techniques and encapsulation practices. Pay special attention to any public facing APIs. You should hide complexity as much as possible and only expose what is necessary.

If the plan is adding functionality into an existing application, consider if the new features follow the patterns established by the existing app, and reuse shared functionality / libraries / components / modules where applicable. Don't roll your own custom functionality unless it is necessary + doesn't already exist.

## Step 2 - Section to add

Append the following section verbatim to the plan, after the implementation steps and before any "Test plan" or trailing sections. Do not paraphrase — these are explicit requirements for the agent that will implement the plan.

```markdown
## Implementation requirements

The agent implementing this plan must follow these requirements:

- **Code comments**: Do not add comments that explain what the code does. The only exception is doc comments on public interface functions/objects, and even those must be extremely concise. Inline comments within function bodies are almost never needed; when they are, they should explain _why_ a particular design was chosen (especially when the reasoning is non-obvious), not _what_ the code does.
- **Avoid scope-creep**: Anything in-scope for this plan should be explicitly agreed upon before starting the implementation. Therefore, when implementing this plan, it is a safe assumption that anything not explicitly mentioned is out of scope.
- **No backwards compatibility / migrations**: Do not add migration code, schema-versioning shims, or deprecation wrappers. Schema changes go straight into the embedded `CREATE TABLE` in `backend/src/db.rs` and assume a `--new` wipe.
```

## Step 3 - Use /grill-with-docs

Use the /grill-with-docs skill to resolve ambiguities and ensure the plan aligns with the user's intent.

## Plan cleanup

As a final step before plan finalization, go through the plan and remove redundancies. Remove low-value fluff that should be assumed by the implementing agent.

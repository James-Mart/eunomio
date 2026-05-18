---
name: manual-mode
description: Restricts the agent to the current branch, leaves all edits unstaged by default with no commits or pushes. Use when the user attaches the manual-mode skill or asks for manual mode workflow.
disable-model-invocation: true
---

# Manual mode

When this skill is attached, follow these constraints:

- Do NOT create a new branch. Continue to work on your existing branch.
- Do NOT commit or push anything unless EXPLICITLY asked by the user. All changes made should be left unstaged.

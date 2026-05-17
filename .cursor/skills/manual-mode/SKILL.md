---
name: manual-mode
description: Restricts the agent to the current branch, leaves all edits unstaged with no commits or pushes, and forbids full repo builds in favor of lint-only verification. Use when the user attaches the manual-mode skill or asks for manual mode workflow.
disable-model-invocation: true
---

# Manual mode

When this skill is attached, follow these constraints:

* Do NOT create a new branch. Continue to work on your existing branch.
* Do NOT commit or push anything. All changes made should be left unstaged.
* Do NOT attempt a build. All code verification should use simple lint checks, not repo builds.

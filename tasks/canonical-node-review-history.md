# Canonical Node Review History

**Status:** Follow-up

## Summary

Canonical Nodes should eventually expose the review history that produced
them. When a Pending Partition is accepted or finished, its useful
Plan, Construct, and Timeline transcripts can be copied or linked
to the canonical target Node so reviewers can inspect the reasoning and
agent work after the Pending Partition row is gone.

## Acceptance criteria

- Accepted Partition transcripts remain available from canonical view.
- Node-owned history distinguishes Plan, Construct, and Timeline
  entries.
- Existing Pending Partition transcript behavior is unchanged.
- Deleting a Session removes its Node review history.

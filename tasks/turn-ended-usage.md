# Emit `turn-ended.usage` from helper subprocess

**Status:** Deferred

## Summary

Ensure the Node helper (`helper/src/run.mjs`) emits `turn-ended.usage` events in the shape the Rust server expects, so partition usage metering receives complete data from Cursor SDK streams.

## Why deferred

Rust already parses `turn-ended` usage from SDK messages inside `sdkMessage` envelopes via `eunomio-helper-protocol::parse_turn_ended_usage`. The remaining work is validation against live SDK streams and helper-side fixes if the envelope shape differs or events are dropped before emission.

## Implementation notes

- Confirm event shape against **live** Cursor SDK streams (not only fixtures).
- Extend `helper/src/run.mjs` if the SDK wraps usage differently or omits fields before the helper forwards messages.
- Align with coordinator `QuotaEnforcer::record_usage` (warn-and-continue on failure — metering must not abort runs).

## Acceptance criteria

- [ ] End-to-end: a completed helper turn produces a `turn-ended.usage` (or equivalent) message the server parses successfully.
- [ ] Usage fields match what `parse_turn_ended_usage` expects (document any mapping in `eunomio-helper-protocol`).
- [ ] Regression test or recorded fixture covers the confirmed SDK shape.

## References

- `eunomio-helper-protocol::parse_turn_ended_usage`
- `helper/src/run.mjs`

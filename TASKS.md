# Deferred tasks

## Swap `eunomio-sqlite` driver from `tokio-rusqlite` to `sqlx`

Why deferred: bundled into a multi-crate split would conflate failure modes; the `Datastore` trait surface is now stable enough to swap drivers without disturbing call sites.

Pre-conditions: do this before any Postgres `eunomio-postgres` crate is written so the two SQLite/Postgres impls share idiom.

Notes: prepare-offline workflow needed for hermetic CI; rewrite `Connection.call(|c| { ... })` blocks to `sqlx::query(...).fetch_one(&pool).await?`.

## Wire `npm run check:license` into CI

When CI infrastructure exists, add a workflow step running `npm run check:license` to guard against FSL header leaks into the OSS repo.

## Emit `turn-ended.usage` from helper subprocess

The Rust side parses `turn-ended` usage from SDK messages inside `sdkMessage` envelopes (`eunomio-helper-protocol::parse_turn_ended_usage`). Confirm against live Cursor SDK streams and extend `helper/src/run.mjs` if the SDK shape differs or events are dropped before emission.

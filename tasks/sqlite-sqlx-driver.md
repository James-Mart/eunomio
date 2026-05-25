# Swap `eunomio-sqlite` driver from `tokio-rusqlite` to `sqlx`

**Status:** Deferred

## Summary

Replace the SQLite access layer in `eunomio-sqlite` with `sqlx` while keeping the `Datastore` trait surface and call sites unchanged.

## Why deferred

Bundling this with the multi-crate workspace split would conflate failure modes: driver migration and crate-boundary refactors would be hard to bisect if something regressed. The `Datastore` trait surface is now stable enough to swap drivers without disturbing call sites.

## Preconditions

- Complete **before** any Postgres `eunomio-postgres` crate is written, so SQLite and Postgres implementations share the same query/connection idioms from the start.

## Implementation notes

- Add a **prepare-offline** workflow for hermetic CI (`sqlx` compile-time query checking).
- Rewrite `Connection.call(|c| { ... })` blocks to async `sqlx` calls, e.g. `sqlx::query(...).fetch_one(&pool).await?`.
- Preserve existing error mapping via `map_sqlite_err` / `AppError` paths in `eunomio-server`.

## Acceptance criteria

- [ ] `eunomio-sqlite` uses `sqlx` with no `tokio-rusqlite` dependency.
- [ ] All existing datastore integration tests pass unchanged.
- [ ] CI can build and test without network access to a live database (offline query data / `SQLX_OFFLINE`).
- [ ] No changes required at `Datastore` trait consumers beyond the sqlite crate internals.

## References

- [ADR 0006](../docs/adr/0006-deployment-trait-seams-and-workspace.md) — defers driver swap until after workspace split; `Datastore` umbrella trait is the stable seam.

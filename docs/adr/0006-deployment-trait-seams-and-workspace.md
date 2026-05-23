# ADR 0006: Deployment trait seams and workspace split

## Status

Accepted

## Context

Hosted deployment requires swapping auth, datastore, keystore, sandbox, and quota implementations without `if hosted` branches in shared code. Local auth and org/user tenancy have shipped in the monolithic `backend/` crate.

## Decision

Split `backend/` into a Cargo workspace of eight crates matching `HOSTED_DEPLOYMENT.md`. Expose five deployment seams as traits. Only the local binary and test crates construct concrete impls (e.g. `eunomio-bin-local`, integration tests with `FakeSubagentRunner`). There is no runtime `DeploymentMode`; the running binary is the deployment shape.

**Datastore** is an umbrella trait exposing per-entity sub-traits via accessor methods (`sessions()`, `nodes()`, …); flat-trait and per-`AppState`-field alternatives were rejected for mockability and grouping.

**AppState** carries `Arc<dyn Datastore>`, `Arc<dyn KeyStore>`, and `Arc<dyn AuthProvider>`. It does **not** carry `SandboxRuntime`, `QuotaEnforcer`, or `SubagentRunner`.

**Coordinator** owns `Arc<dyn SubagentRunner>` and `Arc<dyn QuotaEnforcer>`. `QuotaEnforcer::check_can_start_run` runs before spawning; `record_usage` is called on usage events (warn-and-continue on failure — metering must not abort runs).

**SubagentRunner** (in `eunomio-helper-protocol`) owns helper subprocess spawning. `CursorHelperRunner` holds `Arc<dyn SandboxRuntime>` and calls `sandbox.wrap(...)` before every helper launch. Model listing (`list_models(cursor_api_key)`) is also on this trait so sandbox stays off `AppState`.

**Auth SQL** lives in `eunomio-sqlite`; `eunomio-auth-local` orchestrates cookies and login without SQL. Combined tx-atomic methods (`rotate_with_audit`, `delete_with_audit`) preserve login/logout atomicity.

**AppError** stays in `eunomio-core` without axum; HTTP mapping uses a `ServerError` newtype in `eunomio-server`. SQLite errors map via `map_sqlite_err`.

## Considered options

- **Flat `Datastore` trait** — rejected: ~80 methods on one trait; god-mocks in tests.
- **Per-entity traits as separate `AppState` fields** — rejected with flat trait: same mockability problem at the wiring layer; umbrella + accessors group by entity without exploding `AppState`.
- **Runtime `DeploymentMode` flag** — rejected: trait extraction removes the need; invites `if hosted` branches in shared code.
- **`IntoResponse for AppError` in `eunomio-core`** — rejected despite orphan rules making a sqlite-side `From` look like the obvious fix: any `IntoResponse` impl in core would force every leaf crate to transitively compile axum. `ServerError` newtype in `eunomio-server` solves HTTP mapping without that dependency leak.
- **sqlx driver swap now** — deferred (not rejected) to `TASKS.md`: bundled with the multi-crate split would conflate refactor failure modes; `Datastore` surface is stable enough to swap drivers later.

## Consequences

- Hosted repo can depend on OSS crates and supply its own bin + impl crates.
- Mechanical import churn and workspace build complexity increase short-term.

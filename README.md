# Eunomia

A standalone tool for turning a noisy "ref A → ref B" diff into a clean, reviewable commit history by exploring a graph of synthesized commits.

This is the tracer-bullet MVP: it stands up the whole stack (React/Vite/Tailwind/shadcn UI ↔ Rust axum backend ↔ SQLite ↔ git worktree) implementing only `createSession`, `getGraph`, rename, and `branchFromNode`.

## Layout

```
backend/    Rust axum + rusqlite binary crate `eunomia`
frontend/   Vite + React + TS + Tailwind + shadcn
```

## Dev

```bash
npm install
npm run dev
```

Vite serves the UI on `:5173` (proxying `/api` to `:3001`); the backend runs on `:3001` and uses the `cargo watch` cwd as `REPO_ROOT`.

## Build

```bash
npm run build
```

Produces `backend/target/release/eunomia`, a single binary that serves UI + API on one port. Run it from any git repo to use that repo as `REPO_ROOT`.

```bash
cd /path/to/some/git/repo
/path/to/eunomia/backend/target/release/eunomia serve --port 3001
```

State (SQLite DB + per-session synthesis worktrees) lives in `~/.eunomia/`, shared across every repo a user runs Eunomia against.

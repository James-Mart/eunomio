# Eunomia

Turn a noisy "ref A → ref B" diff into a clean, reviewable commit history by exploring a graph of AI-assisted, human-supervised commits.

## Documentation

User docs: run `npm run dev:docs` and open [http://localhost:3000/docs](http://localhost:3000/docs).

## Development

```bash
npm install
npm run dev
```

Vite serves the UI on `:5173` (proxying `/api` to `:3001`); the backend runs on `:3001`. Open `http://localhost:5173`.

Build the release binary:

```bash
npm run build
# → target/release/eunomia
```

## Maintainer docs

- [`CONTEXT.md`](CONTEXT.md) — canonical terminology (glossary)
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — dev setup, security, implementation model
- [`HOSTED_DEPLOYMENT.md`](HOSTED_DEPLOYMENT.md) — future hosted SaaS design
- [`docs/adr/`](docs/adr/) — architecture decision records

## Layout

```
backend/    Rust axum + SQLite
frontend/   Vite + React + TS + Tailwind
docs/       Fumadocs user documentation site
subagents/  prompt markdown for surveyor/planner/constructor
```

# Eunomio

Turn a noisy "ref A → ref B" diff into a clean, reviewable commit history by exploring a graph of AI-assisted, human-supervised commits.

Licensed under [Apache License 2.0](LICENSE).

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
# → target/release/eunomio
```

Install the binary to `$PATH`:

```bash
cargo install --path crates/eunomio-bin-local --force
```

License header check (run before committing code changes):

```bash
npm run check:license
```

## Maintainer docs

- [`CONTEXT.md`](CONTEXT.md) — canonical terminology (glossary)
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — dev setup, security, implementation model
- [`HOSTED_DEPLOYMENT.md`](HOSTED_DEPLOYMENT.md) — future hosted SaaS design
- [`docs/adr/`](docs/adr/) — architecture decision records

## Layout

```
crates/
  eunomio-core/           domain types + traits
  eunomio-server/         axum handlers, middleware, coordinator
  eunomio-helper-protocol/ cursor-helper wire format
  eunomio-sqlite/         Datastore impl (SQLite)
  eunomio-sandbox-linux/  SandboxRuntime impl (no-op stub today)
  eunomio-auth-local/     AuthProvider impl
  eunomio-keystore-file/  KeyStore impl
  eunomio-bin-local/      local single-binary main()
frontend/                 Vite + React + TS + Tailwind
helper/                   cursor-helper (Node SEA)
docs/                     Fumadocs user documentation site
subagents/                prompt markdown for surveyor/planner/constructor
```

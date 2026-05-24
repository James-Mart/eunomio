---
name: create-remote
description: Start eunomio in dev mode against a target git repo and print the public trycloudflare URL. Use when the user asks to "create a remote", share a dev environment via tunnel, expose eunomio publicly, or start eunomio dev mode on a specific repo.
disable-model-invocation: true
---

# create-remote

Boot eunomio's dev stack (Rust backend + Vite frontend + external tunnel) against an arbitrary
git repo, and surface the public Cloudflare tunnel URL.

## Parameters

- `REPO_ROOT` (required): absolute path to the git repo eunomio should operate
  on. If the user does not supply one, ask before proceeding — do not silently
  default to `$PWD`.

## Preconditions

- Run from the eunomio repo root (`npm run dev` lives in its `package.json`).
- `cloudflared` on `$PATH` or at `~/.eunomio/bin/cloudflared` (if missing, run
  `cargo run -p eunomio-bin-local -- --enable-tunnel` once to download, or install manually).
- Ports `3001` (backend) and `5173` (frontend Vite) must be free. Kill any
  prior `eunomio` / `vite` / `cloudflared` processes before starting.
- `REPO_ROOT` must be a git repo (contains `.git`).

## Procedure

1. Verify preconditions above.

2. Wipe the on-disk eunomio database so the dev stack boots from a clean
   slate:

   ```bash
   rm -f ~/.eunomio/eunomio.db ~/.eunomio/eunomio.db-wal ~/.eunomio/eunomio.db-shm
   ```

   This must happen *after* killing any running eunomio process (see
   preconditions) and *before* `npm run dev` starts the backend — once the
   backend opens the SQLite file it holds it open via WAL, and deleting
   underneath it leaves the running process with a phantom db. Wiping all
   three files (`.db`, `-wal`, `-shm`) matches what the backend's hidden
   `--new` flag does in `crates/eunomio-bin-local/src/main.rs`; deleting only
   `eunomio.db` leaves WAL frames that will be replayed on next open.

   `npm run dev` invokes the backend with no `--data-dir`, so it defaults to
   `~/.eunomio/`. If the user has overridden the data dir elsewhere, wipe
   the corresponding files under that directory instead.

3. Launch dev mode in the background, capturing logs:

   ```bash
   cd /path/to/eunomio
   rm -f /tmp/eunomio-dev.log
   EUNOMIO_REPO_ROOT=<REPO_ROOT> npm run dev > /tmp/eunomio-dev.log 2>&1 &
   ```

   `npm run dev` runs `cloudflared` in a separate process (stable across backend
   rebuilds), the backend with `--allow-dev-url`, and Vite.

   Do *not* append `--new` to the cargo invocation in `package.json`. The
   one-shot `rm` in step 2 is the correct boundary.

4. Poll until the tunnel URL is available (usually within ~10s after
   `cloudflared` starts, longer on a cold cargo build). Prefer, in order:

   ```bash
   cat ~/.eunomio/dev-tunnel.url
   ```

   ```bash
   grep -oE 'https://[a-zA-Z0-9-]+\.trycloudflare\.com' /tmp/eunomio-dev.log | tail -1
   ```

   If nothing appears after ~3 minutes, dump the tail of the log and stop.

5. **Print the URL to the user in the assistant message body.** Minimum:

   ```
   Public tunnel: https://<sub>.trycloudflare.com
   Backend:       http://127.0.0.1:3001
   Frontend:      http://127.0.0.1:5173
   ```

## Stable URL across backend rebuilds

`cargo watch` restarts only the backend. The `[tunnel]` process keeps the same
`*.trycloudflare.com` URL for the whole `npm run dev` session. A new URL appears
only when the dev stack (or `cloudflared`) is restarted.

## Other caveats

- Anyone with the URL has full UI access (no share token in dev). Do not share
  outside trusted channels.
- Quick tunnels may not proxy SSE reliably; live session streams on a phone may
  need polling or a named tunnel later.

## Reference: CLI flags

- `--allow-dev-url`: set by `npm run dev` backend; allows `*.trycloudflare.com`
  API origins. Does not start `cloudflared`.
- `--enable-tunnel`: in-app tunnel (token, mobile UI, spawn at boot). Use once to
  populate `~/.eunomio/bin/cloudflared` if the dev tunnel cannot find a binary.

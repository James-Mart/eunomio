---
name: create-remote
description: Start eunomia in dev mode against a target git repo and print the public trycloudflare URL. Use when the user asks to "create a remote", share a dev environment via tunnel, expose eunomia publicly, or start eunomia dev mode on a specific repo.
disable-model-invocation: true
---

# create-remote

Boot eunomia's dev stack (Rust backend + Vite frontend) against an arbitrary
git repo, and surface the public Cloudflare tunnel URL.

## Parameters

- `REPO_ROOT` (required): absolute path to the git repo eunomia should operate
  on. If the user does not supply one, ask before proceeding — do not silently
  default to `$PWD`.

## Preconditions

- Run from the eunomia repo root (`npm run dev` lives in its `package.json`).
- `cloudflared` must be on `PATH` (check with `which cloudflared`).
- Ports `3001` (backend) and `5173` (frontend Vite) must be free. Kill any
  prior `eunomia` / `vite` / `cloudflared` processes before starting.
- `REPO_ROOT` must be a git repo (contains `.git`).

## Procedure

1. Verify preconditions above.

2. Wipe the on-disk eunomia database so the dev stack boots from a clean
   slate:

   ```bash
   rm -f ~/.eunomia/eunomia.db ~/.eunomia/eunomia.db-wal ~/.eunomia/eunomia.db-shm
   ```

   This must happen *after* killing any running eunomia process (see
   preconditions) and *before* `npm run dev` starts the backend — once the
   backend opens the SQLite file it holds it open via WAL, and deleting
   underneath it leaves the running process with a phantom db. Wiping all
   three files (`.db`, `-wal`, `-shm`) matches what the backend's hidden
   `--new` flag does in `backend/src/main.rs`; deleting only `eunomia.db`
   leaves WAL frames that will be replayed on next open.

   `npm run dev` invokes the backend with no `--data-dir`, so it defaults to
   `~/.eunomia/`. If the user has overridden the data dir elsewhere, wipe
   the corresponding files under that directory instead.

3. Launch dev mode in the background, capturing logs:

   ```bash
   cd /path/to/eunomia
   rm -f /tmp/eunomia-dev.log
   EUNOMIA_REPO_ROOT=<REPO_ROOT> npm run dev > /tmp/eunomia-dev.log 2>&1 &
   ```

   Why this works: `npm run dev` already invokes the backend with
   `--dev-tunnel --start-tunnel`, which auto-starts cloudflared and prints the
   trycloudflare URL to stdout on a single line.

   Do *not* try to bake the wipe into this command by appending `--new` to
   the cargo invocation in `package.json`. `npm run dev` runs the backend
   under `cargo watch`, which respawns the binary on every Rust edit; with
   `--new` baked in, every rebuild would silently wipe the user's sessions
   mid-session. The one-shot `rm` in step 2 is the correct boundary.

4. Poll `/tmp/eunomia-dev.log` until a line matching `https://*.trycloudflare.com`
   appears (usually within ~10s after first compile, longer on a cold cargo
   build). If nothing appears after ~3 minutes, dump the tail of the log and
   stop — something is wrong (missing cloudflared, port conflict, compile
   error, etc.).

5. **Print the URL to the user in the assistant message body.** The whole
   point of this skill is that the user gets a clickable, copyable URL — do
   not bury it inside tool output, a code fence the user has to expand, or a
   summary that paraphrases it away. The minimum acceptable output is a line
   in the chat reply containing the bare URL, e.g.

   ```
   Public tunnel: https://<sub>.trycloudflare.com
   Backend:       http://127.0.0.1:3001
   Frontend:      http://127.0.0.1:5173
   ```

## Why the URL rotates on every backend rebuild

`npm run dev` runs the backend under `cargo watch`, which kills and respawns
the eunomia binary whenever Rust sources change. Each eunomia process spawns
`cloudflared` as a child with `kill_on_drop(true)` (`backend/src/tunnel.rs`,
`spawn_cloudflared`), so the cloudflared process dies with it. On restart,
eunomia invokes `cloudflared tunnel --url http://localhost:5173` with no
named tunnel and no Cloudflare account credentials — that is a TryCloudflare
"Quick Tunnel", which allocates a fresh random `*.trycloudflare.com`
subdomain every invocation. There is no way to pin or reuse the subdomain
without switching to a named tunnel tied to a Cloudflare account.

Practical consequence: any Rust edit invalidates the URL the user is
currently sharing. If they need a stable URL, tell them to either (a) stop
editing backend sources, or (b) run eunomia without `cargo watch` (e.g.
`cargo run -- --port 3001 --no-open --dev-tunnel --start-tunnel` directly).

When you detect that a new URL has been issued (e.g. the user asks again, or
you re-tail after a rebuild), print the new URL in the chat reply the same
way — do not assume the previously-printed URL is still valid.

## Other caveats

- The dev tunnel skips the share-token gate (see `--dev-tunnel` in
  `backend/src/main.rs`) — anyone with the URL has full UI access. Do not
  share it outside trusted channels.
- Logs go to `/tmp/eunomia-dev.log`. To recover the current URL later:
  `grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' /tmp/eunomia-dev.log | tail -1`.

## Reference: relevant CLI flags

Both are hidden from `--help` and set by `npm run dev`'s backend invocation:

- `--dev-tunnel`: route `/api/tunnel` traffic to the Vite dev server and skip
  the share-token gate so HMR works over the public URL.
- `--start-tunnel` (requires `--dev-tunnel`): auto-start cloudflared at boot
  and print the trycloudflare URL to stdout.

# Security

Eunomia is a developer tool that runs on the user's workstation against the user's
own repositories. This file documents the trust model and the known security
boundaries.

## Trust model

The Eunomia HTTP listener binds `127.0.0.1` only. The local OS user that ran
`eunomia` is the trust boundary: any process executing as that user can
already read the source tree, the SQLite state under `~/.eunomia/`, and the
synthesis worktrees, so the API does not add an authentication layer on top.

A host guard middleware on the local listener rejects requests whose `Host`
header (or `Origin` header, when present) is not `127.0.0.1`, `localhost`, or
`[::1]`. This closes two cross-origin attack paths a co-resident browser would
otherwise enable:

- **CSRF from arbitrary sites** the user happens to have open. Submitting a form
  to `http://127.0.0.1:3001/...` only works if the request also carries one of
  the allowed `Host`/`Origin` headers; the browser will not let a script forge
  these.
- **DNS rebinding reads.** A page on `evil.example.com` cannot rebind that
  hostname to `127.0.0.1` and then read the API: the `Host` header would still
  say `evil.example.com` and the request is rejected.

The guard applies to every method, not just state-changing ones, so SSE
subscriptions are protected too.

## Tunnel

When a user enables the Cloudflare quick tunnel (`POST /api/tunnel`), a second
HTTP listener is wrapped in a token-checking middleware and exposed via
`cloudflared`. **The share token grants full admin access to Eunomia** — anyone
holding the URL can view diffs, accept or abandon partitions, change settings,
and trigger API-billing runs. The UI describes the link as such.

Operational notes:

- Rotate the token by `DELETE /api/tunnel` followed by `POST /api/tunnel`.
  Old tokens stop working immediately; in-flight Cloudflare connections do not
  see the new token.
- Tokens appear in Cloudflare edge logs at first hit. Treat them as bearer
  secrets even though they are short-lived.
- The share token is **not** broadcast over `/api/tunnel/events`. SSE
  subscribers see a redacted DTO; the full token is only returned by
  `GET /api/tunnel`, which is reachable only on the host-gated local listener.
- The hidden `--dev-tunnel` flag, set only by `npm run dev`'s backend
  invocation, points cloudflared at the Vite dev server on `:5173` and skips
  the share-token gate entirely. See the "Dev escape hatch" section of
  [`docs/adr/0003-public-url-token-tunnel.md`](docs/adr/0003-public-url-token-tunnel.md).

## Subagents are unsandboxed local processes

Subagents (Surveyor, Planner, Constructor) run via the embedded `cursor-helper`
Node binary. The helper inherits Eunomia's filesystem and network access, so a
malicious or prompt-injected agent can do anything the eunomia process can do —
including read shell history, write outside the synthesis worktree, exfiltrate
secrets, or make outbound network calls. The Constructor in particular
interprets attacker-influenced content (commit messages, diff hunks) under the
soft constraint of [`subagents/constructor.md`](subagents/constructor.md) only.

Mitigation direction: the Cursor SDK supports cloud-hosted agents in addition
to the `local: { cwd }` runtime currently used in
[`helper/src/run.mjs`](helper/src/run.mjs). Switching to the cloud-agent
runtime would move agent execution into Cursor's sandboxed environment and
contain prompt-injection blast radius to that sandbox. This is a future
direction, not implemented today.

## Cloudflared binary

When `cloudflared` is not on `PATH`, Eunomia downloads it from the GitHub
release for a pinned version. The download URL is TLS-protected and the
downloaded asset is SHA-256 verified against a hash embedded in the binary
before it is extracted or executed. A hash mismatch deletes the download and
fails the request with `cloudflared_sha_mismatch`.

The upgrade procedure (bump the version constant and re-pin the five
per-platform hashes) is documented in the header comment of
[`backend/src/tunnel.rs`](backend/src/tunnel.rs).

## Reporting a vulnerability

This project does not currently have a coordinated disclosure process. If you
find something you believe is a real-world risk for users, open a GitHub issue
with enough detail to reproduce.

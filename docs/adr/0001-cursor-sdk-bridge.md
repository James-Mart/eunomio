# Cursor SDK bridge: Node SEA helper invoked as a per-call subprocess

Eunomia is a Rust binary that needs to call `@cursor/sdk` for every interaction with Cursor agents — model listing today, subagent runs (surveyor / planner / constructor) for Partitions later. The bridge has to preserve the single-binary CLI deploy story from `ARCHITECTURE.md` while letting us use the real SDK rather than a hand-rolled REST client.

We compile a small Node helper (`helper/src/cursor.mjs`) plus `@cursor/sdk` into a single self-contained `cursor-helper` executable using `esbuild` (to bundle) and Node's Single Executable Applications feature (to inject the bundle into a copy of the `node` runtime). The resulting binary is embedded into the eunomia executable via `rust-embed` at `cargo build --release` time, extracted to a temp directory on first use, and invoked as a per-call subprocess with subcommand arguments (e.g. `cursor-helper list-models`) and JSON over stdout. Each invocation does one thing and exits; future subagent runs will be additional subcommands on the same binary.

## Considered alternatives

- **Bun `bun build --compile`.** Smaller binary (~50 MB vs ~85 MB) and a single-command compile, but introduces a parallel JS runtime ecosystem alongside the npm/Node toolchain we already use. Rejected because the size win didn't justify the new tool.
- **Wasm Component Model via ComponentizeJS + wasmtime.** Architecturally cleaner — in-process calls, typed WIT interfaces, `wasi:http` for the SDK's HTTPS, ~1-5 ms per call. Rejected for now because `@cursor/sdk` contains local-runtime code paths that use Node APIs (`child_process`) which may fail at componentize time, and every SDK update carries componentize-compatibility risk. Subprocess + JSON over stdio is the most-deployed cross-language bridge pattern and has zero SDK-compatibility risk. Revisit if subprocess overhead becomes a real problem and the ComponentizeJS toolchain matures around npm packages with deep Node-isms.
- **Hit Cursor's REST API directly from Rust.** Avoids the JS runtime entirely and keeps the single-binary deploy story trivially. Rejected because we want to use the SDK's higher-level surface (`agent.send`, `run.stream`, `Agent.resume`) for Partition subagents, and reimplementing those in Rust would couple us to the REST API's evolution.
- **Long-lived helper daemon over JSON-RPC.** Lower per-call latency but worse isolation (one bug kills every in-flight subagent run), harder cancellation, and no real benefit for our call pattern (settings dropdowns load once; subagent runs are minute-scale, where ~30-50 ms of spawn cost is noise).

## Consequences

- Binary size grows by ~85 MB (the embedded `node` runtime plus the bundled SDK). Acceptable for a workstation dev tool.
- Cross-platform release builds need per-target `node` binaries available at build time. Scriptable in `helper/build.mjs`.
- The subprocess contract — subcommand args plus JSON over stdout — is intentionally portable. If we later migrate to the Component Model or to a long-lived daemon, callers in `cursor_bridge.rs` change but the helper's JavaScript code mostly doesn't.

import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, chmodSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { build } from "esbuild";

const here = dirname(fileURLToPath(import.meta.url));
const dist = join(here, "dist");
mkdirSync(dist, { recursive: true });

const bundlePath = join(dist, "bundle.cjs");
await build({
  entryPoints: [join(here, "src/cursor.mjs")],
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  outfile: bundlePath,
  conditions: ["node", "require"],
  mainFields: ["main"],
  resolveExtensions: [".js", ".cjs", ".mjs", ".json"],
  loader: { ".d.ts": "empty", ".map": "empty", ".LICENSE.txt": "empty" },
  alias: { bindings: join(here, "src/bindings-loader.cjs") },
});

// Native bindings can't ride inside the SEA blob (Node SEA only embeds JS, and
// loading a `.node` requires it to live on disk for `process.dlopen`). We copy
// them into `dist/` so they get embedded into the eunomia Rust binary by
// rust-embed and extracted next to `cursor-helper` at runtime. See
// `helper/src/bindings-loader.cjs` for the runtime loader.
const nativeBindings = [
  ["node_modules/sqlite3/build/Release/node_sqlite3.node", "node_sqlite3.node"],
];
for (const [src, dst] of nativeBindings) {
  copyFileSync(join(here, src), join(dist, dst));
}

const blobPath = join(dist, "sea-prep.blob");
if (existsSync(blobPath)) rmSync(blobPath);
execFileSync(
  process.execPath,
  ["--experimental-sea-config", join(here, "sea-config.json")],
  { stdio: "inherit", cwd: here },
);

const outBinary = join(dist, "cursor-helper");
copyFileSync(process.execPath, outBinary);
chmodSync(outBinary, 0o755);

if (process.platform === "darwin") {
  spawnSync("codesign", ["--remove-signature", outBinary], { stdio: "inherit" });
}

const postjectArgs = [
  "postject",
  outBinary,
  "NODE_SEA_BLOB",
  blobPath,
  "--sentinel-fuse",
  "NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2",
];
if (process.platform === "darwin") postjectArgs.push("--macho-segment-name", "NODE_SEA");

const result = spawnSync("npx", postjectArgs, { stdio: "inherit", cwd: here });
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

if (process.platform === "darwin") {
  spawnSync("codesign", ["--sign", "-", outBinary], { stdio: "inherit" });
}

console.log(`built ${outBinary}`);

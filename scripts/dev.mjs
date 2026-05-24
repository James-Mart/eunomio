/* SPDX-License-Identifier: Apache-2.0 */

import concurrently from "concurrently";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const TUNNEL_URL_RE = /https:\/\/[a-zA-Z0-9-]+\.trycloudflare\.com/;
const VITE_DEV_PORT = 5173;

function shellQuote(arg) {
  if (/^[A-Za-z0-9_@%+=:,./-]+$/.test(arg)) return arg;
  return `'${arg.replace(/'/g, `'\\''`)}'`;
}

function dataDir() {
  return path.join(os.homedir(), ".eunomio");
}

function devTunnelUrlFile() {
  return path.join(dataDir(), "dev-tunnel.url");
}

function resolveCloudflaredBinary() {
  const managed = path.join(dataDir(), "bin", "cloudflared");
  if (fs.existsSync(managed)) return managed;
  const found = spawnSync("sh", ["-c", "command -v cloudflared"], {
    encoding: "utf8",
  });
  if (found.status === 0) {
    const p = found.stdout.trim();
    if (p) return p;
  }
  return null;
}

function cloudflaredMissingMessage() {
  return [
    "cloudflared not found.",
    "Install it on PATH, or run once:",
    "  cargo run -p eunomio-bin-local -- --enable-tunnel",
    "to download ~/.eunomio/bin/cloudflared",
  ].join("\n");
}

function removeDevTunnelUrlFile() {
  try {
    fs.unlinkSync(devTunnelUrlFile());
  } catch (err) {
    if (err?.code !== "ENOENT") throw err;
  }
}

function writeDevTunnelUrl(url) {
  const dir = dataDir();
  fs.mkdirSync(dir, { recursive: true });
  const dest = devTunnelUrlFile();
  const tmp = `${dest}.tmp`;
  fs.writeFileSync(tmp, `${url}\n`, "utf8");
  fs.renameSync(tmp, dest);
}

function runDevTunnel() {
  const binary = resolveCloudflaredBinary();
  if (!binary) {
    console.error(cloudflaredMissingMessage());
    process.exit(1);
  }

  removeDevTunnelUrlFile();

  const child = spawn(
    binary,
    ["tunnel", "--no-autoupdate", "--url", `http://localhost:${VITE_DEV_PORT}`],
    { stdio: ["ignore", "pipe", "pipe"] },
  );

  let urlWritten = false;

  const onLine = (line) => {
    process.stdout.write(`[tunnel] ${line}\n`);
    if (urlWritten) return;
    const match = line.match(TUNNEL_URL_RE);
    if (!match) return;
    urlWritten = true;
    const url = match[0];
    writeDevTunnelUrl(url);
    console.log(`[tunnel] ${url}`);
  };

  const attach = (stream) => {
    let buf = "";
    stream.on("data", (chunk) => {
      buf += chunk.toString();
      let idx;
      while ((idx = buf.indexOf("\n")) !== -1) {
        const line = buf.slice(0, idx).replace(/\r$/, "");
        buf = buf.slice(idx + 1);
        if (line) onLine(line);
      }
    });
  };

  attach(child.stdout);
  attach(child.stderr);

  const cleanup = () => {
    removeDevTunnelUrlFile();
  };

  child.on("exit", (code, signal) => {
    cleanup();
    if (signal) process.exit(1);
    process.exit(code ?? 1);
  });

  process.on("SIGINT", () => {
    child.kill("SIGTERM");
  });
  process.on("SIGTERM", () => {
    child.kill("SIGTERM");
  });
}

function runDev() {
  removeDevTunnelUrlFile();

  const extraArgs = process.argv.slice(2);
  const cargoRunArgs = ["--port", "3001", "--allow-dev-url", ...extraArgs];
  const cargoRunTail = cargoRunArgs.map(shellQuote).join(" ");

  const backendCommand =
    `cargo watch -w crates -w subagents -- cargo run -p eunomio-bin-local -- ${cargoRunTail}`;

  const tunnelRunner = `"${process.execPath}" "${fileURLToPath(import.meta.url)}" --tunnel-only`;

  const { result } = concurrently(
    [
      { name: "backend", command: backendCommand, prefixColor: "blue" },
      { name: "frontend", command: "npm run dev:frontend", prefixColor: "green" },
      { name: "tunnel", command: tunnelRunner, prefixColor: "magenta" },
    ],
    { prefix: "name" },
  );

  result
    .catch((code) => {
      process.exit(typeof code === "number" ? code : 1);
    })
    .finally(() => {
      removeDevTunnelUrlFile();
    });
}

if (process.argv.includes("--tunnel-only")) {
  runDevTunnel();
} else {
  runDev();
}

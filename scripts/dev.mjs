/* SPDX-License-Identifier: Apache-2.0 */

import concurrently from "concurrently";

function shellQuote(arg) {
  if (/^[A-Za-z0-9_@%+=:,./-]+$/.test(arg)) return arg;
  return `'${arg.replace(/'/g, `'\\''`)}'`;
}

const extraArgs = process.argv.slice(2);
const cargoRunArgs = ["--port", "3001", "--dev-tunnel", ...extraArgs];
const cargoRunTail = cargoRunArgs.map(shellQuote).join(" ");

const backendCommand =
  `cargo watch -w crates -w subagents -- cargo run -p eunomio-bin-local -- ${cargoRunTail}`;

const { result } = concurrently(
  [
    { name: "backend", command: backendCommand, prefixColor: "blue" },
    { name: "frontend", command: "npm run dev:frontend", prefixColor: "green" },
  ],
  { prefix: "name" },
);

result.catch((code) => {
  process.exit(typeof code === "number" ? code : 1);
});

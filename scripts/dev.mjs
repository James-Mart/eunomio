import concurrently from "concurrently";

function shellQuote(arg) {
  if (/^[A-Za-z0-9_@%+=:,./-]+$/.test(arg)) return arg;
  return `'${arg.replace(/'/g, `'\\''`)}'`;
}

const extraArgs = process.argv.slice(2);
const cargoRunArgs = ["--port", "3001", "--dev-tunnel", ...extraArgs];
const cargoRunTail = cargoRunArgs.map(shellQuote).join(" ");

const backendCommand =
  `cd backend && cargo watch -w src -w Cargo.toml -w ../subagents -- cargo run -- ${cargoRunTail}`;

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

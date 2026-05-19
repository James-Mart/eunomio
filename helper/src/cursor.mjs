import { Cursor } from "@cursor/sdk";

async function listModels() {
  const models = await Cursor.models.list();
  const payload = { models: models.map((m) => ({ id: m.id })) };
  process.stdout.write(JSON.stringify(payload));
}

function fail(code, message) {
  process.stdout.write(JSON.stringify({ error: message, code }));
  process.exit(1);
}

async function main() {
  const sub = process.argv[2];
  if (!sub) fail("usage", "usage: cursor-helper <subcommand>");
  switch (sub) {
    case "list-models":
      await listModels();
      return;
    default:
      fail("usage", `unknown subcommand: ${sub}`);
  }
}

main().catch((err) => {
  const message = err instanceof Error ? err.message : String(err);
  fail("cursor_sdk_unavailable", message);
});

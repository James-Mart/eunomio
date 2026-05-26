/* SPDX-License-Identifier: Apache-2.0 */

import { Cursor } from "@cursor/sdk";
import { run } from "./run.mjs";

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  const buf = Buffer.concat(chunks);
  return buf.toString("utf8");
}

async function listModels() {
  let request;
  try {
    const raw = await readStdin();
    request = JSON.parse(raw);
  } catch (err) {
    fail("bad_request", err instanceof Error ? err.message : String(err));
  }

  const { cursorApiKey } = request;
  delete process.env.CURSOR_API_KEY;

  if (typeof cursorApiKey !== "string" || cursorApiKey.length === 0) {
    fail("bad_request", "cursorApiKey required");
  }

  const models = await Cursor.models.list({ apiKey: cursorApiKey });
  const payload = { models: models.map(mapCatalogModel) };
  process.stdout.write(JSON.stringify(payload));
}

function mapCatalogModel(m) {
  const out = { id: m.id };
  if (m.displayName != null) out.displayName = m.displayName;
  if (m.description != null) out.description = m.description;
  if (Array.isArray(m.aliases) && m.aliases.length > 0) out.aliases = m.aliases;
  if (Array.isArray(m.parameters) && m.parameters.length > 0) {
    out.parameters = m.parameters.map((p) => {
      const def = { id: p.id, values: (p.values ?? []).map((v) => mapParamValueOption(v)) };
      if (p.displayName != null) def.displayName = p.displayName;
      return def;
    });
  }
  if (Array.isArray(m.variants) && m.variants.length > 0) {
    out.variants = m.variants.map((v) => {
      const variant = {
        params: (v.params ?? []).map((p) => ({ id: p.id, value: String(p.value) })),
      };
      if (v.displayName != null) variant.displayName = v.displayName;
      if (v.description != null) variant.description = v.description;
      if (v.isDefault != null) variant.isDefault = v.isDefault;
      return variant;
    });
  }
  return out;
}

function mapParamValueOption(v) {
  const opt = { value: String(v.value) };
  if (v.displayName != null) opt.displayName = v.displayName;
  return opt;
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
    case "run":
      await run();
      return;
    default:
      fail("usage", `unknown subcommand: ${sub}`);
  }
}

main().catch((err) => {
  const message = err instanceof Error ? err.message : String(err);
  fail("cursor_sdk_unavailable", message);
});

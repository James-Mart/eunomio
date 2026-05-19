// Drop-in replacement for the `bindings` npm package.
//
// `sqlite3`'s lib/sqlite3-binding.js does `require('bindings')('node_sqlite3.node')`.
// Inside our Node SEA helper we can't keep .node files at one of the paths the real
// `bindings` package searches, so esbuild's alias points here instead. The .node file
// is shipped alongside the helper executable (extracted to the same directory by the
// Rust host), and we dlopen it from there.

const os = require("node:os");
const path = require("node:path");

const dlopened = new Map();

function bindings(opts) {
  const requested = typeof opts === "string" ? opts : opts.bindings;
  const name = requested.endsWith(".node") ? requested : `${requested}.node`;
  const cached = dlopened.get(name);
  if (cached) return cached;
  const file = path.join(path.dirname(process.execPath), name);
  const m = { exports: {} };
  process.dlopen(m, file, os.constants.dlopen.RTLD_NOW);
  dlopened.set(name, m.exports);
  return m.exports;
}

module.exports = bindings;
module.exports.default = bindings;

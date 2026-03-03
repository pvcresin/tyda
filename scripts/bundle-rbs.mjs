// Bundle vendor/rbs/{core,stdlib} into a single JSON (path -> content) the
// browser fetches and mounts into the wasm WASI virtual fs at /rbs.
//
//   node scripts/bundle-rbs.mjs vendor/rbs web/rbs-bundle.json
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.argv[2] ?? "vendor/rbs";
const out = process.argv[3] ?? "web/rbs-bundle.json";

const bundle = {};
function walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p);
    else if (name.endsWith(".rbs")) {
      // keys look like "core/array.rbs", "stdlib/securerandom/0/securerandom.rbs"
      bundle[relative(root, p)] = readFileSync(p, "utf8");
    }
  }
}
for (const sub of ["core", "stdlib"]) walk(join(root, sub));

writeFileSync(out, JSON.stringify(bundle));
const bytes = Buffer.byteLength(JSON.stringify(bundle));
console.log(
  `bundled ${Object.keys(bundle).length} rbs files -> ${out} (${(bytes / 1e6).toFixed(1)} MB)`,
);

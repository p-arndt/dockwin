// Stamp a version string into every dockwin manifest.
//
//   node scripts/set-version.mjs 0.2.0
//
// Updates package.json, src-tauri/tauri.conf.json, and the three crate
// Cargo.tomls. Uses targeted regex replaces (not JSON.parse round-tripping) so
// existing formatting, key order, and comments are left untouched. Cargo.lock
// refreshes itself on the next build since the workspace crates are path deps.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+/.test(version)) {
  console.error("usage: node scripts/set-version.mjs <version>   (e.g. 0.2.0)");
  process.exit(1);
}

// Repo root is one level up from this script's scripts/ directory.
const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Each target: the file, the pattern to match, and its replacement.
// `$1` keeps the matched key/prefix so only the value changes.
const targets = [
  // JSON: the top-level "version": "..." key only.
  { file: "package.json",            re: /("version":\s*)"[^"]*"/,    sub: `$1"${version}"` },
  { file: "src-tauri/tauri.conf.json", re: /("version":\s*)"[^"]*"/,  sub: `$1"${version}"` },
  // Cargo.toml: the [package] version (anchored at line start, multiline),
  // never the inline dependency `version = "..."` entries.
  { file: "crates/dockwin-cli/Cargo.toml",  re: /^version = "[^"]*"/m, sub: `version = "${version}"` },
  { file: "crates/dockwin-core/Cargo.toml", re: /^version = "[^"]*"/m, sub: `version = "${version}"` },
  { file: "src-tauri/Cargo.toml",           re: /^version = "[^"]*"/m, sub: `version = "${version}"` },
];

for (const { file, re, sub } of targets) {
  const path = join(root, file);
  const before = readFileSync(path, "utf8");
  const after = before.replace(re, sub);
  if (after === before) {
    console.error(`! no version match in ${file} — pattern may be stale`);
    process.exit(1);
  }
  writeFileSync(path, after);
  console.log(`  ${file}`);
}

console.log(`Stamped version ${version}.`);

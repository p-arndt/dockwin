// Generate the `latest.json` manifest the in-app updater polls.
//
//   TAG=v0.2.0 node scripts/make-latest-json.mjs
//
// Reads the NSIS installer + its detached signature produced by `tauri build`
// (with bundle.createUpdaterArtifacts=true) and writes dist-release/latest.json
// pointing the download URL at this release's GitHub asset. The updater endpoint
// in src-tauri/tauri.conf.json resolves `releases/latest/download/latest.json`
// to this file.
//
// If the .sig is absent (signing secrets weren't configured), it warns and
// exits 0 without writing a manifest, so an unsigned release still publishes —
// the in-app updater simply won't see an update for it.

import {
  readFileSync,
  writeFileSync,
  readdirSync,
  mkdirSync,
  existsSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// owner/repo the release assets live under (mirrors tauri.conf.json endpoints).
const REPO = "dockwin/dockwin";
// tauri's platform key for a 64-bit Windows target.
const PLATFORM = "windows-x86_64";

const version = JSON.parse(
  readFileSync(join(root, "package.json"), "utf8")
).version;
// Prefer the pushed tag; fall back to a v-prefixed version.
const tag = process.env.TAG || `v${version}`;

const nsisDir = join(root, "target", "release", "bundle", "nsis");
if (!existsSync(nsisDir)) {
  console.error(`make-latest-json: no NSIS bundle dir at ${nsisDir}`);
  process.exit(1);
}

const setup = readdirSync(nsisDir).find((f) => f.endsWith("-setup.exe"));
if (!setup) {
  console.error(`make-latest-json: no *-setup.exe found in ${nsisDir}`);
  process.exit(1);
}

const sigPath = join(nsisDir, `${setup}.sig`);
if (!existsSync(sigPath)) {
  console.warn(
    `make-latest-json: ${setup}.sig not found — release is unsigned; ` +
      `skipping latest.json (set TAURI_SIGNING_PRIVATE_KEY to enable updates).`
  );
  process.exit(0);
}

const signature = readFileSync(sigPath, "utf8").trim();
const url = `https://github.com/${REPO}/releases/download/${tag}/${encodeURIComponent(
  setup
)}`;

const manifest = {
  version,
  notes: `See the release notes for ${tag}.`,
  pub_date: new Date().toISOString(),
  platforms: {
    [PLATFORM]: { signature, url },
  },
};

const outDir = join(root, "dist-release");
mkdirSync(outDir, { recursive: true });
const outPath = join(outDir, "latest.json");
writeFileSync(outPath, JSON.stringify(manifest, null, 2));
console.log(`make-latest-json: wrote ${outPath}`);
console.log(`  version=${version} tag=${tag}`);
console.log(`  url=${url}`);

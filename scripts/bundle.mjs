// Build the release GUI + NSIS installer, signing the updater artifacts when a
// local signing key is available. Invoked by `just installer` / `just bundle`.
//
// Updater signing: if %USERPROFILE%\.dockwin\dockwin-updater.key exists, its
// contents are passed as TAURI_SIGNING_PRIVATE_KEY so `tauri build` SIGNS the
// installer (emitting the `.sig` the in-app updater verifies). Without it, the
// build runs with updater artifacts disabled (via the merge-override config
// src-tauri/tauri.no-updater.conf.json) so a key-free local bundle still
// succeeds. CI signs via the TAURI_SIGNING_PRIVATE_KEY repo secret instead and
// never runs this script (it calls `pnpm tauri build` directly).
import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const keyPath = join(homedir(), ".dockwin", "dockwin-updater.key");

const env = { ...process.env };
const args = ["tauri", "build"];

if (existsSync(keyPath)) {
  console.log(`bundle: signing installer with ${keyPath}`);
  env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyPath, "utf8").trim();
  // The generated key has no password; respect an explicit one if already set.
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";
} else {
  console.log(
    `bundle: no signing key at ${keyPath} — building unsigned (updater artifacts off).`
  );
  // A path (not inline JSON) so there are no shell-quoting pitfalls.
  args.push("--config", "src-tauri/tauri.no-updater.conf.json");
}

// pnpm is a .cmd on Windows, so spawn through a shell.
const r = spawnSync("pnpm", args, { stdio: "inherit", env, shell: true, cwd: root });
process.exit(r.status ?? 1);

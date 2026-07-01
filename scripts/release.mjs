// Cut a release: bump the version, stamp it everywhere, commit, tag, and push.
// Pushing the tag triggers the "Build and Publish Release" GitHub Action.
//
//   node scripts/release.mjs           # patch bump (default)
//   node scripts/release.mjs minor
//   node scripts/release.mjs major
//   node scripts/release.mjs 1.4.0     # explicit version
//
// Safety: refuses to run on a dirty working tree (so the release commit holds
// only the version bump) and refuses to clobber an existing tag.

import { execSync } from "node:child_process";
import { readVersion, resolveVersion, setVersion } from "./set-version.mjs";

const git = (args, opts = {}) =>
  execSync(`git ${args}`, { encoding: "utf8", ...opts }).trim();

function fail(msg) {
  console.error(`error: ${msg}`);
  process.exit(1);
}

const bump = process.argv[2] ?? "patch";
const next = resolveVersion(bump);
const tag = `v${next}`;

// 1. Clean tree — the release commit must contain only the version bump.
if (git("status --porcelain")) {
  fail("working tree is not clean — commit or stash your changes first.");
}

// 2. Don't reuse an existing tag.
if (git("tag --list").split(/\r?\n/).includes(tag)) {
  fail(`tag ${tag} already exists.`);
}

console.log(`Releasing ${tag}  (${readVersion()} -> ${next})\n`);

// 3. Stamp all manifests.
setVersion(next);

// 4. Refresh Cargo.lock so the new workspace versions land in the release commit.
//    `--workspace` only re-pins the workspace members (path deps); other
//    dependencies are left untouched. Without this the lock keeps the previous
//    versions and has to be fixed up in a follow-up commit.
console.log("\nRefreshing Cargo.lock ...");
execSync("cargo update --workspace", { stdio: "inherit" });

// 5. Commit + annotated tag.
git("add -A");
git(`commit -m "release: ${tag}"`);
git(`tag -a ${tag} -m "${tag}"`);

// 6. Push the current branch together with the new tag.
const branch = git("rev-parse --abbrev-ref HEAD");
console.log(`\nPushing ${branch} + ${tag} ...`);
execSync(`git push origin ${branch} --follow-tags`, { stdio: "inherit" });

console.log(
  `\nDone. ${tag} pushed — the "Build and Publish Release" workflow is now running.`,
);

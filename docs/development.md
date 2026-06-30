# Development & release workflow

dockwin uses [`just`](https://github.com/casey/just) as its task runner — every
common command (dev, build, lint, engine control, release) is a recipe in the
[`justfile`](../justfile). This keeps the raw `pnpm` / `cargo` / `git`
invocations in one place instead of scattered across docs.

```powershell
winget install Casey.Just   # or: cargo install just
just                        # list every recipe
```

> The version-stamping and release orchestration are small Node scripts under
> `scripts/` (`set-version.mjs`, `release.mjs`); `just` just calls them. They run
> with the Node that already ships with the frontend toolchain.

---

## Everyday recipes

### Setup

| Recipe | What it does |
| --- | --- |
| `just install` | `pnpm install` — frontend deps (run once / after `package.json` changes). |

### Dev

| Recipe | What it does |
| --- | --- |
| `just dev` | GUI in dev mode (`pnpm tauri dev`) — hot-reloads the frontend, builds the Rust side. |
| `just dev-web` | Vite frontend only, no Tauri shell. |
| `just cli <args>` | Run the CLI from source, e.g. `just cli status`. |

### Build

| Recipe | Output |
| --- | --- |
| `just build` / `just build-release` | Whole workspace (core + CLI + GUI). |
| `just build-cli` / `just build-cli-release` | Just `dockwin.exe` → `target/{debug,release}/`. |
| `just build-web` | Frontend bundle → `dist/`. |
| `just installer` (alias `bundle`) | Release GUI + NSIS installer → `target/release/bundle/nsis/`. |
| `just sidecar` | Stage the release CLI as the Tauri sidecar binary. |

### Quality

| Recipe | What it does |
| --- | --- |
| `just check` | Type-check the Svelte/TS frontend. |
| `just check-rs` | `cargo check` the workspace. |
| `just clippy` | Clippy with warnings denied. |
| `just fmt` / `just fmt-check` | Format Rust / verify formatting. |
| `just test` | `cargo test --workspace`. |
| `just ci` | Everything CI runs: `fmt-check clippy test check`. |

Run `just ci` before opening a PR.

### Engine (drives the `dockwin` CLI against the WSL2 distro)

| Recipe | What it does |
| --- | --- |
| `just status` | Is the distro registered/running and dockerd reachable? |
| `just engine-install [args]` | Provision the dedicated `dockwin` WSL2 distro. |
| `just engine-start` | Boot the distro and bring dockerd up. |
| `just engine-stop [args]` | Stop dockerd (`--terminate` to release the distro's RAM). |
| `just engine-uninstall [args]` | Tear down (`--backup` to export a `.tar` first). |
| `just example-up` / `just example-down` | Bring the `example/compose.yml` stack up/down. |

### Housekeeping

| Recipe | What it does |
| --- | --- |
| `just clean` | Remove Rust build artifacts and `dist/`. |
| `just clean-all` | Also remove `node_modules` (full reset; re-run `just install`). |

---

## Versioning

The version lives in **five** manifests, kept in sync by one script:

- `package.json`
- `src-tauri/tauri.conf.json`
- `crates/dockwin-cli/Cargo.toml`
- `crates/dockwin-core/Cargo.toml`
- `src-tauri/Cargo.toml`

```powershell
just version              # print the current version
just set-version patch    # 0.1.3 -> 0.1.4   (also: minor, major)
just set-version 0.2.0    # set an explicit version
```

`set-version` only stamps the files (targeted regex replaces — formatting and
comments are preserved). `Cargo.lock` refreshes itself on the next build because
the workspace crates are path dependencies.

---

## Cutting a release

```powershell
just release          # patch bump (default)
just release minor    # or: major, or an explicit version like  just release 1.0.0
```

`just release` runs `scripts/release.mjs`, which:

1. computes the next version (bump keyword or explicit `x.y.z`) → tag `vX.Y.Z`;
2. **refuses to run on a dirty working tree** (so the release commit contains
   only the version bump) and **refuses to reuse an existing tag**;
3. stamps all five manifests;
4. commits (`release: vX.Y.Z`) and creates an annotated tag;
5. pushes the current branch **and** the tag.

Pushing the tag triggers the **Build and Publish Release** GitHub Action
([`.github/workflows/release.yml`](../.github/workflows/release.yml)), which on a
Windows runner:

- builds the CLI as a **single statically-linked** `dockwin.exe`
  (`-C target-feature=+crt-static`, no vcruntime DLL dependency);
- builds the GUI + NSIS installer (`pnpm tauri build`);
- generates release notes with `git-cliff` and publishes a GitHub Release with
  both artifacts attached.

So a typical release is just: land your changes, ensure the tree is clean, then
`just release minor`. Everything else happens in CI.

> **Tag prefix:** tags are `vX.Y.Z` (with the `v`); the manifests stay `X.Y.Z`.
> The workflow trigger (`tags: ['*']`) matches them.

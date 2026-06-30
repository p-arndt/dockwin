# dockwin — task runner
#
# Install `just`:  winget install Casey.Just   (or  cargo install just)
# List recipes:    just            (or  just --list)
#
# Layout:
#   crates/dockwin-core  — shared WSL engine lifecycle + provisioning (the brain)
#   crates/dockwin-cli   — the `dockwin` CLI binary        (-> target/<profile>/dockwin.exe)
#   src-tauri            — the Tauri v2 GUI (dockwin-gui)
#   src/                 — Svelte 5 + TS + Tailwind v4 frontend

# Run recipes through PowerShell on Windows so multi-line bodies and env work.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Default: show the recipe list.
default:
    @just --list

# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------

# Install frontend deps (run once, or after package.json changes).
install:
    pnpm install

# Alias for `install`.
deps: install

# ---------------------------------------------------------------------------
# Dev
# ---------------------------------------------------------------------------

# Run the GUI in dev mode (hot-reloads the frontend, builds the Rust side).
dev:
    pnpm tauri dev

# Run only the Vite frontend dev server (no Tauri shell).
dev-web:
    pnpm dev

# Run the CLI from source, passing through any args:  just cli status
cli *ARGS:
    cargo run -p dockwin-cli -- {{ARGS}}

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

# Build everything (core + CLI + GUI) in debug.
build:
    cargo build --workspace

# Build the whole workspace in release.
build-release:
    cargo build --workspace --release

# Build just the CLI (debug) -> target/debug/dockwin.exe
build-cli:
    cargo build -p dockwin-cli

# Build just the CLI (release) -> target/release/dockwin.exe
build-cli-release:
    cargo build -p dockwin-cli --release

# Build the frontend production bundle (-> dist/).
build-web:
    pnpm build

# Build the release GUI + NSIS installer (-> target/release/bundle/nsis/).
# `pnpm tauri build` runs `pnpm build` + `pnpm sidecar` first via beforeBuildCommand.
installer:
    pnpm tauri build

# Alias for `installer`.
bundle: installer

# Build the release CLI and stage it as the Tauri sidecar binary.
sidecar:
    pnpm sidecar

# ---------------------------------------------------------------------------
# Quality
# ---------------------------------------------------------------------------

# Type-check the Svelte/TS frontend.
check:
    pnpm check

# Cargo type-check the whole workspace (faster than a full build).
check-rs:
    cargo check --workspace

# Lint the whole workspace with clippy (warnings as errors).
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Format all Rust code.
fmt:
    cargo fmt --all

# Verify Rust formatting without writing changes.
fmt-check:
    cargo fmt --all -- --check

# Run the Rust test suite.
test:
    cargo test --workspace

# Run every check the way CI should: format, clippy, tests, frontend check.
ci: fmt-check clippy test check

# ---------------------------------------------------------------------------
# Engine (drives the dockwin CLI against the WSL2 distro)
# ---------------------------------------------------------------------------

# Show whether the dockwin distro is registered / running and dockerd reachable.
status:
    cargo run -p dockwin-cli -- status

# Provision the dedicated 'dockwin' WSL2 distro and install dockerd.
engine-install *ARGS:
    cargo run -p dockwin-cli -- install {{ARGS}}

# Boot the distro and bring dockerd up.
engine-start:
    cargo run -p dockwin-cli -- start

# Stop dockerd (pass `--terminate` to release the distro's RAM).
engine-stop *ARGS:
    cargo run -p dockwin-cli -- stop {{ARGS}}

# Tear down the distro (`wsl --unregister`). Pass `--backup` to export a .tar first.
engine-uninstall *ARGS:
    cargo run -p dockwin-cli -- uninstall {{ARGS}}

# Bring up the example compose stack against the dockwin engine.
example-up:
    cargo run -p dockwin-cli -- up -f example/compose.yml

# Tear down the example compose stack.
example-down:
    cargo run -p dockwin-cli -- down -f example/compose.yml

# ---------------------------------------------------------------------------
# Release
# ---------------------------------------------------------------------------

# Print the current version (read from package.json).
version:
    @(Get-Content package.json -Raw | ConvertFrom-Json).version

# Stamp a version into every manifest (package.json, tauri.conf.json, the three
# crate Cargo.tomls). Accepts a bump keyword or an explicit version; Cargo.lock
# refreshes on the next build (workspace members are path deps). Examples:
#   just set-version patch        just set-version 0.2.0
set-version BUMP="patch":
    node scripts/set-version.mjs {{BUMP}}

# Cut a release: bump the version (patch|minor|major, or an explicit x.y.z),
# stamp all manifests, commit, tag, and push -> triggers the release workflow
# which builds the static CLI + installer. Examples:
#   just release            just release minor            just release 1.0.0
release BUMP="patch":
    node scripts/release.mjs {{BUMP}}

# Submit/update the winget manifest for VERSION pointing at the released
# installer URL (needs wingetcreate + a published GitHub release asset).
# The package id is a placeholder until the manifest is first accepted.
winget VERSION URL:
    wingetcreate update Dockwin.Dockwin --version {{VERSION}} --urls "{{URL}}" --submit

# ---------------------------------------------------------------------------
# Housekeeping
# ---------------------------------------------------------------------------

# Remove Rust build artifacts and the frontend bundle.
clean:
    cargo clean
    -Remove-Item -Recurse -Force dist -ErrorAction SilentlyContinue

# Nuke build artifacts AND node_modules (full reset; re-run `just install` after).
clean-all: clean
    -Remove-Item -Recurse -Force node_modules -ErrorAction SilentlyContinue

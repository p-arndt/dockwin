# Shipping dockwin without raw PowerShell scripts

**Status:** decision document / RFC
**Scope:** how to provision + manage the dockwin engine without asking users to
run `scripts/*.ps1` directly.
**Date:** 2026-06 (factual claims verified against 2025/2026 sources — see
[References](#references)).

---

## Problem

Today provisioning is four PowerShell scripts (`Install-Dockwin.ps1`,
`Uninstall-Dockwin.ps1`, `Start-Dockwin.ps1`, `Stop-Dockwin.ps1`). They work, but
shipping `.ps1` as the primary install surface is unpleasant: users hit
ExecutionPolicy prompts, "are you sure you want to run this script" SmartScreen-ish
friction, no Start-Menu entry, no uninstall registration, and it *feels* like a
hack rather than a product. We want a "real executable."

The good news: the architecture already has a Rust core that shells out to
`wsl.exe`. The scripts contain **no Windows-specific magic** — they are just
orchestrated `wsl.exe` / `docker.exe` calls plus a gzip decompress and a couple
of file copies. All of that ports cleanly to Rust. So "no PowerShell" does **not**
mean "rewrite everything"; it means *move the orchestration into native code we
already need to write for the GUI anyway.*

### What actually needs privileges (important, drives everything below)

| Operation | Needs admin/elevation? |
| --- | --- |
| `wsl --install` (enabling the WSL *Windows feature* the first time, installing the kernel) | **Yes** — Windows shows a UAC elevation prompt; this is expected behaviour. |
| `wsl --update` | Effectively yes on most builds (it updates the MSIX/kernel). |
| `wsl --import` / `--unregister` / `--export` / running commands in a distro | **No** — runs as the normal user once the WSL feature exists. |
| Everything `provision-inside.sh` does (apt, dockerd, systemd) | Runs as **root inside the distro**, which is *not* Windows admin. |
| `docker context create` | No. |

**Conclusion:** the *only* step that fundamentally needs Windows Administrator is
the one-time `wsl --install` to turn the WSL feature on. The dockwin-specific
provisioning (import, configure, install dockerd, wire context) needs **no
elevation at all**. So whatever we ship should *detect* a missing/old WSL and
guide the user to run the one elevated command, but should not itself demand to
run elevated for normal operation.

---

## Options

### (a) Standalone native Rust `dockwin.exe` CLI — **RECOMMENDED**

Port the four scripts into a single `clap`-based binary with subcommands
`status / install / start / stop / uninstall` (prototype delivered in
`crates/dockwin-cli/`). It makes the exact same `wsl.exe` / `docker.exe` calls,
decodes wsl's UTF-16LE output correctly, decompresses the rootfs with the pure-Rust
`flate2`, and reads the unchanged `distro/wsl.conf` + `distro/provision-inside.sh`
assets.

**Pros**
- A genuine `.exe`. No ExecutionPolicy, no `.ps1` mental tax. Tab-completable,
  `--help`, exit codes, pipeable.
- It is the *same code path the GUI needs* — see [Migration path](#migration-path).
  One provisioning implementation, two front-ends (CLI + GUI), zero PowerShell.
- Tiny and permissive: deps are `clap` + `anyhow` + `flate2` (all MIT/Apache,
  no C, no OpenSSL). Release profile is `opt-level=z` + `lto` + `strip`; expect a
  ~1–2 MB single static `.exe`.
- No admin needed for the common path (matches the privilege table above).
- Cross-distributable via winget as a portable/zip package (see (b)).

**Cons**
- A bare `.exe` alone gives no Start-Menu shortcut / Add-Remove-Programs entry.
  (Solved by wrapping it in an installer — option (b) — which is additive, not an
  alternative.)
- An unsigned `.exe` downloaded from the internet still triggers SmartScreen
  "Unknown publisher" until it earns reputation (see [Signing](#code-signing--smartscreen-reality)).
  Note a raw `.ps1` has the *same or worse* trust story, so this is not a regression.

### (b) Bundle the CLI (+ GUI) into an NSIS or WiX/MSI installer

Tauri's own bundler already produces Windows installers: **NSIS** (`-setup.exe`)
and **WiX Toolset v3 MSI** (`.msi`). You choose via
`bundle.targets` / `bundle.windows` in `tauri.conf.json` (e.g.
`"targets": ["nsis"]`). Tauri downloads the NSIS toolchain itself; WiX/MSI can
only be *built* on Windows, whereas NSIS can be built cross-platform. Neither is a
hard "default" — both are first-class — but **NSIS is the better fit here**: it
produces a friendlier per-user `-setup.exe`, supports per-user (non-admin)
installs, and is what most Tauri apps ship.

The dockwin CLI rides along as a **Tauri sidecar / external binary**: drop
`dockwin.exe` in `src-tauri/binaries/` and list it under `bundle.externalBin`
(needs the `-$TARGET_TRIPLE` suffix, e.g.
`dockwin-x86_64-pc-windows-msvc.exe`). The installer then lays down the GUI *and*
the CLI together, adds Start-Menu/uninstall entries, and the GUI can invoke the
sidecar for provisioning. The `distro/` assets (`wsl.conf`,
`provision-inside.sh`) ship as bundle **resources**.

**Pros**
- Real product feel: Start-Menu shortcut, Add/Remove Programs, versioned upgrades.
- One download installs GUI + CLI + assets.
- Reuses tooling dockwin already has (it's a Tauri app); no separate installer
  project to maintain.

**Cons**
- Slightly more build pipeline (sidecar naming, codesign step).
- MSI/WiX is heavier and enterprise-flavoured; only needed if orgs demand MSI for
  GPO deployment. Default to NSIS, optionally also emit MSI for enterprise.
- Installer `.exe`/`.msi` is itself subject to SmartScreen reputation.

### (c) In-GUI first-run Setup wizard (Tauri)

The GUI detects "engine not provisioned" on launch and runs the *same Rust
provisioning code* (the future `dockwin-core` module) behind a friendly wizard
(progress bar over the apt/dockerd install, a clear "WSL needs updating — click to
run the elevated command" step).

**Pros**
- Best UX for non-CLI users; no terminal at all.
- Zero extra binary — it's the GUI calling shared core logic.
- Can surface the one elevation moment (`wsl --install`) as a single explicit,
  explained UAC prompt instead of a scary script.

**Cons**
- Only helps GUI users; headless/CI/scripted setups still want the CLI. So this
  is *complementary* to (a), not a replacement.
- Long-running apt install inside a GUI needs good progress/streaming UX (the
  core should stream provision output to the front-end).

### (d) Keep `.ps1`, wrap with a tiny launcher `.exe` — **weak, not recommended**

Ship a stub `.exe` that just calls
`powershell -ExecutionPolicy Bypass -File Install-Dockwin.ps1`.

**Why it's weak**
- It's lipstick: the `.ps1` (and all its quirks) still ships and still runs; you've
  added a layer, not removed the thing the user disliked.
- `-ExecutionPolicy Bypass` from an `.exe` is exactly the pattern security tooling
  and AV flag; worse trust story, not better.
- Still no shared implementation with the GUI — the logic stays duplicated in
  PowerShell forever.
- Two artifacts to sign and ship instead of one.
- The *only* upside (don't rewrite the scripts) evaporates the moment the GUI needs
  the same logic in Rust anyway — which it does.

---

## Recommendation

**Adopt (a) as the foundation, package it with (b/NSIS), and expose (c) as the
GUI's friendly front-end — all three sharing ONE Rust provisioning implementation.**

Concretely:

1. **Port the scripts to Rust now** as `crates/dockwin-cli` (done — prototype
   compiles). This is the "real executable" the user wants and kills the `.ps1`
   primary surface immediately.
2. **Lift the provisioning logic into `dockwin-core`** (a `provision` module) so
   the CLI is a thin `clap` shell over it. The GUI links the same crate.
3. **Bundle with Tauri's NSIS bundler**, shipping `dockwin.exe` as an
   `externalBin` sidecar and `distro/*` as resources → Start-Menu + uninstall
   entry, GUI + CLI in one install.
4. **GUI first-run wizard** calls the shared core for a no-terminal experience;
   the CLI covers headless/CI/power users.
5. Keep the `.ps1` scripts in-repo only as a temporary fallback / reference, and
   delete them once the CLI reaches parity (it nearly has).

This keeps the anti-bloat thesis intact (one small Rust workspace, ~1–2 MB CLI,
no background service, no telemetry) and stays MIT/Apache top to bottom.

---

## Migration path — one provisioning implementation, no duplication

The risk to avoid is logic living in *both* PowerShell and Rust (and later in both
CLI and GUI). Target shape:

```
crates/
  dockwin-core/        # the brain (lib) — provisioning lives here
    src/provision.rs   #   install/start/stop/status/uninstall as pub fns
    src/wsl.rs         #   wsl.exe helpers + UTF-16LE decode (already in the prototype)
  dockwin-cli/         # thin clap shell -> calls dockwin_core::provision::*
src-tauri/             # GUI -> Tauri commands -> call dockwin_core::provision::*
distro/                # wsl.conf + provision-inside.sh: SINGLE source of truth,
                       #   read by core (or include_str!-embedded for a standalone exe)
```

- The prototype already isolates the two layers: `wsl.rs` (reusable helpers) and
  `ops.rs` (the operations). Promoting `wsl.rs` + `ops.rs` into `dockwin-core`
  and re-pointing the CLI/GUI at them is a near-mechanical move.
- The in-distro shell assets stay in `distro/` as the single source of truth. The
  prototype reads them from disk; the bundled build can `include_str!` them (or
  ship them as Tauri resources) so a standalone `.exe` carries them too.
- **Do not** introduce a workspace `Cargo.toml` until `src-tauri` is ready to join
  it — that would change how `src-tauri` builds. The prototype is deliberately its
  own workspace root (`[workspace]` in its `Cargo.toml`) so it builds independently
  today and can be folded into a real workspace later.

---

## Code-signing / SmartScreen reality

Be honest with ourselves here — this applies to **any** artifact we ship (`.exe`,
`.msi`, *or* `.ps1`):

- **Unsigned** binaries downloaded from the internet get the Microsoft Defender
  SmartScreen "Windows protected your PC / Unknown publisher" blue dialog until the
  file accrues download reputation. A `.ps1` is no better and arguably worse.
- **OV code-signing certificate** (~$200–400/yr): removes the "Unknown publisher"
  name, but the file **still builds reputation organically** — early downloads may
  still see SmartScreen for weeks/months. There is no published threshold.
- **EV certificate** (~$250–600/yr, hardware token): historically bypassed
  SmartScreen instantly, **but Microsoft removed that instant-trust behaviour in
  2024** — EV now goes through the same reputation process as OV. So EV's main
  remaining advantage is kernel-mode driver signing (irrelevant to us).
- **Microsoft Trusted Signing / "Azure Artifact Signing"** ($9.99/mo Basic,
  5,000 signatures): the cheapest legitimate path, identity-validated via Entra.
  Caveat: **individual** enrollment is currently **US/Canada only**, and it also
  does **not** grant instant SmartScreen trust — same reputation model.

**Practical stance for an early-stage OSS project:** ship unsigned to start
(document the SmartScreen "More info → Run anyway" step in the README), and adopt
**Trusted Signing** (cheapest, $9.99/mo) once there's a steady release cadence to
build reputation. Distributing via **winget** also helps: users get the binary
through a trusted channel and the per-download SmartScreen friction is reduced.
Signing is **orthogonal** to the PowerShell-vs-exe decision — moving to an `.exe`
does not by itself fix SmartScreen, and staying on `.ps1` does not avoid it.

---

## Distribution via winget

winget is a viable, free distribution channel: submit a manifest to the
`microsoft/winget-pkgs` community repo (use `wingetcreate` to generate/submit).
This gives `winget install dockwin` and an upgrade path without us hosting
anything. It pairs naturally with the NSIS installer from (b) (winget can install
either the NSIS `-setup.exe` or a portable zip of the bare CLI). Note WSL itself is
now largely open-source (Microsoft, May 2025) and also winget-installable, so we
can even script "ensure WSL present" guidance around it.

---

## Size / footprint summary

| Artifact | Approx size | Admin to run? |
| --- | --- | --- |
| `dockwin.exe` CLI (release, stripped, lto, opt-level=z) | ~1–2 MB | No (except one-time `wsl --install`) |
| NSIS `-setup.exe` (GUI + CLI sidecar + WebView2 bootstrap is on Win11) | a few MB | Per-user install: no |
| WiX MSI | similar, slightly larger | Per-machine MSI: yes |

All dependencies are MIT/Apache; no GPL, no OpenSSL/C toolchain pulled in by the
CLI. Anti-bloat thesis preserved.

---

## Auto-updates (in-app updater)

dockwin updates **two independent things**, so there are two update paths. Both
are **opt-in and notify-only**: a small toast appears on launch when an update
exists, and nothing installs until the user clicks. No background service, no
silent installs — consistent with the anti-bloat thesis.

### 1. The dockwin app (GUI + bundled CLI + embedded `distro/` assets)

Uses Tauri's official **`tauri-plugin-updater`**. On launch the frontend
(`src/lib/updater.ts`) calls `check()` against the manifest endpoint configured
in `src-tauri/tauri.conf.json` (`plugins.updater.endpoints`), which points at the
GitHub release's `latest.json`. If a newer **signed** installer exists, the user
can install it; the plugin downloads the new NSIS `-setup.exe`, verifies its
Ed25519 signature against `plugins.updater.pubkey`, applies it, and relaunches.

**Maintainer setup — REQUIRED for app updates to work:**

1. Generate a signing keypair once: `pnpm tauri signer generate -w dockwin.key`
   (a key *was* generated during initial setup; regenerate if you don't have the
   private half). The public key is already in
   `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
2. Add two repo **secrets** (Settings → Secrets and variables → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — the contents of the private key file.
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the key's password (empty string if
     the key has none).
3. The release workflow (`.github/workflows/release.yml`) signs during
   `pnpm tauri build` and publishes `latest.json` (assembled by
   `scripts/make-latest-json.mjs`) alongside the installer. If the secrets are
   unset the build still succeeds but produces **no** `.sig`/`latest.json`, so the
   updater simply sees no update — releases stay publishable, just not
   auto-updatable.

> Losing the private key means you can't ship updates that existing installs will
> accept (the pinned `pubkey` won't verify a new key's signatures). Keep it safe;
> rotating it requires every user to reinstall manually once.

### 2. The Docker Engine (inside the WSL distro)

The app updater can't touch the engine — `dockerd` lives in the distro, installed
from the pinned Docker apt repo. `dockwin-core::ops::update_engine_reporting`
runs an in-place `apt-get install --only-upgrade docker-ce …` and restarts
`dockerd`, streaming progress. Exposed three ways:

- **CLI:** `dockwin update`.
- **GUI command:** `engine_update` (progress via the `engine://update` event),
  with `engine_update_check` for the "Docker Engine X → Y available" badge.
- The launch toast surfaces it when the engine is running and apt reports a newer
  candidate. The check is cheap (`apt-get update` + `apt-cache policy`) and never
  boots a stopped distro, so it's safe on launch.

---

## References

- Tauri — Embedding External Binaries (sidecar / `externalBin`, `-$TARGET_TRIPLE`):
  <https://v2.tauri.app/develop/sidecar/>
- Tauri — Embedding Additional Files (bundle resources):
  <https://v2.tauri.app/develop/resources/>
- Tauri — Windows Installer (NSIS vs WiX/MSI; WiX is Windows-only, NSIS cross-platform):
  <https://v2.tauri.app/distribute/windows-installer/>
- WSL elevation: `wsl --install` shows a UAC elevation prompt when run non-elevated
  (microsoft/WSL discussions/issues): <https://github.com/microsoft/WSL/discussions/11896>
- Code signing options (OV/EV, reputation model) — Microsoft Learn:
  <https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options>
- EV no longer bypasses SmartScreen instantly (2024 change) & reputation building:
  <https://weblog.west-wind.com/posts/2025/Jul/20/Fighting-through-Setting-up-Microsoft-Trusted-Signing>
- Microsoft Trusted Signing / Azure Artifact Signing pricing ($9.99/mo Basic) and
  individual eligibility (US/Canada): 
  <https://azure.microsoft.com/en-us/pricing/details/artifact-signing/> and
  <https://techcommunity.microsoft.com/blog/microsoft-security-blog/trusted-signing-is-now-open-for-individual-developers-to-sign-up-in-public-previ/4273554>
- winget community repo + `wingetcreate`:
  <https://github.com/microsoft/winget-pkgs>
- WSL open-sourced (May 2025): 
  <https://blogs.windows.com/windowsdeveloper/2026/06/02/build-2026-furthering-windows-as-the-trusted-platform-for-development/>

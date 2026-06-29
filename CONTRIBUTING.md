# Contributing to dockwin

Thanks for your interest in hacking on **dockwin** — a lightweight, un-bloated
Docker Desktop alternative for Windows 11. This is a small Rust workspace with a
Tauri v2 GUI; contributions of all sizes are welcome.

By contributing you agree that your contributions are dual-licensed under
**Apache-2.0 OR MIT**, matching the rest of the project.

---

## Prerequisites

You are developing **on Windows 11** (the engine target is Windows-native).

- **Windows 11** with WSL2 enabled and up to date: `wsl --update` then
  `wsl --version` (need a recent build for `systemd=true` support).
- **Rust** (stable, latest): install via <https://rustup.rs>.
  - MSVC toolchain (`x86_64-pc-windows-msvc`) is the supported target.
- **Node.js** 18+ and a package manager (npm/pnpm) for the GUI frontend.
- **Tauri v2 prerequisites**: the WebView2 runtime (preinstalled on Win11) and
  the MSVC build tools. See <https://v2.tauri.app/start/prerequisites/>.
- For end-to-end engine testing: enough disk for an imported Ubuntu WSL distro.

---

## Project layout

dockwin is **one Tauri v2 desktop app** at the repo root. The web frontend
(`src/`) is a stateless view; all real logic lives on the Rust side
(`src-tauri/`). The architecture's three logical pieces — a `dockwin-core`
brain, a thin `dockwin` CLI, and the `dockwin-gui` shell — are currently
**consolidated into this single crate**; the engine/Docker logic lives in
`src-tauri/src/docker.rs`. A standalone CLI binary is planned.

```
dockwin/
├── index.html              # frontend entry (Vite)
├── package.json            # frontend deps + `tauri` script
├── vite.config.js
├── svelte.config.js
├── src/                    # WEB FRONTEND (stateless view layer only)
│   ├── main.js             #   talks to the backend ONLY via Tauri commands/events
│   └── styles.css
├── src-tauri/              # RUST SIDE — the "dockwin-gui" crate (the brain, for now)
│   ├── Cargo.toml          #   the only Cargo manifest in the repo (no workspace yet)
│   ├── build.rs
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs         #   Tauri app entry, registers commands
│       ├── commands.rs     #   Tauri command surface (start/stop, list, logs, exec…)
│       └── docker.rs       #   engine lifecycle + bollard client + pipe relay
├── distro/                 # WSL provisioning assets shipped into the distro
│   ├── wsl.conf            #   [boot] systemd=true, interop off
│   └── provision-inside.sh #   installs pinned dockerd, iptables-legacy, enables autostart
├── scripts/
│   ├── Install-Dockwin.ps1 #   end-to-end, idempotent engine provisioner + wiring
│   └── Uninstall-Dockwin.ps1
├── README.md
├── CONTRIBUTING.md
├── LICENSE
└── .gitignore
```

> Note: the layout will likely split back into a real `crates/` Cargo workspace
> (core lib + CLI + GUI) as it grows; the **three logical pieces** — core logic,
> CLI, GUI — are the contract regardless of where they physically live.

### Where things live (mental model)

- **WSL provisioning** (import the rootfs, write `/etc/wsl.conf`,
  `/etc/docker/daemon.json`, install pinned `docker-ce`, force `iptables-legacy`,
  `systemctl enable docker`) → `dockwin-core` provisioner module. It shells out
  to `wsl.exe` and runs in-distro commands; it is idempotent.
- **Rust core / engine wiring** (named-pipe proxy, bollard client, engine
  start/stop/status) → `dockwin-core`. The pipe server relays each connection
  into the distro with `wsl.exe -d dockwin -e socat - UNIX-CONNECT:/var/run/docker.sock`.
- **WSL provisioning assets** (the `wsl.conf` and `provision-inside.sh` that get
  written into / run inside the distro, plus the PowerShell installer) →
  `distro/` and `scripts/Install-Dockwin.ps1`.
- **Frontend** (container/image lists, logs, exec terminal, stats) → `src/`. It
  is a **dumb view**: it talks only via Tauri commands/events. **No business
  logic in JS** — if you need logic, it belongs in the Rust side
  (`src-tauri/src/`) and is exposed through a Tauri command.

---

## Build & run

```powershell
# Install frontend deps (once) — this project uses pnpm
pnpm install

# Run the GUI in dev mode (hot-reloads the frontend, builds the Rust side)
pnpm tauri dev

# Production bundle
pnpm tauri build

# Provision / manage the engine (until a CLI binary lands, use the installer)
./scripts/Install-Dockwin.ps1      # import + set up the dedicated WSL distro
./scripts/Uninstall-Dockwin.ps1    # teardown (wsl --unregister, with confirm)
```

Lint & format before opening a PR (Rust side lives under `src-tauri/`):

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## Pull requests

- **Stay in your lane / keep PRs focused.** Touch the minimum surface for the
  change.
- Keep the **anti-bloat thesis** in mind: no telemetry, no always-on background
  service, no auto-updater. Features that add a persistent daemon will be
  pushed back on.
- Match existing style; run `cargo fmt` and `clippy`.
- Note any change to the **engine wiring** (pipe vs TCP fallback) or the
  **provisioning steps** clearly in the PR description — those are the riskiest
  areas (see "open risks" in the README).
- New user-visible behavior should update the **"What works / What's stubbed"**
  table in the README.

---

## Reporting bugs

Include: Windows build (`winver`), `wsl --version`, whether Docker Desktop is
also installed (mutual-exclusivity caveat), and whether you are on the
named-pipe path or the TCP 2375 fallback.

Thanks for helping keep Docker on Windows light. 🐳

<div align="center">

<img src="src-tauri/icons/icon.png" width="120" alt="dockwin icon" />

# dockwin

**A lightweight, un-bloated Docker Desktop alternative for Windows 11.**

Stock `dockerd` in a single dedicated WSL2 distro · native Tauri GUI · scriptable CLI · nothing else.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Platform](https://img.shields.io/badge/platform-Windows%2011-0078D6?logo=windows11&logoColor=white)](#getting-started)
[![Built with Tauri](https://img.shields.io/badge/Tauri-2-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![Built with Rust](https://img.shields.io/badge/Rust-stable-CE422B?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Built with Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Release](https://img.shields.io/github/v/release/p-arndt/dockwin?display_name=tag&sort=semver)](https://github.com/p-arndt/dockwin/releases)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

</div>

> [!IMPORTANT]
> Early development. Check [What works / what's stubbed](#what-works--whats-stubbed)
> before relying on this for anything important.

---

## Contents

- [Features](#features)
- [Getting started](#getting-started)
- [Why dockwin exists](#why-dockwin-exists)
- [Architecture](#architecture)
- [Known risks & caveats](#known-risks--caveats)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- 🪶 **No persistent Windows service, no VPN proxy, no telemetry, no auto-updater** — just a GUI/CLI talking to a dedicated `dockerd`.
- 🔌 **Native named-pipe relay** into the engine's unix socket — works with `bollard` *and* the stock `docker.exe` CLI via `docker context`.
- 📦 **Containers, images, volumes, networks** — full lifecycle: start/stop/restart/remove, pull/prune/tag/inspect, create/connect/disconnect.
- 🧩 **Compose stacks** — `up`/`down`/`build`/`pull`/`restart`/`logs`, with containers grouped by project, from the GUI or `dockwin up`/`down`.
- 📊 **Live container details** — CPU/mem/net/blk stats, `inspect` JSON, `top` processes, rename, pause/unpause.
- 🧹 **System view** — disk usage (`df`), prune (incl. all-images / volumes), engine info.
- ⚙️ **One-click provisioning** — imports a minimal (~29 MB) Ubuntu rootfs, installs pinned `dockerd`, wires systemd autostart, from the GUI's first-run panel or `dockwin install`.
- 🖥️ **Scriptable CLI** — `dockwin status/install/start/stop/uninstall` mirrors the GUI's setup logic exactly, plus `dockwin logs [-f] [<container>]` to tail (and follow) a single container or, with no container, the whole compose stack.

---

## Getting started

> [!WARNING]
> Don't run dockwin alongside Docker Desktop. Mirrored networking and
> `docker context` collisions between the two cause silent failures.

> [!NOTE]
> Requires Windows 11 with WSL2 enabled and up to date (`wsl --update`) — a
> recent build is needed for `systemd=true` support.

### Option A — download a release

Grab the installer from [Releases](https://github.com/p-arndt/dockwin/releases),
run it, then click **Set up engine** on the first-run screen. That's it —
the installer ships `dockwin.exe` as a sidecar, so there's nothing else to set up.

### Option B — build the installer yourself

```powershell
pnpm install
pnpm tauri build
```

Produces an NSIS installer under `target/release/bundle/nsis/`. Requires Rust
(stable, MSVC toolchain) and Node 18+ — see
[CONTRIBUTING.md](CONTRIBUTING.md#prerequisites) for the full prerequisite list.
Most tasks are also wrapped as [`just`](https://github.com/casey/just) recipes
(`just installer`, `just release minor`, …) — see
[docs/development.md](docs/development.md).

### Option C — run it in dev mode

```powershell
pnpm install
pnpm tauri dev      # hot-reloads the frontend, builds the Rust side
```

To work on just the CLI: `cargo build -p dockwin-cli` →
`target/debug/dockwin.exe`.

### Provisioning from the CLI

The GUI's first-run setup and the CLI call the same `dockwin-core` code:

```powershell
dockwin install      # provision the dockwin WSL2 distro
dockwin status        # is it registered/running and is dockerd reachable?
dockwin uninstall    # tear down (add --backup to export a .tar first)
```

### Behind a corporate proxy

Provisioning downloads the Ubuntu image and installs Docker via `apt`, so a
locked-down host that only reaches the internet through a proxy needs that proxy
configured. By default dockwin **auto-detects**: it uses the proxy WSL injects
from Windows *only if that proxy is actually reachable* from inside the distro,
and otherwise provisions with direct egress (so an inherited corporate proxy that
can't be resolved off the VPN/LAN no longer blocks setup). A chosen proxy is
threaded into the distro's `apt`, `curl` and `dockerd` (runtime `docker pull`)
consistently. To override:

```powershell
# GUI: type into the optional "HTTP(S) proxy" field on the setup screen.
dockwin install --proxy http://USER:PASS@HOST:PORT   # force a specific proxy
dockwin install --proxy direct                       # force proxy-less egress
```

> [!TIP]
> Many corporate proxies use **Negotiate/Kerberos**, which `apt` can't
> authenticate to even with `user:pass` (you'll see `407 Proxy authentication
> required` in the log). Run a local no-auth forwarder that does the
> Windows-credential handshake for you (e.g. [`px`](https://github.com/genotrance/px)
> or `cntlm`) and point dockwin at it: `--proxy http://127.0.0.1:3128`.

### Point the stock Docker CLI at dockwin (optional)

```powershell
docker context create dockwin --docker host=npipe:////./pipe/dockwin_engine
docker context use dockwin
docker run --rm hello-world
```

> [!TIP]
> Under default NAT, wildcard-published ports (`-p 8080:80`) are reachable at
> Windows `localhost:8080` automatically. `127.0.0.1`-bound publishes
> (`-p 127.0.0.1:8080:80`) are **not** forwarded — the GUI flags this when it
> links a port. If ports stop forwarding after sleep/wake, try `wsl --shutdown`.

---

## Why dockwin exists

Docker Engine (Moby) is Apache-2.0 — free, open, unencumbered. The friction is
**Docker Desktop**, whose subscription terms require a paid license once an
organization crosses a size/revenue threshold. Teams end up either paying
per-seat for what is essentially a GUI wrapped around an engine that's already
free, or dropping Docker Desktop and losing a decent developer experience.

dockwin's thesis: the engine is already free, so the wrapper should be too.
It also skips what Docker Desktop bundles on top of the engine — the
always-on background backend, the vpnkit-style network proxy, the
auto-updater, the telemetry. The whole product is one small Rust workspace.

| Piece | License |
| --- | --- |
| Docker Engine / `dockerd` (Moby) | Apache-2.0 |
| Rust crates (bollard, tokio, …) | MIT / Apache-2.0 |
| Tauri v2 | MIT / Apache-2.0 |
| Ubuntu WSL rootfs (base userland) | permissive / redistributable |
| **dockwin itself** | **Apache-2.0 OR MIT** |

You can use dockwin commercially, at any org size, for free.

---

## Architecture

A thin Windows-native Rust core (`dockwin-core`) drives one dedicated WSL2
distro (`dockwin`) running stock `dockerd`, with a Tauri v2 GUI and a `dockwin`
CLI built on top of it.

```mermaid
graph TD
    subgraph Windows["Windows (host)"]
        GUI["dockwin-gui<br/>(Tauri v2)<br/>web frontend"]
        CLI["dockwin.exe<br/>(CLI)"]
        Core["dockwin-core<br/>(Rust lib)<br/>- engine lifecycle<br/>- bollard client<br/>- named-pipe proxy"]
        Pipe["\\.\pipe\dockwin_engine"]

        GUI -->|Tauri cmd/event| Core
        CLI -->|args| Core
        Core -->|connect| Pipe
    end

    subgraph WSL["WSL2 distro 'dockwin' (Ubuntu)"]
        Systemd["systemd"]
        Dockerd["dockerd"]
        Socket["unix:///var/run/docker.sock"]
        Resources["containers / images / volumes"]

        Systemd -->|start| Dockerd
        Dockerd -->|listen| Socket
        Dockerd -->|manage| Resources
    end

    Pipe -->|wsl.exe -e socat| Socket

    Note["fallback: dockerd tcp://127.0.0.1:2375<br/>← localhostForwarding → Windows"]
```

`dockerd` listens only on its unix socket inside the distro — no TCP, no
network attack surface. `dockwin-core` relays it to Windows over a named pipe
(`\\.\pipe\dockwin_engine`), which both `bollard` and the stock `docker.exe`
CLI can use. Provisioning imports a minimal ~29 MB Ubuntu rootfs, installs
systemd and a pinned `dockerd`, and verifies the engine before reporting ready.

📖 Full wiring details, the provisioning steps, the component breakdown, and
the design-decision rationale live in **[docs/architecture.md](docs/architecture.md)**.

---

## Known risks & caveats

- **Relay throughput** under high connection churn (per-connection `wsl.exe`+socat spawn) is unproven; long-lived log/exec streams are fine, heavy churn may need the TCP fallback.
- **`systemd=true` needs recent WSL** (~2.1.5+); on stale inbox WSL it's silently ignored. The provisioner checks the WSL version first.
- **iptables nftables-vs-legacy** mismatch on newer Ubuntu can break container bridge networking even when dockerd starts; provisioning forces legacy.
- **Gzipped rootfs import** can fail with "Incorrect function." on older WSL; the provisioner always decompresses to a plain `.tar` first as a safety net.

> [!CAUTION]
> `wsl --unregister` permanently deletes the distro's `ext4.vhdx`. Teardown
> confirms first and can optionally export a backup tar.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for project layout, build/lint
commands (`just ci`), and PR guidelines. Short version: keep the anti-bloat
thesis in mind (no telemetry, no always-on background service, no
auto-updater), and call out any change to the engine wiring or provisioning
steps in your PR description — those are the riskiest areas.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE](LICENSE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in dockwin shall be dual-licensed as
above, without any additional terms or conditions.

---

<div align="center">

Made for developers who just want `dockerd` on Windows — nothing more. 🐳

</div>

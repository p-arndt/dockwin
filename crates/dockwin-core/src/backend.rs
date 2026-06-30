//! Platform engine backend abstraction.
//!
//! dockwin's job is "manage a Docker engine host and expose how a local client
//! reaches its `dockerd`". *How* that host is provided is platform-specific:
//!
//!   * Windows  → a dedicated WSL2 distro running stock `dockerd`, reached over a
//!     named-pipe relay ([`WslBackend`], today the only implementation).
//!   * Linux    → native `dockerd` already on `/var/run/docker.sock` (future
//!     `NativeUnixBackend`; would need no rootfs import and no relay).
//!   * macOS    → a Lima/colima-style VM exposing a unix socket (future).
//!
//! The CLI and the Tauri GUI depend on the [`EngineBackend`] trait and obtain the
//! right implementation from [`detect`], instead of calling the WSL-specific
//! [`crate::ops`] / [`crate::wsl`] functions directly. The WSL implementation
//! still lives in those modules; [`WslBackend`] is the thin seam over them, so
//! this refactor changes no Windows behaviour.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::ops::{self, EngineState, InstallOpts, Progress};

/// How a local Docker client (bollard / `docker.exe`) should reach `dockerd`.
///
/// Platform-neutral on purpose: the GUI maps this to its own bollard transport,
/// so the connection detail lives with the backend that knows it.
#[derive(Debug, Clone)]
pub enum EngineConnection {
    /// Windows named pipe hosted by a relay, in bollard form
    /// (e.g. `//./pipe/dockwin_engine`).
    NamedPipe(String),
    /// Direct unix domain socket (native Linux `dockerd`; future mac VM).
    Unix(PathBuf),
    /// Loopback TCP (INSECURE: unauthenticated, loopback only).
    Tcp(String),
}

/// bollard-form named pipe the Windows relay hosts (mirrors
/// `src-tauri/src/docker.rs::DEFAULT_PIPE_ADDR` and `wsl::PIPE_HOST`).
pub const PIPE_ADDR: &str = "//./pipe/dockwin_engine";

/// Loopback TCP fallback address (insecure; loopback only, never `0.0.0.0`).
pub const TCP_ADDR: &str = "tcp://127.0.0.1:2375";

/// A platform-specific Docker engine host. One implementation per platform
/// strategy; selected by [`detect`].
pub trait EngineBackend: Send + Sync {
    /// Coarse engine state (not provisioned / stopped / running).
    fn state(&self) -> Result<EngineState>;

    /// Provision the engine, forwarding every progress update to `report`.
    fn install(&self, opts: InstallOpts, report: &dyn Fn(Progress)) -> Result<()>;

    /// Boot the engine and wait up to `timeout_secs` for `dockerd` to answer.
    fn start(&self, timeout_secs: u64) -> Result<()>;

    /// Stop `dockerd` (and, when `terminate`, release the host VM's RAM).
    fn stop(&self, terminate: bool) -> Result<()>;

    /// Tear the engine down (optionally exporting a backup first).
    fn uninstall(
        &self,
        backup: bool,
        backup_path: Option<PathBuf>,
        assume_yes: bool,
    ) -> Result<()>;

    /// Run `docker compose -f <file> <action…>` against this engine, streaming
    /// combined output to `on_line`. Returns child success.
    fn compose_run(
        &self,
        file: &Path,
        action: &[&str],
        on_line: &mut dyn FnMut(&str),
    ) -> Result<bool>;

    /// Reset a broken / dangling engine registration so it can be cleanly
    /// reprovisioned (e.g. its disk image was deleted out from under it — the
    /// [`EngineState::Broken`] case).
    fn repair(&self) -> Result<()>;

    /// `docker compose up` (detached by default) for `file`.
    fn compose_up(
        &self,
        file: &Path,
        detach: bool,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<bool> {
        let mut action = vec!["up"];
        if detach {
            action.push("-d");
        }
        self.compose_run(file, &action, on_line)
    }

    /// `docker compose down` for `file`.
    fn compose_down(&self, file: &Path, on_line: &mut dyn FnMut(&str)) -> Result<bool> {
        self.compose_run(file, &["down"], on_line)
    }

    /// Primary connection a local client should use to reach `dockerd`.
    fn connection(&self) -> EngineConnection;

    /// Insecure loopback-TCP fallback, if this backend offers one.
    fn fallback_connection(&self) -> Option<EngineConnection> {
        None
    }
}

/// The Windows / WSL2 backend: a dedicated `dockwin` distro running stock
/// `dockerd`, reached via the named-pipe relay. A thin seam over [`crate::ops`].
#[derive(Default)]
pub struct WslBackend;

impl EngineBackend for WslBackend {
    fn state(&self) -> Result<EngineState> {
        ops::engine_state()
    }

    fn install(&self, opts: InstallOpts, report: &dyn Fn(Progress)) -> Result<()> {
        ops::install_reporting(opts, report)
    }

    fn start(&self, timeout_secs: u64) -> Result<()> {
        ops::start(timeout_secs)
    }

    fn stop(&self, terminate: bool) -> Result<()> {
        ops::stop(terminate)
    }

    fn uninstall(
        &self,
        backup: bool,
        backup_path: Option<PathBuf>,
        assume_yes: bool,
    ) -> Result<()> {
        ops::uninstall(backup, backup_path, assume_yes)
    }

    fn compose_run(
        &self,
        file: &Path,
        action: &[&str],
        on_line: &mut dyn FnMut(&str),
    ) -> Result<bool> {
        ops::compose_run(file, action, on_line)
    }

    fn repair(&self) -> Result<()> {
        ops::repair()
    }

    fn connection(&self) -> EngineConnection {
        EngineConnection::NamedPipe(PIPE_ADDR.to_string())
    }

    fn fallback_connection(&self) -> Option<EngineConnection> {
        Some(EngineConnection::Tcp(TCP_ADDR.to_string()))
    }
}

/// Select the engine backend for the current platform.
///
/// Today this is always [`WslBackend`]; dockwin targets Windows 11 and the
/// non-Windows branch only has to type-check on CI. Step 2 (a real
/// `NativeUnixBackend` for Linux dev against native `dockerd`) slots in behind a
/// `#[cfg(not(windows))]` arm here without touching any call site.
pub fn detect() -> Box<dyn EngineBackend> {
    Box::new(WslBackend)
}

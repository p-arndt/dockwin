//! The provisioning operations, ported from the original `scripts/*.ps1` to
//! native Rust. Each public fn maps to one engine action and is called by BOTH
//! the `dockwin` CLI and the Tauri GUI (via `engine_provision`/`engine_teardown`).
//!
//! The in-distro assets (`distro/wsl.conf`, `distro/provision-inside.sh`) are
//! EMBEDDED at build time via `include_str!`, so a bundled GUI works without any
//! loose files on disk. When the repo's `distro/` directory is present next to
//! the binary/cwd (dev), that on-disk copy is preferred so edits take effect
//! without a rebuild — the embedded copy is the shipping fallback.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::wsl::{self, DISTRO};

/// Shipping fallbacks: the real `distro/` assets, baked into the binary.
const EMBEDDED_WSL_CONF: &str = include_str!("../../../distro/wsl.conf");
const EMBEDDED_PROVISION: &str = include_str!("../../../distro/provision-inside.sh");
const EMBEDDED_SUPERVISE: &str = include_str!("../../../distro/dockwin-supervise.sh");

// ---------------------------------------------------------------------------
// Progress reporting
// ---------------------------------------------------------------------------

/// A provisioning progress update emitted as [`install_reporting`] advances.
/// The CLI prints these; the GUI forwards them to the frontend progress bar.
#[derive(Clone, Debug)]
pub struct Progress {
    /// Stable phase id ("preflight" | "download" | "decompress" | "import" |
    /// "configure" | "provision" | "verify" | "context" | "done").
    pub phase: &'static str,
    /// Human-readable message for this update.
    pub message: String,
    /// Best-effort overall completion, 0.0..=100.0 (weighted across phases).
    pub pct: f32,
    /// Severity hint for styling: "step" | "info" | "warn".
    pub level: &'static str,
}

impl Progress {
    fn step(phase: &'static str, pct: f32, msg: impl Into<String>) -> Self {
        Self { phase, message: msg.into(), pct, level: "step" }
    }
    fn info(phase: &'static str, pct: f32, msg: impl Into<String>) -> Self {
        Self { phase, message: msg.into(), pct, level: "info" }
    }
    fn warn(phase: &'static str, pct: f32, msg: impl Into<String>) -> Self {
        Self { phase, message: msg.into(), pct, level: "warn" }
    }
}

/// The default reporter used by the public [`install`]: print to stdout/stderr
/// exactly like the original CLI output. Public so callers driving a backend's
/// [`crate::backend::EngineBackend::install`] can reuse the canonical CLI output.
pub fn print_progress(p: &Progress) {
    match p.level {
        "warn" => eprintln!("    WARN: {}", p.message),
        "step" => println!("==> {}", p.message),
        _ => println!("    {}", p.message),
    }
}

fn step(msg: &str) {
    println!("==> {msg}");
}
fn ok(msg: &str) {
    println!("    {msg}");
}
fn warn(msg: &str) {
    eprintln!("    WARN: {msg}");
}

/// Format a byte count compactly (e.g. "226.3 MB").
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut f = n as f64;
    let mut i = 0;
    while f >= 1024.0 && i < UNITS.len() - 1 {
        f /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{f:.1} {}", UNITS[i])
    }
}

/// A `Read` adapter that reports how many bytes have flowed through it, used to
/// turn the opaque decompression step into a live progress fraction (based on
/// compressed input consumed, which is monotonic).
struct CountingReader<'a, R: Read> {
    inner: R,
    read: u64,
    total: u64,
    last_report: u64,
    cb: &'a dyn Fn(f32, &str),
}

impl<'a, R: Read> CountingReader<'a, R> {
    fn new(inner: R, total: u64, cb: &'a dyn Fn(f32, &str)) -> Self {
        Self { inner, read: 0, total, last_report: 0, cb }
    }
}

impl<R: Read> Read for CountingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        // Report at most every ~8 MiB to keep the event rate sane.
        if self.read - self.last_report >= 8 * 1024 * 1024 {
            self.last_report = self.read;
            let frac = if self.total > 0 {
                (self.read as f32 / self.total as f32).min(1.0)
            } else {
                0.0
            };
            (self.cb)(frac, &format!("decompressing… {}", human_bytes(self.read)));
        }
        Ok(n)
    }
}

/// Resolve the dir holding the distro's ext4.vhdx (default %LOCALAPPDATA%\dockwin\distro).
fn default_install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("dockwin").join("distro")
}

/// Where we record the chosen install dir so teardown/repair can find a CUSTOM
/// location. Kept in %LOCALAPPDATA%\dockwin (the default parent) — NOT inside the
/// distro dir, which gets deleted. A plain one-line text file (no serde dep).
fn state_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("dockwin").join("install-dir.txt")
}

/// Record the install dir an install used, so a later teardown/repair removes
/// the right folder even when it was a custom `--install_dir`. Best-effort.
fn record_install_dir(dir: &Path) {
    let p = state_path();
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&p, dir.to_string_lossy().as_bytes());
}

/// Read the install dir recorded at install time, if any.
fn read_recorded_install_dir() -> Option<PathBuf> {
    let s = fs::read_to_string(state_path()).ok()?;
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// Remove the distro's install directory (recorded custom dir, else the default)
/// plus the recorded-path file. Shared by [`uninstall`] and [`repair`] so the
/// folder never outlives the WSL registration. Best-effort: warns, never fails.
fn cleanup_install_dir() {
    let dir = read_recorded_install_dir().unwrap_or_else(default_install_dir);
    let is_default = dir == default_install_dir();
    if dir.exists() {
        match fs::remove_dir_all(&dir) {
            Ok(()) => ok(&format!("removed install directory {}", dir.display())),
            Err(e) => warn(&format!(
                "could not remove {} ({e}); delete it manually",
                dir.display()
            )),
        }
    }
    // Drop the recorded-path file (it lives in the default parent, not in `dir`).
    let _ = fs::remove_file(state_path());
    // Tidy our OWN default parent (%LOCALAPPDATA%\dockwin) only when it is empty;
    // never remove a user-chosen custom dir's parent. `remove_dir` no-ops unless
    // the directory is already empty.
    if is_default {
        if let Some(parent) = dir.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

/// Directory for persisted provisioning logs (%LOCALAPPDATA%\dockwin\logs).
fn logs_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("dockwin").join("logs")
}

/// Seconds since the Unix epoch (used to name a provisioning log uniquely).
fn unix_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A persisted provisioning log. Every [`Progress`] update during an install —
/// including the streamed in-distro apt/docker output — is appended here so a
/// failed setup can be debugged after the fact. Best-effort: if the file can't
/// be opened, logging silently no-ops and provisioning proceeds. Single-threaded
/// use (the install reporter is only ever called from one thread), so a `RefCell`
/// suffices for the interior mutability the `Fn` reporter needs.
struct ProvisionLog {
    file: Option<std::cell::RefCell<File>>,
    path: PathBuf,
}

impl ProvisionLog {
    fn open() -> Self {
        let dir = logs_dir();
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("provision-{}.log", unix_secs()));
        let file = File::create(&path).ok().map(std::cell::RefCell::new);
        if let Some(f) = &file {
            let _ = writeln!(
                f.borrow_mut(),
                "# dockwin provisioning log (epoch {})",
                unix_secs()
            );
        }
        Self { file, path }
    }

    fn write(&self, p: &Progress) {
        if let Some(f) = &self.file {
            let _ = writeln!(
                f.borrow_mut(),
                "[{:<10}] {:<4} {:>3.0}%  {}",
                p.phase, p.level, p.pct, p.message
            );
        }
    }
}

/// Locate one of the shipped distro assets (wsl.conf / provision-inside.sh) by
/// walking a few likely locations relative to cwd and the executable. Returns
/// None when only the embedded copy is available (the normal shipping case).
fn find_asset(name: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("distro").join(name));
        candidates.push(cwd.join("..").join("distro").join(name));
        candidates.push(cwd.join("..").join("..").join("distro").join(name));
        candidates.push(cwd.join("..").join("..").join("..").join("distro").join(name));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("distro").join(name));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Prefer an explicit path, then an on-disk `distro/` asset, then the embedded copy.
fn load_text_asset(explicit: Option<PathBuf>, name: &str, embedded: &str) -> Result<String> {
    if let Some(p) = explicit.or_else(|| find_asset(name)) {
        return fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()));
    }
    Ok(embedded.to_string())
}

/// Best-effort `Content-Length` for `url` via a `curl.exe` HEAD, so the download
/// can report a determinate percentage. Returns None when unknown.
fn head_content_length(url: &str) -> Option<u64> {
    let out = wsl::command("curl.exe")
        .args(["--fail", "--silent", "--show-error", "--location", "--head", url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // After redirects there may be several headers; the last one wins.
    text.lines()
        .rfind(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<u64>().ok())
}

/// Download a rootfs via the bundled Windows `curl.exe` (present on Win10 1803+),
/// avoiding a heavyweight HTTP crate dependency. Output is captured (not
/// inherited) so this works from the console-less GUI; the download runs as a
/// child while we poll the destination file size to report a live fraction, and
/// curl's own error text is surfaced on failure.
fn download_rootfs(url: &str, dest: &Path, progress: &dyn Fn(f32, &str)) -> Result<()> {
    use std::process::Stdio;
    let total = head_content_length(url);
    progress(0.0, "starting download…");
    let mut child = wsl::command("curl.exe")
        .arg("--fail")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("-o")
        .arg(dest)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn curl.exe for rootfs download (is curl.exe on PATH?)")?;

    loop {
        match child.try_wait().context("waiting on curl.exe")? {
            Some(status) => {
                if !status.success() {
                    let _ = fs::remove_file(dest);
                    let mut stderr = String::new();
                    if let Some(mut e) = child.stderr.take() {
                        let _ = e.read_to_string(&mut stderr);
                    }
                    let detail = stderr.trim();
                    let detail = if detail.is_empty() { "no error output from curl" } else { detail };
                    bail!("curl.exe failed to download the rootfs: {detail}\n(check your network, or pass an explicit rootfs path)");
                }
                let got = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                progress(1.0, &format!("downloaded {}", human_bytes(got)));
                return Ok(());
            }
            None => {
                let got = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                match total {
                    Some(t) if t > 0 => progress(
                        (got as f32 / t as f32).min(0.999),
                        &format!("downloaded {} / {}", human_bytes(got), human_bytes(t)),
                    ),
                    _ => progress(0.0, &format!("downloaded {}", human_bytes(got))),
                }
                sleep(Duration::from_millis(300));
            }
        }
    }
}

/// Lowercase hex encoding of arbitrary bytes (for SHA-256 digests). Avoids
/// pulling in a hex crate for this single, tiny use.
fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Stream `path` through SHA-256 and return its lowercase hex digest. Streaming
/// (64 KiB at a time) keeps the hundreds-of-MB rootfs out of memory.
fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path).with_context(|| format!("opening {} to hash", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).context("reading rootfs while hashing")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

/// Decompress a compressed rootfs tarball to a plain `.tar` (gz import fails with
/// "Incorrect function." on older WSL builds, and `wsl --import` cannot read xz
/// at all). `.gz`/`.tgz` use gzip; `.xz` uses lzma; a plain `.tar` is passed
/// through untouched.
fn ensure_plain_tar(src: &Path, tmp_dir: &Path, progress: &dyn Fn(f32, &str)) -> Result<PathBuf> {
    let lower = src.to_string_lossy().to_lowercase();
    let total = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
    if lower.ends_with(".gz") || lower.ends_with(".tgz") {
        progress(0.0, "decompressing .tar.gz → .tar");
        let out = tmp_dir.join("rootfs.tar");
        let input = File::open(src).with_context(|| format!("opening {}", src.display()))?;
        let counted = CountingReader::new(input, total, progress);
        let mut gz = flate2::read::GzDecoder::new(counted);
        let mut output = File::create(&out).context("creating decompressed tar")?;
        io::copy(&mut gz, &mut output).context("decompressing rootfs")?;
        progress(1.0, "decompressed");
        Ok(out)
    } else if lower.ends_with(".xz") {
        progress(0.0, "decompressing .tar.xz → .tar (wsl --import cannot read xz directly)");
        let out = tmp_dir.join("rootfs.tar");
        let input = File::open(src).with_context(|| format!("opening {}", src.display()))?;
        let counted = CountingReader::new(input, total, progress);
        let mut reader = io::BufReader::new(counted);
        let mut output = io::BufWriter::new(File::create(&out).context("creating decompressed tar")?);
        lzma_rs::xz_decompress(&mut reader, &mut output).context("decompressing xz rootfs")?;
        output.flush().ok();
        progress(1.0, "decompressed");
        Ok(out)
    } else {
        Ok(src.to_path_buf())
    }
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

/// Coarse engine state, shared by the CLI's `status` and the GUI status badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    NotProvisioned,
    Stopped,
    Running,
    /// Registered with WSL but unbootable — its backing `ext4.vhdx` is missing
    /// (e.g. the install dir was deleted). Needs a repair + reprovision.
    Broken,
    /// Registered and bootable, but provisioning was interrupted before Docker
    /// was installed (no `dockerd` binary). Re-running the idempotent provision
    /// finishes it — distinct from `Stopped` (which expects a healthy engine).
    Incomplete,
}

impl EngineState {
    pub fn as_str(self) -> &'static str {
        match self {
            EngineState::NotProvisioned => "not_provisioned",
            EngineState::Stopped => "stopped",
            EngineState::Running => "running",
            EngineState::Broken => "broken",
            EngineState::Incomplete => "incomplete",
        }
    }
}

/// Programmatic status used by callers that want a value (e.g. the GUI).
pub fn engine_state() -> Result<EngineState> {
    if !wsl::distro_exists(DISTRO)? {
        return Ok(EngineState::NotProvisioned);
    }
    // BROKEN: registered but its backing disk image is gone (e.g. the install
    // dir was deleted). Only flag Broken when we positively located its vhdx
    // path and the file is missing — if the path can't be read we don't guess,
    // so there are no false positives for unusual setups.
    if let Some(base) = wsl::distro_base_path(DISTRO) {
        if !base.join("ext4.vhdx").exists() {
            return Ok(EngineState::Broken);
        }
    }
    if wsl::distro_running(DISTRO)? {
        if wsl::docker_server_version()?.is_some() {
            return Ok(EngineState::Running);
        }
        // The distro is up but dockerd didn't answer. If Docker isn't even
        // installed, a previous provisioning was interrupted before it finished
        // — surface that as INCOMPLETE so callers offer "finish setup" (the
        // provision is idempotent) rather than a start that can't succeed. The
        // distro is already running here, so this probe never cold-boots it.
        if !wsl::docker_installed().unwrap_or(true) {
            return Ok(EngineState::Incomplete);
        }
    }
    Ok(EngineState::Stopped)
}

/// Human-readable status print, used by the CLI `status` subcommand.
pub fn status() -> Result<()> {
    step(&format!("dockwin status (distro '{DISTRO}')"));
    if wsl::command("wsl.exe").arg("--status").output().is_err() {
        bail!("wsl.exe not found. Install WSL2 first: wsl --install");
    }
    let registered = wsl::distro_exists(DISTRO)?;
    let running = registered && wsl::distro_running(DISTRO)?;
    println!("    distro registered : {}", yesno(registered));
    println!("    distro running    : {}", yesno(running));
    if running {
        match wsl::docker_server_version()? {
            Some(v) => println!("    dockerd reachable : yes (server {v})"),
            None => println!("    dockerd reachable : no (try `dockwin start`)"),
        }
    } else {
        println!("    dockerd reachable : n/a");
    }
    if registered && !running && matches!(engine_state()?, EngineState::Broken) {
        warn("distro is registered but its disk image is MISSING (broken).");
        warn("reset it with `dockwin repair`, then reprovision with `dockwin install`.");
    }
    if !registered {
        ok("not provisioned yet -> run `dockwin install`");
    }
    Ok(())
}

fn yesno(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

// ---------------------------------------------------------------------------
// install
// ---------------------------------------------------------------------------
#[derive(Default)]
pub struct InstallOpts {
    pub rootfs: Option<PathBuf>,
    pub install_dir: Option<PathBuf>,
    pub wsl_conf: Option<PathBuf>,
    pub provision_script: Option<PathBuf>,
    pub enable_tcp: bool,
}

/// Provision the engine, printing progress to stdout (the CLI path).
pub fn install(opts: InstallOpts) -> Result<()> {
    install_reporting(opts, &|p| print_progress(&p))
}

/// Boot the dockwin distro and return only once a trivial `-- true` exec
/// succeeds, in a bounded, self-healing loop.
///
/// Any WSL boot can transiently wedge in the shared WSL2 VM's teardown↔startup
/// window: the distro shows as "Running" but every exec fails with
/// `Wsl/Service/E_UNEXPECTED` and it never becomes usable (seen on locked-down
/// machines with other distros running / endpoint security inspecting the VM
/// start). This is a GENERAL WSL-service race, not systemd-specific — dropping
/// systemd (see distro/wsl.conf) removes the cold-boot that used to make it far
/// worse, but the teardown/startup race can still surface, so this retry loop
/// stays as cheap insurance. A `--terminate` + retry reliably clears the wedge,
/// and each attempt is time-capped so a genuine hang fails fast with output
/// instead of freezing silently (a plain `wsl -d … -- true` with no timeout
/// hangs forever there, freezing the whole install with no further log lines).
///
/// Set `terminate_first` to force a fresh boot on the first attempt too — used
/// right after writing a new wsl.conf so the new config is actually picked up.
fn boot_distro_resilient(report: &dyn Fn(Progress), terminate_first: bool) -> Result<()> {
    const ATTEMPTS: u32 = 6;
    // A HEALTHY boot is fast: without systemd the distro's /init is up and the
    // `-- true` exec returns within a few seconds (measured <15s on the
    // locked-down laptop this guards). The wedge, by contrast, either fails
    // *instantly* (E_UNEXPECTED — handled as a non-timeout below) or hangs
    // essentially forever. So a SHORT per-attempt cap loses nothing on a good boot
    // yet catches a hang quickly and spends the budget on MORE retries — each a
    // fresh roll of the timing dice — instead of a few long, doomed waits.
    const BOOT_TIMEOUT: Duration = Duration::from_secs(45);
    // Remembered across attempts so the final error can quote what WSL actually
    // said (the E_UNEXPECTED wedge, a MountVhd error, …) instead of a generic
    // "would not start".
    let mut last_output = String::new();
    for attempt in 1..=ATTEMPTS {
        if terminate_first || attempt > 1 {
            let _ = wsl::run(&["--terminate", DISTRO]);
            // GROWING backoff before re-booting. The E_UNEXPECTED / hang is a race
            // in WSL's teardown↔startup window for the shared WSL2 VM — worse when
            // another distro (e.g. docker-desktop) keeps that VM alive and endpoint
            // security inspects the VM start. Rushing terminate→boot re-triggers
            // it; giving the VM progressively more time to fully drop the instance
            // lets the window clear. Capped so worst-case runtime stays bounded.
            let backoff = (3 + (attempt - 1) * 2).min(15) as u64;
            sleep(Duration::from_secs(backoff));
        }
        report(Progress::info(
            "configure",
            81.0,
            format!("booting the distro (attempt {attempt}/{ATTEMPTS})…"),
        ));
        // Heartbeat every 15s so a slow-but-progressing boot doesn't look frozen.
        let mut on_tick = |secs: u64| {
            if secs > 0 && secs % 15 == 0 {
                report(Progress::info(
                    "configure",
                    81.0,
                    format!("still waiting for the distro to come up… ({secs}s elapsed)"),
                ));
            }
        };
        match wsl::run_with_timeout_captured(
            &["-d", DISTRO, "-u", "root", "--", "true"],
            BOOT_TIMEOUT,
            &mut on_tick,
        ) {
            Ok((true, _)) => return Ok(()),
            // Exec ran but exited non-zero (typically the E_UNEXPECTED wedge):
            // surface WSL's own message so the log shows WHY it failed.
            Ok((false, out)) => {
                last_output = out;
                let detail = last_output
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("(no output)");
                report(Progress::warn(
                    "configure",
                    81.0,
                    format!("distro exec failed (attempt {attempt}/{ATTEMPTS}): {detail}"),
                ));
            }
            // Timed out (killed) — the bail message already carries any output.
            Err(e) => {
                last_output = format!("{e:#}");
                report(Progress::warn(
                    "configure",
                    81.0,
                    format!("boot timed out (attempt {attempt}/{ATTEMPTS}): {last_output}"),
                ));
            }
        }
        if attempt < ATTEMPTS {
            report(Progress::warn(
                "configure",
                81.0,
                "resetting the distro and retrying…",
            ));
        }
    }
    let tail = if last_output.trim().is_empty() {
        String::new()
    } else {
        format!("\n\nLast WSL output:\n{}", last_output.trim())
    };
    bail!(
        "distro '{DISTRO}' would not start cleanly (tried {ATTEMPTS}×). This is \
         usually endpoint-security software delaying the WSL VM start, or another \
         WSL distro holding the VM — try `dockwin install` again, or reboot \
         Windows first.{tail}"
    );
}

/// Provision the engine, forwarding every progress update to `report`. The GUI
/// passes a reporter that re-emits these as Tauri events for its progress bar;
/// [`install`] passes a stdout printer. The pct values are weighted across the
/// phases so a single 0..100 bar advances smoothly:
///   preflight 0–3 · download 3–45 · decompress 45–62 · import 62–78 ·
///   configure 78–82 · provision 82–96 · verify/context 96–100.
pub fn install_reporting(opts: InstallOpts, report: &dyn Fn(Progress)) -> Result<()> {
    // Persist every progress update (incl. the streamed in-distro apt/docker
    // output) to a timestamped file so a failed setup can be debugged later.
    // Shadow `report` with a tee that logs then forwards to the real reporter,
    // so all existing `report(...)` call sites are captured with no extra churn.
    let log = ProvisionLog::open();
    let report = &|p: Progress| {
        log.write(&p);
        report(p);
    };
    report(Progress::info("preflight", 0.0, format!("logging to {}", log.path.display())));

    // --- 1. Preflight -------------------------------------------------------
    report(Progress::step("preflight", 1.0, "Preflight: checking WSL2"));
    let (ver_ok, ver_text) = wsl::capture(&["--version"])?;
    if ver_ok && !ver_text.trim().is_empty() {
        let first = ver_text.lines().next().unwrap_or("").trim().to_string();
        report(Progress::info("preflight", 2.0, first));
    } else {
        report(Progress::warn("preflight", 2.0, "old \"inbox\" WSL (no --version). wsl.conf [boot] command= needs a modern WSL; attempting `wsl --update`"));
        let _ = wsl::run(&["--update"]);
    }
    let _ = wsl::run(&["--set-default-version", "2"]);

    // --- 2. Resolve / fetch rootfs -----------------------------------------
    report(Progress::step("download", 3.0, "Resolving Ubuntu rootfs"));
    let tmp_dir = std::env::temp_dir().join(format!("dockwin-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).context("creating temp dir")?;

    let rootfs = match opts.rootfs {
        Some(p) => {
            if !p.is_file() {
                bail!("rootfs not found: {}", p.display());
            }
            p
        }
        None => {
            // Name the download after the URL's own file so decompression picks
            // the right codec (`.gz` for ubuntu-base, `.xz` for the cloud image)
            // — see `ensure_plain_tar`.
            let fname = wsl::DEFAULT_ROOTFS_URL
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("rootfs.tar.gz");
            let dest = tmp_dir.join(fname);
            let dl = |frac: f32, msg: &str| {
                report(Progress::info("download", 3.0 + frac * 42.0, msg));
            };
            download_rootfs(wsl::DEFAULT_ROOTFS_URL, &dest, &dl)?;
            // Verify the download against its pinned checksum BEFORE we import it
            // as a root distro. HTTPS authenticates the transport but not the
            // bytes at rest (poisoned mirror / intercepting proxy / stale cache);
            // a pinned hash does. Hard-fail (and delete the file) on mismatch.
            report(Progress::step("download", 44.0, "Verifying rootfs checksum"));
            let got = sha256_file(&dest)?;
            if !got.eq_ignore_ascii_case(wsl::DEFAULT_ROOTFS_SHA256) {
                let _ = fs::remove_file(&dest);
                bail!(
                    "rootfs checksum mismatch — refusing to import it.\n  \
                     expected {}\n  got      {}\n\
                     The download may be corrupt or tampered with. Retry; if it \
                     persists, pass a trusted local rootfs via --rootfs.",
                    wsl::DEFAULT_ROOTFS_SHA256,
                    got
                );
            }
            report(Progress::info("download", 45.0, "rootfs checksum verified"));
            dest
        }
    };

    // --- 2b. Decompress to a plain tar -------------------------------------
    report(Progress::step("decompress", 45.0, "Preparing rootfs for import"));
    let dz = |frac: f32, msg: &str| {
        report(Progress::info("decompress", 45.0 + frac * 17.0, msg));
    };
    let import_tar = ensure_plain_tar(&rootfs, &tmp_dir, &dz)?;
    report(Progress::info("decompress", 62.0, format!("rootfs ready: {}", import_tar.display())));

    // --- 3. Import the distro (idempotent) ----------------------------------
    report(Progress::step("import", 64.0, format!("Importing WSL2 distro '{DISTRO}'")));
    // Resolve the install dir up front and record it, so a later teardown/repair
    // can clean up a CUSTOM location (the record lives outside the deleted dir).
    let install_dir = opts.install_dir.unwrap_or_else(default_install_dir);
    record_install_dir(&install_dir);
    if wsl::distro_exists(DISTRO)? {
        report(Progress::warn("import", 78.0, format!(
            "distro '{DISTRO}' already exists; skipping import (re-provisioning in place)"
        )));
    } else {
        fs::create_dir_all(&install_dir).context("creating install dir")?;
        wsl::run_checked(&[
            "--import",
            DISTRO,
            &install_dir.to_string_lossy(),
            &import_tar.to_string_lossy(),
            "--version",
            "2",
        ])?;
        report(Progress::info("import", 78.0, format!("imported into {}", install_dir.display())));
    }

    // --- 4. Place /etc/wsl.conf + the dockerd supervisor -------------------
    report(Progress::step("configure", 79.0, "Configuring /etc/wsl.conf"));
    // Ensure the distro is booted before we exec into it. This first boot goes
    // through the bounded, self-healing helper (a plain unbounded `-- true` here
    // hangs forever on a WSL-service wedge, which is what froze the install at
    // 79%).
    boot_distro_resilient(report, false)?;

    // NON-systemd model (like Docker Desktop): the distro's own /init stays PID 1
    // and dockerd is kept alive by dockwin-supervise.sh, launched from wsl.conf's
    // [boot] command=. Write both BEFORE the apply-reboot below so the new
    // wsl.conf's boot command finds the supervisor already in place.
    let wsl_conf = load_text_asset(opts.wsl_conf, "wsl.conf", EMBEDDED_WSL_CONF)?;
    wsl::write_into_distro(&wsl_conf, "/etc/wsl.conf", "0644")?;

    let supervise = load_text_asset(None, "dockwin-supervise.sh", EMBEDDED_SUPERVISE)?;
    wsl::write_into_distro(&supervise, "/usr/local/sbin/dockwin-supervise.sh", "0755")?;

    report(Progress::info("configure", 81.0, "applying wsl.conf (restarting distro)"));
    // Terminate ONLY our distro — not a global `wsl --shutdown`, which would also
    // tear down every other running distro (e.g. a Docker Desktop VM) and can
    // block for a long time on locked-down machines. A distro re-reads its
    // /etc/wsl.conf on the next start, so terminating just `dockwin` is enough to
    // pick up the new config. `terminate_first` forces that fresh boot so the
    // newly written wsl.conf actually takes effect. This is now RACE-FREE (no
    // systemd cold-boot); the resilient loop remains as cheap insurance.
    report(Progress::info("configure", 81.0, "waiting for the distro to come back up…"));
    boot_distro_resilient(report, true)?;

    // --- 5. Provision inside ------------------------------------------------
    report(Progress::step("provision", 82.0, "Installing Docker Engine inside the distro (a few minutes)…"));
    let provision = load_text_asset(opts.provision_script, "provision-inside.sh", EMBEDDED_PROVISION)?;
    wsl::write_into_distro(&provision, "/usr/local/sbin/dockwin-provision.sh", "0755")?;

    let env_prefix = if opts.enable_tcp {
        report(Progress::warn("provision", 82.0, "TCP fallback enabled: dockerd will ALSO bind 127.0.0.1:2375 (INSECURE)"));
        "DOCKWIN_ENABLE_TCP=1 "
    } else {
        ""
    };
    // Stream the in-distro apt/docker output line-by-line so the GUI shows life.
    // pct creeps from 82 toward 95 as lines arrive (capped — duration is unknown).
    let cmd = format!("{env_prefix}/usr/local/sbin/dockwin-provision.sh 2>&1");
    let mut lines: u32 = 0;
    let mut on_line = |line: &str| {
        lines += 1;
        let pct = (82.0 + lines as f32 * 0.15).min(95.0);
        report(Progress::info("provision", pct, line.to_string()));
    };
    let provisioned = wsl::run_streaming(
        &["-d", DISTRO, "-u", "root", "--", "bash", "-lc", &cmd],
        &mut on_line,
    )
    .context("in-distro provisioning failed")?;
    if !provisioned {
        bail!("in-distro provisioning script exited with an error (see the log above)");
    }

    // --- 6. Verify dockerd --------------------------------------------------
    report(Progress::step("verify", 96.0, "Verifying dockerd"));
    match wsl::docker_server_version()? {
        Some(v) => report(Progress::info("verify", 97.0, format!("dockerd server version: {v}"))),
        None => report(Progress::warn("verify", 97.0, "could not confirm dockerd yet; check: wsl -d dockwin -u root -- tail -n 50 /var/log/dockwin-dockerd.log")),
    }

    // --- 7. Wire the Windows docker context (best effort) -------------------
    report(Progress::step("context", 98.0, "Wiring Windows docker context"));
    wire_docker_context(opts.enable_tcp);

    report(Progress::step("done", 100.0, "dockwin engine ready."));
    let _ = fs::remove_dir_all(&tmp_dir);
    Ok(())
}

/// Register/update the Windows docker CONTEXT pointing at the named pipe.
/// Best-effort: silently skips if docker.exe is not on PATH.
fn wire_docker_context(enable_tcp: bool) {
    let probe = wsl::command("docker.exe")
        .args(["context", "ls", "--format", "{{.Name}}"])
        .output();
    let existing = match probe {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => {
            warn("docker.exe not found on PATH; skipped context creation");
            return;
        }
    };
    let has = |name: &str| existing.lines().any(|l| l.trim() == name);
    let host = format!("host={}", wsl::PIPE_HOST);
    if has(DISTRO) {
        let _ = wsl::command("docker.exe")
            .args(["context", "update", DISTRO, "--docker", &host])
            .status();
        ok(&format!("updated docker context '{DISTRO}' -> {}", wsl::PIPE_HOST));
    } else {
        let _ = wsl::command("docker.exe")
            .args([
                "context",
                "create",
                DISTRO,
                "--docker",
                &host,
                "--description",
                "dockwin engine via named-pipe relay",
            ])
            .status();
        ok(&format!("created docker context '{DISTRO}' -> {}", wsl::PIPE_HOST));
    }
    if enable_tcp {
        // TODO: register an insecure `dockwin-tcp` loopback context too.
        warn("TCP fallback context wiring is stubbed (TODO)");
    }
}

// ---------------------------------------------------------------------------
// compose
// ---------------------------------------------------------------------------

/// Translate an absolute Windows path to its WSL `/mnt` path so the in-distro
/// `docker compose` can read the file AND resolve relative bind-mounts against
/// the same folder (e.g. `E:\proj\compose.yml` -> `/mnt/e/proj/compose.yml`).
fn to_wsl_path(p: &Path) -> Result<String> {
    let s = p.to_string_lossy().replace('\\', "/");
    let b = s.as_bytes();
    if s.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        let drive = (b[0] as char).to_ascii_lowercase();
        Ok(format!("/mnt/{drive}{}", &s[2..]))
    } else if s.starts_with('/') {
        Ok(s) // already a unix-style path
    } else {
        bail!("compose file must be an absolute path: {}", p.display())
    }
}

/// Make sure dockerd is up before a compose call (best effort, quick).
/// Non-systemd model: the supervisor keeps dockerd alive and is idempotent (a
/// no-op while one is running), so simply (re)invoking it is enough.
fn ensure_dockerd() {
    let _ = wsl::run(&[
        "-d", DISTRO, "-u", "root", "--",
        "/usr/local/sbin/dockwin-supervise.sh",
    ]);
}

/// Run `docker compose -f <file> <action…>` INSIDE the dockwin distro, streaming
/// combined output to `on_line`. We `cd` into the compose file's directory first
/// so relative paths (bind mounts, env_file, build contexts) resolve exactly as
/// they would on the Windows side via the `/mnt` mount. Returns child success.
///
/// This is why plain `docker compose up` on Windows fails for dockwin users: the
/// Windows CLI targets Docker Desktop's pipe. `dockwin up` (this) targets the
/// dockwin engine's own socket from inside the distro instead.
pub fn compose_run(
    file: &Path,
    action: &[&str],
    on_line: &mut dyn FnMut(&str),
) -> Result<bool> {
    if !wsl::distro_exists(DISTRO)? {
        bail!("engine '{DISTRO}' is not provisioned. Set it up first (`dockwin install`).");
    }
    ensure_dockerd();
    let wpath = to_wsl_path(file)?;
    let dir = match wpath.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => wpath[..i].to_string(),
    };
    let joined = action.join(" ");
    // Shell-escape the user-derived paths (this runs as root in the distro, so a
    // single quote in the path must not be able to break out and inject). The
    // `action` args are fixed app-controlled flags, so they pass through as-is.
    let script = format!(
        "cd {} && docker compose -f {} {joined} 2>&1",
        wsl::sh_quote(&dir),
        wsl::sh_quote(&wpath)
    );
    wsl::run_streaming(
        &["-d", DISTRO, "-u", "root", "--", "bash", "-lc", &script],
        on_line,
    )
    .context("running docker compose inside the distro failed")
}

/// `dockwin up`: `docker compose up` (detached by default) for `file`.
pub fn compose_up(file: &Path, detach: bool, on_line: &mut dyn FnMut(&str)) -> Result<bool> {
    let mut action = vec!["up"];
    if detach {
        action.push("-d");
    }
    compose_run(file, &action, on_line)
}

/// `dockwin down`: `docker compose down` for `file`.
pub fn compose_down(file: &Path, on_line: &mut dyn FnMut(&str)) -> Result<bool> {
    compose_run(file, &["down"], on_line)
}

/// Options for [`container_logs`].
#[derive(Debug, Clone, Default)]
pub struct LogsOpts {
    /// Keep the stream open and follow new output (`docker logs --follow`).
    pub follow: bool,
    /// Prefix each line with its RFC3339 timestamp (`--timestamps`).
    pub timestamps: bool,
    /// Tail only the last N lines first. `None` shows the full history.
    pub tail: Option<u32>,
}

/// `dockwin logs`: `docker logs [--follow] [--timestamps] [--tail N] <container>`
/// INSIDE the dockwin distro, streaming combined stdout+stderr to `on_line`.
///
/// `2>&1` merges the container's stderr into stdout so `run_streaming` (which
/// only reads the child's stdout) sees both streams, exactly like `compose_run`.
/// Returns whether `docker logs` exited successfully (a `--follow` stream that
/// the user cancels with Ctrl-C reports failure, which the CLI treats as a clean
/// stop rather than an error).
pub fn container_logs(
    container: &str,
    opts: &LogsOpts,
    on_line: &mut dyn FnMut(&str),
) -> Result<bool> {
    if !wsl::distro_exists(DISTRO)? {
        bail!("engine '{DISTRO}' is not provisioned. Set it up first (`dockwin install`).");
    }
    ensure_dockerd();
    // Single-quote and escape the container ref so a name/id can't break out of
    // the shell command (`docker` refs can't legally contain quotes, but be safe).
    let safe = container.replace('\'', "'\\''");
    let mut cmd = String::from("docker logs");
    if opts.follow {
        cmd.push_str(" --follow");
    }
    if opts.timestamps {
        cmd.push_str(" --timestamps");
    }
    if let Some(n) = opts.tail {
        cmd.push_str(&format!(" --tail {n}"));
    }
    cmd.push_str(&format!(" '{safe}' 2>&1"));
    wsl::run_streaming(
        &["-d", DISTRO, "-u", "root", "--", "bash", "-lc", &cmd],
        on_line,
    )
    .context("running docker logs inside the distro failed")
}

// ---------------------------------------------------------------------------
// start
// ---------------------------------------------------------------------------
pub fn start(timeout_secs: u64) -> Result<()> {
    if !wsl::distro_exists(DISTRO)? {
        bail!("distro '{DISTRO}' is not registered. Run install first.");
    }
    if matches!(engine_state()?, EngineState::Broken) {
        bail!(
            "distro '{DISTRO}' is registered but its disk image is missing, so it \
             cannot boot. Run `dockwin repair` to reset the registration, then \
             `dockwin install` to reprovision."
        );
    }
    step(&format!("Booting distro '{DISTRO}'"));
    if !wsl::run(&["-d", DISTRO, "-u", "root", "--", "true"])? {
        bail!(
            "failed to boot distro '{DISTRO}' (its disk image may be missing or \
             corrupt). Try `dockwin repair`, then `dockwin install`."
        );
    }
    ok("distro is running.");

    // Fast-fail an interrupted setup: if Docker was never installed, no amount
    // of waiting will bring dockerd up. Tell the user to finish provisioning
    // (which is idempotent and resumes safely) instead of timing out after 60s.
    if !wsl::docker_installed().unwrap_or(true) {
        bail!(
            "the engine distro '{DISTRO}' is registered but Docker is not \
             installed — a previous setup was interrupted. Re-run `dockwin \
             install` (or the GUI's Set up engine) to finish; it resumes safely."
        );
    }

    // Booting the distro (above) already runs wsl.conf's [boot] command=, which
    // launches the supervisor -> dockerd. Defensively invoke the supervisor once
    // more here in case the boot command didn't run (older WSL, or the distro was
    // already up); it self-detaches and is idempotent (a no-op while a supervisor
    // is alive), so a second call is harmless.
    step("Ensuring dockerd is started");
    let _ = wsl::run(&[
        "-d", DISTRO, "-u", "root", "--",
        "/usr/local/sbin/dockwin-supervise.sh",
    ]);

    step("Waiting for dockerd to become reachable");
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if let Some(v) = wsl::docker_server_version()? {
            ok(&format!("dockerd is up (server {v})."));
            println!();
            println!("dockwin engine started.");
            return Ok(());
        }
        if Instant::now() >= deadline {
            warn(&format!("dockerd not reachable after {timeout_secs}s."));
            warn("diagnose: wsl -d dockwin -u root -- tail -n 50 /var/log/dockwin-dockerd.log");
            bail!("engine did not come up");
        }
        sleep(Duration::from_secs(1));
    }
}

// ---------------------------------------------------------------------------
// stop
// ---------------------------------------------------------------------------
pub fn stop(terminate: bool) -> Result<()> {
    if !wsl::distro_exists(DISTRO)? {
        warn(&format!("distro '{DISTRO}' is not registered; nothing to stop."));
        return Ok(());
    }
    if !wsl::distro_running(DISTRO)? {
        ok(&format!("distro '{DISTRO}' is already stopped."));
        return Ok(());
    }
    step("Stopping dockerd");
    // Kill the SUPERVISOR LOOP first so it stops respawning dockerd, THEN SIGTERM
    // dockerd for a clean shutdown; finally drop the pidfile. Order matters: if we
    // killed dockerd first the supervisor would immediately restart it.
    let stop_cmd = "if [ -f /run/dockwin-supervise.pid ]; then \
        kill \"$(cat /run/dockwin-supervise.pid)\" 2>/dev/null || true; fi; \
        pkill -TERM dockerd 2>/dev/null || true; \
        rm -f /run/dockwin-supervise.pid";
    let _ = wsl::run(&["-d", DISTRO, "-u", "root", "--", "bash", "-lc", stop_cmd]);
    ok("dockerd stopped (the supervisor restarts it on next distro boot).");

    if terminate {
        step(&format!("Terminating distro '{DISTRO}' to free memory"));
        let _ = wsl::run(&["--terminate", DISTRO]);
        ok(&format!("distro '{DISTRO}' terminated."));
    } else {
        warn("distro VM left running (fast restart). Use --terminate to free its RAM.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// uninstall
// ---------------------------------------------------------------------------
pub fn uninstall(backup: bool, backup_path: Option<PathBuf>, assume_yes: bool) -> Result<()> {
    // Remove the Windows docker context(s) first (best effort).
    step("Removing Windows docker context(s)");
    for ctx in [DISTRO.to_string(), format!("{DISTRO}-tcp")] {
        let _ = wsl::command("docker.exe")
            .args(["context", "rm", "-f", &ctx])
            .output();
    }

    if !wsl::distro_exists(DISTRO)? {
        warn(&format!("distro '{DISTRO}' not registered; nothing to unregister."));
        println!("dockwin uninstall complete (context-only).");
        return Ok(());
    }

    step(&format!("Tearing down WSL2 distro '{DISTRO}'"));
    warn("wsl --unregister PERMANENTLY deletes the distro vhdx. There is no undo.");
    if !assume_yes {
        print!("Type the distro name '{DISTRO}' to confirm deletion: ");
        io::stdout().flush().ok();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).context("reading confirmation")?;
        if answer.trim() != DISTRO {
            println!("Aborted; nothing was deleted.");
            return Ok(());
        }
    }

    if backup {
        let path = backup_path.unwrap_or_else(|| {
            let home = std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            home.join(format!("dockwin-backup-{}.tar", std::process::id()))
        });
        step(&format!("Exporting backup -> {}", path.display()));
        if wsl::run(&["--export", DISTRO, &path.to_string_lossy()])? {
            ok("backup written.");
        } else {
            warn("backup export failed; continuing with unregister.");
        }
    }

    let _ = wsl::run(&["--terminate", DISTRO]);
    if wsl::run(&["--unregister", DISTRO])? {
        ok(&format!("unregistered '{DISTRO}' (vhdx deleted)."));
    } else {
        warn("unregister failed; the distro may already be gone.");
    }

    // `wsl --unregister` deletes the ext4.vhdx but leaves the import directory
    // behind — and on a partial/failed unregister the vhdx itself can linger.
    // Remove the recorded (or default) install dir so teardown stays symmetric.
    cleanup_install_dir();

    println!("dockwin uninstall complete.");
    Ok(())
}

// ---------------------------------------------------------------------------
// repair
// ---------------------------------------------------------------------------

/// Reset a broken / dangling engine registration so it can be cleanly
/// reprovisioned — the [`EngineState::Broken`] case, e.g. the distro is still
/// registered with WSL but its `ext4.vhdx` was deleted out from under it.
/// Terminates and unregisters the distro (there is no live data to preserve when
/// it is broken) and removes the recorded/default install dir + state record.
/// Docker contexts are left in place; a fresh `install` reuses them. Idempotent.
pub fn repair() -> Result<()> {
    if !wsl::distro_exists(DISTRO)? {
        ok(&format!(
            "nothing to repair: distro '{DISTRO}' is not registered."
        ));
        cleanup_install_dir(); // drop any stale dir / state record anyway
        println!("Provision with `dockwin install` (or the GUI's Set up engine).");
        return Ok(());
    }
    step(&format!("Repairing engine: unregistering distro '{DISTRO}'"));
    let _ = wsl::run(&["--terminate", DISTRO]);
    if wsl::run(&["--unregister", DISTRO])? {
        ok("unregistered the broken distro.");
    } else {
        warn("unregister failed; the distro may already be gone.");
    }
    cleanup_install_dir();
    println!("Engine reset. Reprovision with `dockwin install` (or the GUI's Set up engine).");
    Ok(())
}

// ---------------------------------------------------------------------------
// update (in-place Docker Engine upgrade)
// ---------------------------------------------------------------------------

/// Installed vs. available Docker Engine versions inside the distro, used by the
/// GUI's "engine update available" badge and the CLI `status`. Both are the apt
/// version strings (e.g. `5:27.3.1-1~ubuntu.24.04~noble`); `None` means unknown
/// (engine not provisioned/running, or apt couldn't answer).
#[derive(Debug, Clone, Default)]
pub struct EngineUpdate {
    pub installed: Option<String>,
    pub candidate: Option<String>,
}

impl EngineUpdate {
    /// True when apt knows of a candidate that differs from what's installed.
    pub fn update_available(&self) -> bool {
        match (&self.installed, &self.candidate) {
            (Some(i), Some(c)) => i != c,
            _ => false,
        }
    }
}

/// Check whether a newer Docker Engine is available in the pinned apt repo.
///
/// Cheap and side-effect-free for the caller: it only refreshes apt metadata
/// (`apt-get update`) and reads `apt-cache policy` — it never installs anything.
/// To keep app launch snappy it does NOT boot a stopped distro; if the engine
/// isn't running it returns an empty result (both fields `None`) rather than
/// paying a cold-boot. Best-effort: any failure yields an empty result.
pub fn engine_update_check() -> Result<EngineUpdate> {
    if !wsl::distro_exists(DISTRO)? || !wsl::distro_running(DISTRO)? {
        return Ok(EngineUpdate::default());
    }
    // Refresh the repo index quietly, then read the installed/candidate lines.
    let script = "export DEBIAN_FRONTEND=noninteractive; \
        apt-get update -qq >/dev/null 2>&1; \
        apt-cache policy docker-ce 2>/dev/null";
    let (ok, text) =
        wsl::capture(&["-d", DISTRO, "-u", "root", "--", "bash", "-lc", script])?;
    if !ok {
        return Ok(EngineUpdate::default());
    }
    let mut out = EngineUpdate::default();
    for line in text.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Installed:") {
            let v = v.trim();
            if v != "(none)" && !v.is_empty() {
                out.installed = Some(v.to_string());
            }
        } else if let Some(v) = l.strip_prefix("Candidate:") {
            let v = v.trim();
            if v != "(none)" && !v.is_empty() {
                out.candidate = Some(v.to_string());
            }
        }
    }
    Ok(out)
}

/// Upgrade the Docker Engine packages in the already-provisioned distro to the
/// latest in the pinned Docker apt repo, then restart `dockerd`. Streams every
/// progress update to `report` (the GUI forwards these as `engine://update`
/// events; the CLI prints them). Idempotent — a no-op when already current.
///
/// This is the in-place engine updater: it upgrades only the docker-* packages
/// (engine/cli/containerd/buildx/compose), never the whole distro, so it is far
/// lighter than re-running [`install`].
pub fn update_engine_reporting(report: &dyn Fn(Progress)) -> Result<()> {
    if !wsl::distro_exists(DISTRO)? {
        bail!("engine '{DISTRO}' is not provisioned. Set it up first (`dockwin install`).");
    }
    if matches!(engine_state()?, EngineState::Broken) {
        bail!(
            "engine '{DISTRO}' is broken (its disk image is missing). Run \
             `dockwin repair`, then `dockwin install` before updating."
        );
    }

    report(Progress::step("preflight", 1.0, "Booting distro"));
    wsl::run_checked(&["-d", DISTRO, "-u", "root", "--", "true"])?;

    // apt-get update + upgrade ONLY the docker packages. --only-upgrade leaves a
    // package untouched if it isn't already installed, so this never pulls in new
    // components — it just moves the existing ones forward.
    report(Progress::step("update", 5.0, "Upgrading Docker Engine packages…"));
    let script = "export DEBIAN_FRONTEND=noninteractive; \
        apt-get update -y && \
        apt-get install -y --only-upgrade --no-install-recommends \
          docker-ce docker-ce-cli containerd.io \
          docker-buildx-plugin docker-compose-plugin 2>&1";
    let mut lines: u32 = 0;
    let mut on_line = |line: &str| {
        lines += 1;
        // Duration is unknown; creep from 10 toward 85 as apt output arrives.
        let pct = (10.0 + lines as f32 * 0.4).min(85.0);
        report(Progress::info("update", pct, line.to_string()));
    };
    let upgraded = wsl::run_streaming(
        &["-d", DISTRO, "-u", "root", "--", "bash", "-lc", script],
        &mut on_line,
    )
    .context("docker package upgrade failed")?;
    if !upgraded {
        bail!("docker package upgrade exited with an error (see the log above)");
    }

    // Restart dockerd so a new daemon binary actually takes effect. Non-systemd
    // model: SIGTERM the running dockerd and let the supervisor respawn it (~2s)
    // on the upgraded binary; if no supervisor is running, (re)invoke it to start
    // one. `pkill` is best-effort — a fresh supervisor start covers the cold case.
    report(Progress::step("restart", 90.0, "Restarting dockerd"));
    let restart = "pkill -TERM dockerd 2>/dev/null || true; \
        /usr/local/sbin/dockwin-supervise.sh";
    let _ = wsl::run(&["-d", DISTRO, "-u", "root", "--", "bash", "-lc", restart]);

    report(Progress::step("verify", 95.0, "Verifying dockerd"));
    match wsl::docker_server_version()? {
        Some(v) => report(Progress::info(
            "verify",
            98.0,
            format!("dockerd server version: {v}"),
        )),
        None => report(Progress::warn(
            "verify",
            98.0,
            "could not confirm dockerd after the upgrade; check: \
             wsl -d dockwin -u root -- tail -n 50 /var/log/dockwin-dockerd.log",
        )),
    }
    report(Progress::step("done", 100.0, "Docker engine up to date."));
    Ok(())
}

/// Upgrade the engine, printing progress to stdout (the CLI path).
pub fn update_engine() -> Result<()> {
    update_engine_reporting(&|p| print_progress(&p))
}

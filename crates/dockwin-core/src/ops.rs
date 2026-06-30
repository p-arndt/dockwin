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
        .filter(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .next_back()
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
}

impl EngineState {
    pub fn as_str(self) -> &'static str {
        match self {
            EngineState::NotProvisioned => "not_provisioned",
            EngineState::Stopped => "stopped",
            EngineState::Running => "running",
        }
    }
}

/// Programmatic status used by callers that want a value (e.g. the GUI).
pub fn engine_state() -> Result<EngineState> {
    if !wsl::distro_exists(DISTRO)? {
        return Ok(EngineState::NotProvisioned);
    }
    if wsl::distro_running(DISTRO)? && wsl::docker_server_version()?.is_some() {
        return Ok(EngineState::Running);
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

/// Provision the engine, forwarding every progress update to `report`. The GUI
/// passes a reporter that re-emits these as Tauri events for its progress bar;
/// [`install`] passes a stdout printer. The pct values are weighted across the
/// phases so a single 0..100 bar advances smoothly:
///   preflight 0–3 · download 3–45 · decompress 45–62 · import 62–78 ·
///   configure 78–82 · provision 82–96 · verify/context 96–100.
pub fn install_reporting(opts: InstallOpts, report: &dyn Fn(Progress)) -> Result<()> {
    // --- 1. Preflight -------------------------------------------------------
    report(Progress::step("preflight", 1.0, "Preflight: checking WSL2"));
    let (ver_ok, ver_text) = wsl::capture(&["--version"])?;
    if ver_ok && !ver_text.trim().is_empty() {
        let first = ver_text.lines().next().unwrap_or("").trim().to_string();
        report(Progress::info("preflight", 2.0, first));
    } else {
        report(Progress::warn("preflight", 2.0, "old \"inbox\" WSL (no --version). systemd=true needs WSL >= 2.1.5; attempting `wsl --update`"));
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
            let dest = tmp_dir.join("ubuntu-noble-cloudimg.rootfs.tar.xz");
            let dl = |frac: f32, msg: &str| {
                report(Progress::info("download", 3.0 + frac * 42.0, msg));
            };
            download_rootfs(wsl::DEFAULT_ROOTFS_URL, &dest, &dl)?;
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
    if wsl::distro_exists(DISTRO)? {
        report(Progress::warn("import", 78.0, format!(
            "distro '{DISTRO}' already exists; skipping import (re-provisioning in place)"
        )));
    } else {
        let install_dir = opts.install_dir.unwrap_or_else(default_install_dir);
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

    // --- 4. Place /etc/wsl.conf --------------------------------------------
    report(Progress::step("configure", 79.0, "Configuring /etc/wsl.conf"));
    wsl::run_checked(&["-d", DISTRO, "-u", "root", "--", "true"])?; // ensure booted
    let wsl_conf = load_text_asset(opts.wsl_conf, "wsl.conf", EMBEDDED_WSL_CONF)?;
    wsl::write_into_distro(&wsl_conf, "/etc/wsl.conf", "0644")?;

    report(Progress::info("configure", 81.0, "applying wsl.conf (restarting distro to enable systemd)"));
    let _ = wsl::run(&["--shutdown"]);
    sleep(Duration::from_secs(3));
    wsl::run_checked(&["-d", DISTRO, "-u", "root", "--", "true"])?;

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
        None => report(Progress::warn("verify", 97.0, "could not confirm dockerd yet; check: wsl -d dockwin -u root -- journalctl -u docker")),
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
fn ensure_dockerd() {
    let start_cmd = "if [ -d /run/systemd/system ]; then systemctl is-active --quiet docker || systemctl start docker; else service docker status >/dev/null 2>&1 || service docker start; fi";
    let _ = wsl::run(&["-d", DISTRO, "-u", "root", "--", "bash", "-lc", start_cmd]);
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
    // Quote the paths (they may contain spaces). Compose files rarely contain
    // single quotes; we accept that edge case rather than full shell-escaping.
    let script = format!("cd '{dir}' && docker compose -f '{wpath}' {joined} 2>&1");
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

// ---------------------------------------------------------------------------
// start
// ---------------------------------------------------------------------------
pub fn start(timeout_secs: u64) -> Result<()> {
    if !wsl::distro_exists(DISTRO)? {
        bail!("distro '{DISTRO}' is not registered. Run install first.");
    }
    step(&format!("Booting distro '{DISTRO}'"));
    wsl::run_checked(&["-d", DISTRO, "-u", "root", "--", "true"])?;
    ok("distro is running.");

    step("Ensuring dockerd is started");
    let start_cmd = "if [ -d /run/systemd/system ]; then systemctl is-active --quiet docker || systemctl start docker; else service docker status >/dev/null 2>&1 || service docker start; fi";
    let _ = wsl::run(&["-d", DISTRO, "-u", "root", "--", "bash", "-lc", start_cmd]);

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
            warn("diagnose: wsl -d dockwin -u root -- journalctl -u docker --no-pager -n 50");
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
    let stop_cmd = "if [ -d /run/systemd/system ]; then systemctl stop docker.socket docker.service 2>/dev/null || systemctl stop docker 2>/dev/null || true; else service docker stop 2>/dev/null || true; fi";
    let _ = wsl::run(&["-d", DISTRO, "-u", "root", "--", "bash", "-lc", stop_cmd]);
    ok("dockerd stopped (systemd will restart it on next boot unless disabled).");

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
    // behind — and on a partial/failed unregister (lock, distro still busy) the
    // vhdx itself can linger. `install` created this dir, so teardown removes it
    // here to stay symmetric. Best-effort: warn (don't fail) if it can't.
    let install_dir = default_install_dir();
    if install_dir.exists() {
        match fs::remove_dir_all(&install_dir) {
            Ok(()) => {
                ok(&format!("removed install directory {}", install_dir.display()));
                // Drop the now-empty parent (%LOCALAPPDATA%\dockwin); `remove_dir`
                // only succeeds when empty, so a shared/non-empty parent is left be.
                if let Some(parent) = install_dir.parent() {
                    let _ = fs::remove_dir(parent);
                }
            }
            Err(e) => warn(&format!(
                "could not remove {} ({e}); delete it manually",
                install_dir.display()
            )),
        }
    }

    println!("dockwin uninstall complete.");
    Ok(())
}

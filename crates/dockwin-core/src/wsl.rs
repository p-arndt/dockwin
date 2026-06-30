//! Low-level helpers for driving `wsl.exe` from Rust.
//!
//! These mirror the wsl.exe invocations in the original `scripts/*.ps1` but in
//! native Rust. The two non-obvious things this module gets right:
//!
//!   * `wsl.exe --list` (and friends) emit their output as **UTF-16LE** (with a
//!     BOM and interleaved NUL bytes). Decoding those bytes as UTF-8 yields
//!     garbage / empty strings, so [`decode_wsl`] detects and decodes UTF-16LE.
//!   * Files written *into* the distro must use LF line endings; [`write_into_distro`]
//!     normalises CRLF -> LF before piping the content in over stdin (avoids the
//!     `/mnt` path-translation dance the PowerShell installer used).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

/// The dedicated distro name. Reused everywhere (matches the original scripts).
pub const DISTRO: &str = "dockwin";

/// Default base rootfs: the **minimal** Ubuntu 24.04 (noble) base image
/// (~29 MB) instead of the full server cloud image (~216 MB) — far less to
/// download. The WSL-specific tarballs under `cloud-images.ubuntu.com/wsl/...`
/// were removed upstream (404), and the cloud image carries cloud-init + a
/// server payload we don't need; `ubuntu-base` is glibc/apt and works with the
/// official Docker apt repo just the same. It ships **no systemd**, so
/// provisioning installs systemd before the `wsl.conf systemd=true` reboot (see
/// [`crate::ops::install_reporting`]). Pinned to a point release for
/// reproducibility — bump when a newer one ships. Swap back to the cloud image
/// (`https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64-root.tar.xz`)
/// if systemd ever misbehaves on the minimal base.
pub const DEFAULT_ROOTFS_URL: &str =
    "https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release/ubuntu-base-24.04.4-base-amd64.tar.gz";

/// Pinned SHA-256 of [`DEFAULT_ROOTFS_URL`], from Ubuntu's published
/// `SHA256SUMS` next to the image. We verify the download against this before
/// importing it as a *root* WSL distro: HTTPS authenticates the transport but
/// not the bytes at rest (a poisoned mirror, a TLS-intercepting proxy, a stale
/// CDN cache), and a pinned hash baked into the binary closes that gap. MUST be
/// bumped together with `DEFAULT_ROOTFS_URL` whenever the point release changes.
pub const DEFAULT_ROOTFS_SHA256: &str =
    "c1e67ef7b17a6300e136118bd1dc04725009cb376c1aad10abcf8cd453628d58";

/// `CREATE_NO_WINDOW` (winbase.h). Without it, every child process spawned from
/// the GUI — which has no console of its own — briefly flashes a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `Command` for an external program that will NOT pop a console window
/// when spawned from a GUI process. Use this for EVERY external spawn so the
/// GUI stays flicker-free (`wsl.exe`, `curl.exe`, `docker.exe`, ...).
pub fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// The named pipe served by the GUI's relay; the docker context points here.
pub const PIPE_HOST: &str = "npipe:////./pipe/dockwin_engine";

/// Quote `s` as a single POSIX shell word, safe to interpolate into a
/// `bash -lc` script. Wraps the value in single quotes and rewrites any embedded
/// single quote as the standard `'\''` sequence, so the value cannot terminate
/// the quoting and inject further commands. These in-distro scripts run as
/// **root**, so every interpolated path/value must go through this.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Decode bytes emitted by `wsl.exe`.
///
/// `wsl.exe` historically writes UTF-16LE for list-style output. We detect that
/// by the characteristic interleaved-NUL pattern and decode accordingly; raw
/// Linux process output piped through `wsl -d ... -- ...` is UTF-8 and falls
/// through to the UTF-8 path.
pub fn decode_wsl(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    // Strip a UTF-16LE BOM if present.
    let body = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        &bytes[2..]
    } else {
        bytes
    };
    let nul_count = body.iter().filter(|&&b| b == 0).count();
    // For ASCII text encoded as UTF-16LE roughly half the bytes are NUL.
    let looks_utf16 = body.len() >= 2 && nul_count * 2 >= body.len().saturating_sub(2);
    if looks_utf16 {
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(body).into_owned()
    }
}

/// Run `wsl.exe <args>` capturing stdout, returning (success, decoded_stdout).
pub fn capture(args: &[&str]) -> Result<(bool, String)> {
    let out = command("wsl.exe")
        .args(args)
        .output()
        .context("failed to spawn wsl.exe (is WSL installed? try `wsl --install`)")?;
    Ok((out.status.success(), decode_wsl(&out.stdout)))
}

/// Run `wsl.exe <args>` inheriting stdio (for long / interactive operations).
/// Returns whether the process exited successfully.
pub fn run(args: &[&str]) -> Result<bool> {
    let status = command("wsl.exe")
        .args(args)
        .status()
        .context("failed to spawn wsl.exe")?;
    Ok(status.success())
}

/// Run `wsl.exe <args>` inheriting stdio and error on a non-zero exit.
pub fn run_checked(args: &[&str]) -> Result<()> {
    if !run(args)? {
        bail!("`wsl.exe {}` failed", args.join(" "));
    }
    Ok(())
}

/// Run `wsl.exe <args>` capturing stdout line-by-line, invoking `on_line` for
/// each line as it arrives (for live progress from long in-distro commands).
/// stderr is dropped — callers that want it merged should append `2>&1` to the
/// in-distro command so it lands on stdout. Returns whether the child succeeded.
pub fn run_streaming(args: &[&str], on_line: &mut dyn FnMut(&str)) -> Result<bool> {
    use std::io::BufRead;
    let mut child = command("wsl.exe")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn wsl.exe")?;
    if let Some(out) = child.stdout.take() {
        // In-distro process output is UTF-8 (unlike wsl.exe's own UTF-16 lists).
        let reader = std::io::BufReader::new(out);
        for line in reader.lines() {
            match line {
                Ok(l) => on_line(&l),
                Err(_) => break,
            }
        }
    }
    let status = child.wait().context("failed to wait on wsl.exe")?;
    Ok(status.success())
}

/// List registered distro names (or only running ones when `running_only`).
pub fn list_distros(running_only: bool) -> Result<Vec<String>> {
    let mut args = vec!["--list", "--quiet"];
    if running_only {
        args.push("--running");
    }
    let (_, text) = capture(&args)?;
    Ok(text
        .lines()
        .map(|l| l.replace('\0', "").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

pub fn distro_exists(name: &str) -> Result<bool> {
    Ok(list_distros(false)?.iter().any(|d| d == name))
}

pub fn distro_running(name: &str) -> Result<bool> {
    Ok(list_distros(true)?.iter().any(|d| d == name))
}

/// Write `content` to `dest` inside the dockwin distro as root, chmod `mode`.
/// CRLF is normalised to LF so shell/conf files are valid under Linux.
pub fn write_into_distro(content: &str, dest: &str, mode: &str) -> Result<()> {
    let normalized = content.replace("\r\n", "\n");
    // Shell-escape the interpolated values: this runs as root in the distro, so
    // a single quote in `dest`/`mode` must not be able to break out and inject.
    let q_dest = sh_quote(dest);
    let script = format!("cat > {q_dest} && chmod {} {q_dest}", sh_quote(mode));
    let mut child = command("wsl.exe")
        .args(["-d", DISTRO, "-u", "root", "--", "bash", "-lc", &script])
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to spawn wsl.exe to write into the distro")?;
    {
        let mut stdin = child.stdin.take().context("failed to open wsl.exe stdin")?;
        stdin
            .write_all(normalized.as_bytes())
            .context("failed to stream file content into the distro")?;
        // stdin drops here -> EOF, so `cat` finishes.
    }
    let status = child.wait().context("failed to wait on wsl.exe")?;
    if !status.success() {
        bail!("writing {dest} into the distro failed");
    }
    Ok(())
}

/// Best-effort: the on-disk folder WSL records as `name`'s BasePath — the
/// directory that holds its `ext4.vhdx`. Read from the WSL registry so we can
/// tell a healthy *stopped* distro from a BROKEN one whose disk image was
/// deleted out from under it (which would otherwise only fail at cold-boot
/// time, with a cryptic `MountVhd` error). Returns None when the distro or its
/// registry value can't be found — callers treat None as "can't tell".
#[cfg(windows)]
pub fn distro_base_path(name: &str) -> Option<PathBuf> {
    // The Lxss registry holds one subkey per distro, each carrying a
    // `DistributionName` and a `BasePath` value. Dump the tree and return the
    // BasePath of the block whose DistributionName matches `name`.
    let out = command("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss",
            "/s",
        ])
        .output()
        .ok()?;
    // reg.exe emits console-codepage text (not UTF-16 like wsl.exe's lists);
    // paths are ASCII so a lossy UTF-8 decode is fine.
    let text = String::from_utf8_lossy(&out.stdout);

    // A value line looks like: "    <Key>    REG_SZ    <data>".
    let value = |line: &str, key: &str| -> Option<String> {
        let rest = line.trim().strip_prefix(key)?;
        Some(rest.split("REG_SZ").nth(1)?.trim().to_string())
    };

    let mut block_name: Option<String> = None;
    let mut block_base: Option<String> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("HKEY_") {
            // A new key block starts: the previous block is now complete.
            if block_name.as_deref() == Some(name) {
                if let Some(b) = block_base.take() {
                    return Some(strip_extended_prefix(&b));
                }
            }
            block_name = None;
            block_base = None;
            continue;
        }
        if let Some(v) = value(line, "DistributionName") {
            block_name = Some(v);
        } else if let Some(v) = value(line, "BasePath") {
            block_base = Some(v);
        }
    }
    // Evaluate the final block (no trailing key header follows it).
    if block_name.as_deref() == Some(name) {
        if let Some(b) = block_base {
            return Some(strip_extended_prefix(&b));
        }
    }
    None
}

#[cfg(not(windows))]
pub fn distro_base_path(_name: &str) -> Option<PathBuf> {
    None
}

/// Strip the `\\?\` extended-length path prefix WSL stores in BasePath.
#[cfg(windows)]
fn strip_extended_prefix(p: &str) -> PathBuf {
    PathBuf::from(p.strip_prefix(r"\\?\").unwrap_or(p))
}

/// Best-effort query of the in-distro dockerd server version.
/// Returns `Ok(Some(version))` only when dockerd answered.
pub fn docker_server_version() -> Result<Option<String>> {
    let (ok, text) = capture(&[
        "-d",
        DISTRO,
        "-u",
        "root",
        "--",
        "bash",
        "-lc",
        "docker version --format '{{.Server.Version}}' 2>/dev/null",
    ])?;
    let v = text.trim().to_string();
    if ok && !v.is_empty() {
        Ok(Some(v))
    } else {
        Ok(None)
    }
}

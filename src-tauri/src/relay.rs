//! Windows named-pipe -> WSL2 dockerd unix-socket relay.
//!
//! This is the keystone that lets a Windows-native process talk to the dockerd
//! running inside the dedicated "dockwin" WSL2 distro. We host the pipe
//! `\\.\pipe\dockwin_engine` and, for every client connection, spawn
//! `wsl.exe -d dockwin -u root -e socat - UNIX-CONNECT:/var/run/docker.sock`
//! and pump bytes both ways. The Docker HTTP protocol then flows transparently
//! between the pipe client and the daemon's unix socket.
//!
//! Both consumers use this single endpoint:
//!   * the GUI's own bollard client (`docker.rs`, `connect_with_named_pipe`), and
//!   * the stock Windows `docker.exe` via `docker context ... host=npipe:////./pipe/dockwin_engine`.
//!
//! `socat` is installed in the distro by distro/provision-inside.sh.
//!
//! Security: the relay fronts a *root* dockerd that can mount the whole Windows
//! filesystem and run privileged containers, so the pipe must not be reachable
//! by other users or lower-integrity processes. Every pipe instance is created
//! with a SECURITY_DESCRIPTOR whose DACL grants access only to the current user
//! (+ LocalSystem) — see [`security::PipeSecurity`]. If the descriptor can't be
//! built we log loudly and fall back to the default DACL rather than failing to
//! start the relay.

/// The pipe name in Win32 form. bollard addresses the same pipe as
/// `//./pipe/dockwin_engine` (see `docker::DEFAULT_PIPE_ADDR`).
pub const PIPE_NAME: &str = r"\\.\pipe\dockwin_engine";

#[cfg(windows)]
pub async fn run(distro: String) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    // Build the restricted security descriptor once and reuse it for every
    // instance (the DACL must be set on each CreateNamedPipe call, not just the
    // first). `None` => couldn't build it; fall back to the default DACL.
    let security = match security::PipeSecurity::current_user_only() {
        Ok(s) => Some(s),
        Err(e) => {
            log::error!(
                "could not build a restricted DACL for {PIPE_NAME} ({e}); \
                 falling back to the default pipe security"
            );
            None
        }
    };

    // Create one pipe instance, applying the per-user DACL when we have one.
    let make_server = |first: bool| -> std::io::Result<NamedPipeServer> {
        let mut opts = ServerOptions::new();
        opts.first_pipe_instance(first);
        match &security {
            // Safety: `sa` points at a security descriptor owned by `security`,
            // which outlives this synchronous call.
            Some(sec) => {
                let mut sa = sec.attributes();
                unsafe {
                    opts.create_with_security_attributes_raw(
                        PIPE_NAME,
                        &mut sa as *mut _ as *mut std::ffi::c_void,
                    )
                }
            }
            None => opts.create(PIPE_NAME),
        }
    };

    // Claim the pipe name with the first instance, then keep a server instance
    // listening at all times (create the next one as soon as a client connects).
    let mut server = make_server(true)?;

    log::info!("named-pipe relay listening on {PIPE_NAME}");

    loop {
        // Wait for a client (bollard or docker.exe) to connect.
        server.connect().await?;
        let connected = server;

        // Immediately stand up the next instance so we never miss a connection.
        server = make_server(false)?;

        let distro = distro.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(connected, &distro).await {
                // Per-connection failures are normal (client hung up, daemon
                // down, socat missing); keep the relay alive.
                log::debug!("relay connection ended: {e}");
            }
        });
    }
}

#[cfg(windows)]
async fn handle_connection(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    distro: &str,
) -> std::io::Result<()> {
    use std::process::Stdio;
    use tokio::process::Command;

    /// Don't flash a console window when spawning wsl.exe from the GUI.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-d",
            distro,
            "-u",
            "root",
            "-e",
            "socat",
            "-",
            "UNIX-CONNECT:/var/run/docker.sock",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("relay: no child stdin"))?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("relay: no child stdout"))?;

    // Pipe is full-duplex; split it so we can copy both directions concurrently
    // (the child exposes stdin/stdout as two separate half-streams).
    let (mut pipe_reader, mut pipe_writer) = tokio::io::split(pipe);

    let client_to_daemon = tokio::io::copy(&mut pipe_reader, &mut child_stdin);
    let daemon_to_client = tokio::io::copy(&mut child_stdout, &mut pipe_writer);

    // Whichever side closes first ends this connection.
    tokio::select! {
        r = client_to_daemon => { r?; }
        r = daemon_to_client => { r?; }
    }

    // Tear down the socat bridge for this connection.
    let _ = child.kill().await;
    Ok(())
}

/// Builds the named pipe's security descriptor: a DACL granting access only to
/// the current user and LocalSystem, denying everyone else (other users, lower-
/// integrity processes). Implemented with raw Win32 because neither tokio's
/// `ServerOptions` nor the standard library exposes a safe builder for this.
#[cfg(windows)]
mod security {
    use std::ffi::c_void;
    use std::io;
    use std::mem::size_of;
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    /// Owns a self-relative SECURITY_DESCRIPTOR (allocated by Windows inside
    /// `ConvertStringSecurityDescriptorToSecurityDescriptorW`) and frees it on
    /// drop. Hand out a fresh [`SECURITY_ATTRIBUTES`] pointing at it for each
    /// `CreateNamedPipe` call via [`Self::attributes`].
    pub struct PipeSecurity {
        psd: *mut c_void,
    }

    // The descriptor is owned, immutable after construction, and only ever read
    // to fill a SECURITY_ATTRIBUTES, so it is safe to move and share across
    // threads (the relay future is spawned onto the tokio runtime and must be
    // `Send`; the create closure holds a `&PipeSecurity` across an `.await`,
    // which additionally requires `Sync`).
    unsafe impl Send for PipeSecurity {}
    unsafe impl Sync for PipeSecurity {}

    impl PipeSecurity {
        /// Build a descriptor granting full control to the current user and
        /// LocalSystem only.
        pub fn current_user_only() -> io::Result<Self> {
            unsafe { build() }
        }

        /// A SECURITY_ATTRIBUTES referencing this descriptor. Valid only while
        /// `self` is alive (which the relay guarantees for each create call).
        pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
            SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.psd,
                bInheritHandle: 0,
            }
        }
    }

    impl Drop for PipeSecurity {
        fn drop(&mut self) {
            if !self.psd.is_null() {
                unsafe {
                    LocalFree(self.psd);
                }
            }
        }
    }

    /// Closes a token handle on scope exit so early returns don't leak it.
    struct TokenGuard(HANDLE);
    impl Drop for TokenGuard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    unsafe fn build() -> io::Result<PipeSecurity> {
        // 1. Open this process's token to learn the current user's SID.
        let mut token: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _token = TokenGuard(token);

        // 2. Query the TokenUser info (size probe, then the real read).
        let mut len: u32 = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut len);
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buf = vec![0u8; len as usize];
        if GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut c_void, len, &mut len)
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        let token_user = &*(buf.as_ptr() as *const TOKEN_USER);

        // 3. Stringify the SID for use in the SDDL.
        let mut sid_str: *mut u16 = ptr::null_mut();
        if ConvertSidToStringSidW(token_user.User.Sid, &mut sid_str) == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = wide_to_string(sid_str);
        let _ = LocalFree(sid_str as *mut c_void);

        // 4. DACL: protected (no inherited ACEs), allow GENERIC_ALL to the user
        //    and LocalSystem (SY) only — everyone else is implicitly denied.
        let sddl = format!("D:P(A;;GA;;;{sid})(A;;GA;;;SY)");
        let sddl_w: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();

        // 5. Turn the SDDL into a real security descriptor.
        let mut psd: *mut c_void = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_w.as_ptr(),
            SDDL_REVISION_1,
            &mut psd,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }

        Ok(PipeSecurity { psd })
    }

    /// Read a NUL-terminated wide string into an owned `String`.
    unsafe fn wide_to_string(p: *const u16) -> String {
        let mut len = 0usize;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;
        use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

        /// Round-trip the built descriptor back to SDDL and assert its DACL
        /// grants only the current user + LocalSystem — and crucially does NOT
        /// grant any of the broad groups (Everyone `WD`, Authenticated Users
        /// `AU`, Built-in Users `BU`). This is the security property that makes
        /// the relay pipe unreachable by other local principals.
        #[test]
        fn dacl_restricts_to_current_user_and_system() {
            let sec = PipeSecurity::current_user_only().expect("build descriptor");

            let sddl = unsafe {
                let mut out: *mut u16 = ptr::null_mut();
                let ok = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    sec.psd,
                    SDDL_REVISION_1,
                    DACL_SECURITY_INFORMATION,
                    &mut out,
                    ptr::null_mut(),
                );
                assert_ne!(ok, 0, "failed to stringify the descriptor");
                let s = wide_to_string(out);
                let _ = LocalFree(out as *mut c_void);
                s
            };

            // Protected DACL, LocalSystem present, no broad principals.
            assert!(sddl.contains("D:P"), "DACL not protected: {sddl}");
            assert!(sddl.contains(";;;SY)"), "LocalSystem missing: {sddl}");
            for broad in [";;;WD)", ";;;AU)", ";;;BU)", ";;;WD;", ";;;AU;"] {
                assert!(
                    !sddl.contains(broad),
                    "DACL unexpectedly grants a broad principal ({broad}): {sddl}"
                );
            }
            // The current user's own SID should be granted.
            assert!(sddl.contains("(A;;"), "no allow ACE present: {sddl}");
        }
    }
}

/// Non-Windows stub so the crate still type-checks on CI. dockwin targets
/// Windows 11; there is no WSL named pipe elsewhere.
#[cfg(not(windows))]
pub async fn run(_distro: String) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "the named-pipe relay is only available on Windows",
    ))
}

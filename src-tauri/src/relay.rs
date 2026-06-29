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
//! Security note (TODO): the pipe currently uses the default DACL. A future pass
//! should restrict it to the current user via a SECURITY_DESCRIPTOR, matching
//! the "ACL'd to the current user only" design goal.

/// The pipe name in Win32 form. bollard addresses the same pipe as
/// `//./pipe/dockwin_engine` (see `docker::DEFAULT_PIPE_ADDR`).
pub const PIPE_NAME: &str = r"\\.\pipe\dockwin_engine";

#[cfg(windows)]
pub async fn run(distro: String) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    // Claim the pipe name with the first instance, then keep a server instance
    // listening at all times (create the next one as soon as a client connects).
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(PIPE_NAME)?;

    log::info!("named-pipe relay listening on {PIPE_NAME}");

    loop {
        // Wait for a client (bollard or docker.exe) to connect.
        server.connect().await?;
        let connected = server;

        // Immediately stand up the next instance so we never miss a connection.
        server = ServerOptions::new().create(PIPE_NAME)?;

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

/// Non-Windows stub so the crate still type-checks on CI. dockwin targets
/// Windows 11; there is no WSL named pipe elsewhere.
#[cfg(not(windows))]
pub async fn run(_distro: String) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "the named-pipe relay is only available on Windows",
    ))
}

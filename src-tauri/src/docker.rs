//! Docker Engine API client for the dockwin GUI.
//!
//! This module owns the [`bollard`] connection to dockerd. Per the dockwin
//! architecture, the PRIMARY wiring is a Windows named pipe
//! `\\.\pipe\dockwin_engine` that `dockwin-core` hosts and relays into the
//! WSL2 distro's unix socket. bollard speaks npipe natively via
//! [`Docker::connect_with_named_pipe`]. A loopback TCP endpoint
//! (`tcp://127.0.0.1:2375`) is the explicitly-insecure FALLBACK.
//!
//! Everything here returns plain, serde-serializable DTOs so the Tauri command
//! layer (`commands.rs`) can hand them straight to the web frontend without the
//! frontend ever depending on bollard's types.

use std::time::Duration;

use bollard::container::{
    ListContainersOptions, LogsOptions, RemoveContainerOptions, RestartContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::image::ListImagesOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
use serde::Serialize;

/// Named pipe that `dockwin-core` hosts and relays into the distro.
/// In bollard/npipe form the leading `\\.\pipe\` becomes `//./pipe/`.
pub const DEFAULT_PIPE_ADDR: &str = "//./pipe/dockwin_engine";

/// Loopback TCP fallback (INSECURE: unauthenticated, reachable by any local
/// process / other WSL distro). Only used when explicitly selected.
pub const DEFAULT_TCP_ADDR: &str = "tcp://127.0.0.1:2375";

/// Connection/handshake timeout in seconds for the initial bollard connect.
const CONNECT_TIMEOUT_SECS: u64 = 8;

/// How dockwin should reach dockerd.
#[derive(Debug, Clone)]
pub enum Transport {
    /// Primary: Windows named pipe relayed by dockwin-core.
    NamedPipe(String),
    /// Fallback: loopback TCP (insecure).
    Tcp(String),
}

impl Default for Transport {
    fn default() -> Self {
        Transport::NamedPipe(DEFAULT_PIPE_ADDR.to_string())
    }
}

/// Errors surfaced to the command layer. Kept stringly-typed at the boundary so
/// the frontend gets a readable message regardless of the underlying source.
#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("failed to connect to the dockwin engine: {0}")]
    Connect(String),
    #[error("docker engine call failed: {0}")]
    Api(String),
}

impl From<bollard::errors::Error> for DockerError {
    fn from(e: bollard::errors::Error) -> Self {
        DockerError::Api(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DockerError>;

// ---------------------------------------------------------------------------
// DTOs (serde-serializable, frontend-facing)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct PortMappingDto {
    /// Container-side port.
    pub private_port: u16,
    /// Host-side published port, if any.
    pub public_port: Option<u16>,
    /// Bind IP on the host (e.g. "0.0.0.0" wildcard vs "127.0.0.1").
    pub ip: Option<String>,
    pub protocol: String,
    /// True when published on a wildcard address (reachable at Windows
    /// localhost under default NAT). 127.0.0.1-bound publishes are NOT
    /// forwarded — the frontend surfaces this caveat.
    pub forwarded_to_localhost: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerDto {
    pub id: String,
    /// Primary display name (leading slash stripped).
    pub name: String,
    pub image: String,
    /// e.g. "running", "exited".
    pub state: String,
    /// Human status string, e.g. "Up 3 minutes".
    pub status: String,
    /// Compose project label, if this container belongs to a compose project.
    pub compose_project: Option<String>,
    pub ports: Vec<PortMappingDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageDto {
    pub id: String,
    /// Repo:tag entries; may be empty for dangling images.
    pub tags: Vec<String>,
    pub size: i64,
    /// Unix seconds.
    pub created: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionDto {
    pub version: Option<String>,
    pub api_version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub kernel_version: Option<String>,
}

/// A single chunk of container log output, tagged by stream.
#[derive(Debug, Clone, Serialize)]
pub struct LogChunkDto {
    /// "stdout" | "stderr" | "stdin" | "console".
    pub stream: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Thin wrapper over a connected [`bollard::Docker`] handle.
#[derive(Clone)]
pub struct DockerClient {
    inner: Docker,
}

impl DockerClient {
    /// Connect using the given transport and verify reachability with a ping.
    pub async fn connect(transport: Transport) -> Result<Self> {
        let docker = match transport {
            Transport::NamedPipe(addr) => connect_named_pipe(&addr)?,
            Transport::Tcp(addr) => connect_tcp(&addr)?,
        };

        // Verify the engine is actually reachable before reporting ready.
        docker
            .ping()
            .await
            .map_err(|e| DockerError::Connect(e.to_string()))?;

        Ok(Self { inner: docker })
    }

    /// Try the primary named pipe, then fall back to loopback TCP if requested.
    pub async fn connect_default(allow_tcp_fallback: bool) -> Result<Self> {
        match Self::connect(Transport::default()).await {
            Ok(c) => Ok(c),
            Err(primary_err) if allow_tcp_fallback => {
                log::warn!(
                    "named-pipe connect failed ({primary_err}); trying insecure TCP fallback"
                );
                Self::connect(Transport::Tcp(DEFAULT_TCP_ADDR.to_string())).await
            }
            Err(e) => Err(e),
        }
    }

    /// `docker version`.
    pub async fn version(&self) -> Result<VersionDto> {
        let v = self.inner.version().await?;
        Ok(VersionDto {
            version: v.version,
            api_version: v.api_version,
            os: v.os,
            arch: v.arch,
            kernel_version: v.kernel_version,
        })
    }

    /// List containers (all by default, including stopped ones).
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerDto>> {
        let opts = ListContainersOptions::<String> {
            all,
            ..Default::default()
        };
        let summaries = self.inner.list_containers(Some(opts)).await?;

        let out = summaries
            .into_iter()
            .map(|c| {
                let name = c
                    .names
                    .as_ref()
                    .and_then(|n| n.first().cloned())
                    .map(|n| n.trim_start_matches('/').to_string())
                    .unwrap_or_default();

                let compose_project = c
                    .labels
                    .as_ref()
                    .and_then(|l| l.get("com.docker.compose.project").cloned());

                let ports = c
                    .ports
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| {
                        let ip = p.ip.clone();
                        // Wildcard bind (empty/0.0.0.0/::) forwards to Windows
                        // localhost under NAT; explicit 127.0.0.1 does not.
                        let forwarded = match ip.as_deref() {
                            None | Some("") | Some("0.0.0.0") | Some("::") => p.public_port.is_some(),
                            _ => false,
                        };
                        PortMappingDto {
                            private_port: p.private_port,
                            public_port: p.public_port,
                            ip,
                            protocol: p
                                .typ
                                .map(|t| format!("{t:?}").to_lowercase())
                                .unwrap_or_else(|| "tcp".to_string()),
                            forwarded_to_localhost: forwarded,
                        }
                    })
                    .collect();

                ContainerDto {
                    id: c.id.unwrap_or_default(),
                    name,
                    image: c.image.unwrap_or_default(),
                    state: c.state.unwrap_or_default(),
                    status: c.status.unwrap_or_default(),
                    compose_project,
                    ports,
                }
            })
            .collect();

        Ok(out)
    }

    /// Start a container by id or name.
    pub async fn start_container(&self, id: &str) -> Result<()> {
        self.inner
            .start_container(id, None::<StartContainerOptions<String>>)
            .await?;
        Ok(())
    }

    /// Stop a container (graceful, with an optional timeout in seconds).
    pub async fn stop_container(&self, id: &str, timeout_secs: Option<i64>) -> Result<()> {
        let opts = timeout_secs.map(|t| StopContainerOptions { t });
        self.inner.stop_container(id, opts).await?;
        Ok(())
    }

    /// Restart a container.
    pub async fn restart_container(&self, id: &str, timeout_secs: Option<isize>) -> Result<()> {
        let opts = timeout_secs.map(|t| RestartContainerOptions { t });
        self.inner.restart_container(id, opts).await?;
        Ok(())
    }

    /// Remove a container (force + volume removal optional).
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<()> {
        let opts = RemoveContainerOptions {
            force,
            v: false,
            link: false,
        };
        self.inner.remove_container(id, Some(opts)).await?;
        Ok(())
    }

    /// List images.
    pub async fn list_images(&self, all: bool) -> Result<Vec<ImageDto>> {
        let opts = ListImagesOptions::<String> {
            all,
            ..Default::default()
        };
        let images = self.inner.list_images(Some(opts)).await?;

        let out = images
            .into_iter()
            .map(|i| ImageDto {
                id: i.id,
                tags: i.repo_tags,
                size: i.size,
                created: i.created,
            })
            .collect();

        Ok(out)
    }

    /// Fetch a bounded snapshot of container logs (non-following).
    ///
    /// For live/streaming logs the command layer should instead drive
    /// [`Self::stream_logs`] and forward chunks over a Tauri channel. This
    /// helper is the simple "tail N lines once" path.
    pub async fn logs_tail(&self, id: &str, tail: u32) -> Result<Vec<LogChunkDto>> {
        let opts = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            follow: false,
            timestamps: false,
            tail: tail.to_string(),
            ..Default::default()
        };

        let mut stream = self.inner.logs(id, Some(opts));
        let mut chunks = Vec::new();
        while let Some(next) = stream.next().await {
            let output = next?;
            chunks.push(log_output_to_dto(output));
        }
        Ok(chunks)
    }

    /// Borrow the raw bollard handle for advanced callers (e.g. streaming logs,
    /// stats, exec) that the command layer wires to Tauri channels/events.
    pub fn inner(&self) -> &Docker {
        &self.inner
    }
}

/// Convert a bollard `LogOutput` frame into a frontend DTO.
fn log_output_to_dto(output: bollard::container::LogOutput) -> LogChunkDto {
    use bollard::container::LogOutput::*;
    let (stream, bytes) = match output {
        StdOut { message } => ("stdout", message),
        StdErr { message } => ("stderr", message),
        StdIn { message } => ("stdin", message),
        Console { message } => ("console", message),
    };
    LogChunkDto {
        stream: stream.to_string(),
        message: String::from_utf8_lossy(&bytes).to_string(),
    }
}

// ---------------------------------------------------------------------------
// Transport-specific connect helpers
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn connect_named_pipe(addr: &str) -> Result<Docker> {
    // bollard's named-pipe transport is Windows-only and uses the same
    // API version negotiation as the other transports.
    Docker::connect_with_named_pipe(addr, CONNECT_TIMEOUT_SECS, bollard::API_DEFAULT_VERSION)
        .map_err(|e| DockerError::Connect(e.to_string()))
}

#[cfg(not(windows))]
fn connect_named_pipe(_addr: &str) -> Result<Docker> {
    // dockwin targets Windows 11; this branch exists only so the crate still
    // type-checks on non-Windows CI. TODO: route to unix socket for dev.
    Err(DockerError::Connect(
        "named-pipe transport is only available on Windows".to_string(),
    ))
}

fn connect_tcp(addr: &str) -> Result<Docker> {
    // INSECURE fallback. Loopback only — never expose beyond 127.0.0.1.
    let _ = Duration::from_secs(CONNECT_TIMEOUT_SECS); // keep import meaningful
    Docker::connect_with_http(addr, CONNECT_TIMEOUT_SECS, bollard::API_DEFAULT_VERSION)
        .map_err(|e| DockerError::Connect(e.to_string()))
}

//! Tauri command layer.
//!
//! Each `#[tauri::command]` here is a thin async wrapper over [`crate::docker`]
//! (the Docker Engine API) and [`crate::wsl`] (engine distro lifecycle). The
//! frontend talks ONLY to these commands; it never sees bollard or wsl details.
//! All commands return `Result<T, String>` so errors surface as readable strings
//! on the JS side.
//!
//! Command names are the IPC contract with `src/lib/api.ts`. The fuller engine
//! lifecycle (provision/teardown) and the named-pipe relay live in the
//! PowerShell installer and, later, `dockwin-core`; the GUI backend connects to
//! an already-running engine and can start/stop the daemon.

use serde::Serialize;
use tauri::Emitter;
use tokio::sync::Mutex;

use crate::docker::{ContainerDto, DockerClient, ImageDto, LogChunkDto, VersionDto};

/// Shared application state managed by Tauri. Holds the (lazily established)
/// Docker connection so we connect once and reuse it across commands.
#[derive(Default)]
pub struct AppState {
    client: Mutex<Option<DockerClient>>,
    /// Whether to allow the insecure loopback-TCP fallback when the named pipe
    /// is unreachable. Off by default.
    allow_tcp_fallback: Mutex<bool>,
}

impl AppState {
    /// Get the cached client, connecting on first use.
    pub(crate) async fn get_client(&self) -> Result<DockerClient, String> {
        let mut guard = self.client.lock().await;
        if let Some(c) = guard.as_ref() {
            return Ok(c.clone());
        }
        let allow_tcp = *self.allow_tcp_fallback.lock().await;
        let backend = dockwin_core::backend::detect();
        let primary: crate::docker::Transport = backend.connection().into();
        let fallback = if allow_tcp {
            backend.fallback_connection().map(Into::into)
        } else {
            None
        };
        let client = DockerClient::connect_with_fallback(primary, fallback)
            .await
            .map_err(|e| e.to_string())?;
        *guard = Some(client.clone());
        Ok(client)
    }

    /// Drop any cached connection (forces a reconnect on next call).
    pub(crate) async fn reset(&self) {
        *self.client.lock().await = None;
    }
}

/// Coarse engine state for the frontend's status indicator.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EngineStatusDto {
    /// "running" | "stopped" | "not_provisioned" | "unreachable".
    pub status: String,
    /// Populated when running.
    pub version: Option<VersionDto>,
    /// Error/context detail when not running.
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Engine status + lifecycle
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn engine_status(state: tauri::State<'_, AppState>) -> Result<EngineStatusDto, String> {
    // If the dedicated distro isn't even registered, the engine is not
    // provisioned — a distinct state from "stopped".
    let engine_state = tokio::task::spawn_blocking(|| {
        dockwin_core::backend::detect()
            .state()
            .unwrap_or(dockwin_core::ops::EngineState::NotProvisioned)
    })
    .await
    .unwrap_or(dockwin_core::ops::EngineState::NotProvisioned);
    if matches!(engine_state, dockwin_core::ops::EngineState::NotProvisioned) {
        return Ok(EngineStatusDto {
            status: "not_provisioned".to_string(),
            version: None,
            detail: Some(format!(
                "WSL distro '{}' not found — provision the engine first",
                dockwin_core::wsl::DISTRO
            )),
        });
    }

    if matches!(engine_state, dockwin_core::ops::EngineState::Broken) {
        return Ok(EngineStatusDto {
            status: "broken".to_string(),
            version: None,
            detail: Some(format!(
                "WSL distro '{}' is registered but its disk image is missing — repair to reset it, then reprovision",
                dockwin_core::wsl::DISTRO
            )),
        });
    }

    match state.get_client().await {
        Ok(client) => match client.version().await {
            Ok(version) => Ok(EngineStatusDto {
                status: "running".to_string(),
                version: Some(version),
                detail: None,
            }),
            Err(e) => {
                // We connected but the call failed — treat the cached handle as
                // stale and report unreachable.
                state.reset().await;
                Ok(EngineStatusDto {
                    status: "unreachable".to_string(),
                    version: None,
                    detail: Some(e.to_string()),
                })
            }
        },
        // Distro exists but we can't reach dockerd (daemon down, or the
        // named-pipe relay isn't running yet).
        Err(e) => Ok(EngineStatusDto {
            status: "stopped".to_string(),
            version: None,
            detail: Some(e),
        }),
    }
}

#[tauri::command]
pub async fn engine_version(state: tauri::State<'_, AppState>) -> Result<VersionDto, String> {
    let client = state.get_client().await?;
    client.version().await.map_err(|e| e.to_string())
}

/// Boot the engine distro and start dockerd. Forces a fresh connection after.
#[tauri::command]
pub async fn engine_start(state: tauri::State<'_, AppState>) -> Result<(), String> {
    tokio::task::spawn_blocking(|| dockwin_core::backend::detect().start(60))
        .await
        .map_err(|e| format!("engine_start task failed: {e}"))?
        .map_err(|e| format!("{e:#}"))?;
    state.reset().await;
    Ok(())
}

/// Stop dockerd inside the engine distro. Forces a reconnect on next call.
#[tauri::command]
pub async fn engine_stop(state: tauri::State<'_, AppState>) -> Result<(), String> {
    tokio::task::spawn_blocking(|| dockwin_core::backend::detect().stop(false))
        .await
        .map_err(|e| format!("engine_stop task failed: {e}"))?
        .map_err(|e| format!("{e:#}"))?;
    state.reset().await;
    Ok(())
}

/// One provisioning progress update, forwarded to the frontend as the
/// `engine://provision` event so it can drive a progress bar + live log.
#[derive(Clone, Serialize)]
pub struct ProvisionProgressDto {
    pub phase: String,
    pub message: String,
    pub pct: f32,
    /// "step" | "info" | "warn" | "error".
    pub level: String,
    /// True on the final event (success or failure).
    pub done: bool,
    /// Set on the terminal failure event.
    pub error: Option<String>,
}

/// Provision the dedicated WSL2 engine distro end-to-end: download/import the
/// rootfs, install dockerd + socat, write wsl.conf, wire the docker context.
/// Long-running (minutes). `enable_tcp` turns on the insecure loopback fallback.
/// Emits `engine://provision` progress events throughout for the GUI's bar.
#[tauri::command]
pub async fn engine_provision(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enable_tcp: Option<bool>,
) -> Result<(), String> {
    let enable_tcp = enable_tcp.unwrap_or(false);
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        let report = move |p: dockwin_core::ops::Progress| {
            let _ = app2.emit(
                "engine://provision",
                ProvisionProgressDto {
                    phase: p.phase.to_string(),
                    message: p.message,
                    pct: p.pct,
                    level: p.level.to_string(),
                    done: false,
                    error: None,
                },
            );
        };
        dockwin_core::backend::detect().install(
            dockwin_core::ops::InstallOpts {
                enable_tcp,
                ..Default::default()
            },
            &report,
        )
    })
    .await
    .map_err(|e| format!("engine_provision task failed: {e}"))?;

    match result {
        Ok(()) => {
            let _ = app.emit(
                "engine://provision",
                ProvisionProgressDto {
                    phase: "done".into(),
                    message: "dockwin engine ready.".into(),
                    pct: 100.0,
                    level: "step".into(),
                    done: true,
                    error: None,
                },
            );
            state.reset().await;
            Ok(())
        }
        Err(e) => {
            let msg = format!("{e:#}");
            let _ = app.emit(
                "engine://provision",
                ProvisionProgressDto {
                    phase: "error".into(),
                    message: msg.clone(),
                    pct: 100.0,
                    level: "error".into(),
                    done: true,
                    error: Some(msg.clone()),
                },
            );
            Err(msg)
        }
    }
}

/// Tear down the engine distro (`wsl --unregister`) and remove docker context(s).
/// `backup` exports the distro to a .tar first. The GUI shows its own
/// confirmation dialog, so this runs non-interactively.
#[tauri::command]
pub async fn engine_teardown(
    state: tauri::State<'_, AppState>,
    backup: Option<bool>,
) -> Result<(), String> {
    let backup = backup.unwrap_or(false);
    tokio::task::spawn_blocking(move || dockwin_core::backend::detect().uninstall(backup, None, true))
        .await
        .map_err(|e| format!("engine_teardown task failed: {e}"))?
        .map_err(|e| format!("{e:#}"))?;
    state.reset().await;
    Ok(())
}

/// Reset a broken / dangling engine registration (`wsl --unregister` a distro
/// whose disk image is gone) so the first-run provision flow can start fresh.
#[tauri::command]
pub async fn engine_repair(state: tauri::State<'_, AppState>) -> Result<(), String> {
    tokio::task::spawn_blocking(|| dockwin_core::backend::detect().repair())
        .await
        .map_err(|e| format!("engine_repair task failed: {e}"))?
        .map_err(|e| format!("{e:#}"))?;
    state.reset().await;
    Ok(())
}

/// Toggle the insecure loopback-TCP fallback at runtime.
#[tauri::command]
pub async fn set_tcp_fallback(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    *state.allow_tcp_fallback.lock().await = enabled;
    state.reset().await; // force a fresh connect honoring the new policy
    Ok(())
}

// ---------------------------------------------------------------------------
// Containers
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn container_list(
    state: tauri::State<'_, AppState>,
    all: Option<bool>,
) -> Result<Vec<ContainerDto>, String> {
    let client = state.get_client().await?;
    client
        .list_containers(all.unwrap_or(true))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn container_start(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.get_client().await?;
    client.start_container(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn container_stop(
    state: tauri::State<'_, AppState>,
    id: String,
    timeout_secs: Option<i64>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .stop_container(&id, timeout_secs)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn container_restart(
    state: tauri::State<'_, AppState>,
    id: String,
    timeout_secs: Option<isize>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .restart_container(&id, timeout_secs)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn container_remove(
    state: tauri::State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .remove_container(&id, force.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

/// Tail a bounded number of log lines once (non-following snapshot).
///
/// TODO: add a streaming variant that takes a `tauri::ipc::Channel<LogChunkDto>`
/// and forwards `DockerClient::inner().logs(..)` frames live for the GUI log
/// viewer / exec terminal. Kept out of MVP wrapper to keep the surface small.
#[tauri::command]
pub async fn container_logs(
    state: tauri::State<'_, AppState>,
    id: String,
    tail: Option<u32>,
) -> Result<Vec<LogChunkDto>, String> {
    let client = state.get_client().await?;
    client
        .logs_tail(&id, tail.unwrap_or(200))
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn image_list(
    state: tauri::State<'_, AppState>,
    all: Option<bool>,
) -> Result<Vec<ImageDto>, String> {
    let client = state.get_client().await?;
    client
        .list_images(all.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Compose
// ---------------------------------------------------------------------------

/// One line of `docker compose` output, forwarded as a `compose://output` event.
#[derive(Clone, Serialize)]
pub struct ComposeOutputDto {
    pub line: String,
}

/// `docker compose up` for the given Windows compose-file path, run INSIDE the
/// dockwin engine (the Windows `docker compose` targets Docker Desktop's pipe,
/// which dockwin users don't have). Streams output via `compose://output`.
/// Detached by default; `foreground=true` keeps it attached.
#[tauri::command]
pub async fn compose_up(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    foreground: Option<bool>,
) -> Result<(), String> {
    let detach = !foreground.unwrap_or(false);
    let app2 = app.clone();
    let res = tokio::task::spawn_blocking(move || {
        let file = std::path::PathBuf::from(path);
        let mut emit = |line: &str| {
            let _ = app2.emit("compose://output", ComposeOutputDto { line: line.to_string() });
        };
        dockwin_core::backend::detect().compose_up(&file, detach, &mut emit)
    })
    .await
    .map_err(|e| format!("compose_up task failed: {e}"))?;
    match res {
        Ok(true) => {
            state.reset().await;
            Ok(())
        }
        Ok(false) => Err("docker compose up reported an error (see the log)".into()),
        Err(e) => Err(format!("{e:#}")),
    }
}

/// `docker compose down` for the given compose-file path, run inside the engine.
#[tauri::command]
pub async fn compose_down(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let app2 = app.clone();
    let res = tokio::task::spawn_blocking(move || {
        let file = std::path::PathBuf::from(path);
        let mut emit = |line: &str| {
            let _ = app2.emit("compose://output", ComposeOutputDto { line: line.to_string() });
        };
        dockwin_core::backend::detect().compose_down(&file, &mut emit)
    })
    .await
    .map_err(|e| format!("compose_down task failed: {e}"))?;
    match res {
        Ok(true) => {
            state.reset().await;
            Ok(())
        }
        Ok(false) => Err("docker compose down reported an error (see the log)".into()),
        Err(e) => Err(format!("{e:#}")),
    }
}

/// Run an arbitrary `docker compose <args>` for `path` inside the engine,
/// streaming combined output via `compose://output`. Shared by the build/pull/
/// restart/logs commands below so they all behave identically to up/down.
async fn compose_action(
    app: tauri::AppHandle,
    state: &AppState,
    path: String,
    args: Vec<String>,
    what: &'static str,
) -> Result<(), String> {
    let app2 = app.clone();
    let res = tokio::task::spawn_blocking(move || {
        let file = std::path::PathBuf::from(path);
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut emit = |line: &str| {
            let _ = app2.emit("compose://output", ComposeOutputDto { line: line.to_string() });
        };
        dockwin_core::backend::detect().compose_run(&file, &argv, &mut emit)
    })
    .await
    .map_err(|e| format!("compose_{what} task failed: {e}"))?;
    match res {
        Ok(true) => {
            state.reset().await;
            Ok(())
        }
        Ok(false) => Err(format!("docker compose {what} reported an error (see the log)")),
        Err(e) => Err(format!("{e:#}")),
    }
}

/// `docker compose build` for `path` (rebuild service images).
#[tauri::command]
pub async fn compose_build(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    compose_action(app, &state, path, vec!["build".into()], "build").await
}

/// `docker compose pull` for `path` (pull service images).
#[tauri::command]
pub async fn compose_pull(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    compose_action(app, &state, path, vec!["pull".into()], "pull").await
}

/// `docker compose restart` for `path`.
#[tauri::command]
pub async fn compose_restart(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    compose_action(app, &state, path, vec!["restart".into()], "restart").await
}

/// `docker compose logs` (bounded tail snapshot) for `path`.
#[tauri::command]
pub async fn compose_logs(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    tail: Option<u32>,
) -> Result<(), String> {
    let tail = tail.unwrap_or(200).to_string();
    let args = vec![
        "logs".into(),
        "--no-color".into(),
        "--tail".into(),
        tail,
    ];
    compose_action(app, &state, path, args, "logs").await
}

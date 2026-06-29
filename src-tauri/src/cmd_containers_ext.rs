//! Extended container commands for dockwin: inspect / stats / top / rename /
//! pause / unpause.
//!
//! The lifecycle basics (start/stop/restart/remove) live in `docker.rs` +
//! `commands.rs`. This module adds the "details drawer" surface the frontend
//! opens on a single container: a full JSON inspect, a one-shot resource-usage
//! snapshot, the in-container process table, plus rename and pause toggles.
//!
//! All Docker work is added as NEW methods on [`crate::docker::DockerClient`]
//! via its public `inner()` bollard handle, returning the crate-wide
//! `crate::docker::Result`. The `#[tauri::command]` wrappers mirror the style in
//! `commands.rs`: grab a client from [`AppState`], call through, and stringify
//! any error for the frontend.

use bollard::container::{
    InspectContainerOptions, RenameContainerOptions, StatsOptions, TopOptions,
};
use futures_util::stream::StreamExt;
use serde::Serialize;

use crate::commands::AppState;
use crate::docker::DockerClient;

// ---------------------------------------------------------------------------
// DTOs (serde-serializable, frontend-facing)
// ---------------------------------------------------------------------------

/// In-container process listing (output of `docker top`).
#[derive(Debug, Clone, Serialize)]
pub struct ContainerTopDto {
    /// Column headers (e.g. `UID`, `PID`, `CMD`).
    pub titles: Vec<String>,
    /// One row per process; each row aligns with `titles`.
    pub processes: Vec<Vec<String>>,
}

/// A single resource-usage snapshot for a container, already reduced to the
/// handful of numbers the UI renders. Mirrors `docker stats` for one sample.
#[derive(Debug, Clone, Serialize)]
pub struct ContainerStatsDto {
    /// CPU usage as a percentage (can exceed 100 on multi-core).
    pub cpu_pct: f64,
    /// Memory in use (cache-excluded), in bytes.
    pub mem_usage: i64,
    /// Memory limit, in bytes.
    pub mem_limit: i64,
    /// `mem_usage / mem_limit * 100`.
    pub mem_pct: f64,
    /// Total bytes received across all networks.
    pub net_rx: i64,
    /// Total bytes transmitted across all networks.
    pub net_tx: i64,
    /// Total bytes read from block devices.
    pub blk_read: i64,
    /// Total bytes written to block devices.
    pub blk_write: i64,
    /// Number of processes/threads.
    pub pids: i64,
}

// ---------------------------------------------------------------------------
// DockerClient extension methods
// ---------------------------------------------------------------------------

impl DockerClient {
    /// `docker inspect` for one container, returned as pretty JSON.
    ///
    /// Serializing the bollard `ContainerInspectResponse` keeps the frontend
    /// free of bollard types; it just renders the text. Falls back to the debug
    /// representation if (somehow) JSON serialization fails.
    pub async fn inspect_container_pretty(&self, id: &str) -> crate::docker::Result<String> {
        let resp = self
            .inner()
            .inspect_container(id, None::<InspectContainerOptions>)
            .await?;
        let pretty = serde_json::to_string_pretty(&resp).unwrap_or_else(|_| format!("{resp:#?}"));
        Ok(pretty)
    }

    /// Rename a container.
    pub async fn rename_container(&self, id: &str, name: &str) -> crate::docker::Result<()> {
        self.inner()
            .rename_container(id, RenameContainerOptions { name })
            .await?;
        Ok(())
    }

    /// Pause all processes within a container.
    pub async fn pause_container(&self, id: &str) -> crate::docker::Result<()> {
        self.inner().pause_container(id).await?;
        Ok(())
    }

    /// Resume a paused container.
    pub async fn unpause_container(&self, id: &str) -> crate::docker::Result<()> {
        self.inner().unpause_container(id).await?;
        Ok(())
    }

    /// List the processes running inside a container (`docker top`).
    pub async fn top_container(&self, id: &str) -> crate::docker::Result<ContainerTopDto> {
        let resp = self
            .inner()
            .top_processes(id, Some(TopOptions { ps_args: "-ef" }))
            .await?;
        Ok(ContainerTopDto {
            titles: resp.titles.unwrap_or_default(),
            processes: resp.processes.unwrap_or_default(),
        })
    }

    /// Take a single resource-usage snapshot for a container.
    ///
    /// Opens the stats stream in one-shot, non-streaming mode and consumes only
    /// the first frame. CPU percentage is computed the canonical Docker way from
    /// the cpu/precpu deltas; memory excludes page cache; net/blk are summed.
    pub async fn stats_once(&self, id: &str) -> crate::docker::Result<ContainerStatsDto> {
        let opts = StatsOptions {
            stream: false,
            one_shot: true,
        };

        // The returned stream isn't `Unpin`; box-pin so we can `.next()` it.
        let mut stream = Box::pin(self.inner().stats(id, Some(opts)));
        let stats = match stream.next().await {
            Some(frame) => frame?,
            None => {
                return Err(crate::docker::DockerError::Api(
                    "no stats frame returned by the engine".to_string(),
                ))
            }
        };

        // --- CPU % (standard docker calc, guarding divide-by-zero) ---
        let cpu_delta = stats
            .cpu_stats
            .cpu_usage
            .total_usage
            .saturating_sub(stats.precpu_stats.cpu_usage.total_usage)
            as f64;
        let system_delta = stats
            .cpu_stats
            .system_cpu_usage
            .unwrap_or(0)
            .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or(0))
            as f64;
        let online_cpus = stats
            .cpu_stats
            .online_cpus
            .filter(|n| *n > 0)
            .or_else(|| {
                stats
                    .cpu_stats
                    .cpu_usage
                    .percpu_usage
                    .as_ref()
                    .map(|v| v.len() as u64)
            })
            .filter(|n| *n > 0)
            .unwrap_or(1) as f64;
        let cpu_pct = if cpu_delta > 0.0 && system_delta > 0.0 {
            (cpu_delta / system_delta) * online_cpus * 100.0
        } else {
            0.0
        };

        // --- Memory (exclude cache, like `docker stats`) ---
        let usage = stats.memory_stats.usage.unwrap_or(0);
        let cache = mem_cache(&stats.memory_stats);
        let mem_usage = usage.saturating_sub(cache);
        let mem_limit = stats.memory_stats.limit.unwrap_or(0);
        let mem_pct = if mem_limit > 0 {
            (mem_usage as f64 / mem_limit as f64) * 100.0
        } else {
            0.0
        };

        // --- Network (sum every interface) ---
        let (mut net_rx, mut net_tx) = (0u64, 0u64);
        if let Some(networks) = stats.networks.as_ref() {
            for net in networks.values() {
                net_rx = net_rx.saturating_add(net.rx_bytes);
                net_tx = net_tx.saturating_add(net.tx_bytes);
            }
        }

        // --- Block I/O (sum io_service_bytes_recursive by op) ---
        let (mut blk_read, mut blk_write) = (0u64, 0u64);
        if let Some(entries) = stats.blkio_stats.io_service_bytes_recursive.as_ref() {
            for e in entries {
                match e.op.to_ascii_lowercase().as_str() {
                    "read" => blk_read = blk_read.saturating_add(e.value),
                    "write" => blk_write = blk_write.saturating_add(e.value),
                    _ => {}
                }
            }
        }

        let pids = stats.pids_stats.current.unwrap_or(0);

        Ok(ContainerStatsDto {
            cpu_pct,
            mem_usage: clamp_i64(mem_usage),
            mem_limit: clamp_i64(mem_limit),
            mem_pct,
            net_rx: clamp_i64(net_rx),
            net_tx: clamp_i64(net_tx),
            blk_read: clamp_i64(blk_read),
            blk_write: clamp_i64(blk_write),
            pids: clamp_i64(pids),
        })
    }
}

/// Page-cache bytes to subtract from raw memory usage, matching the Docker CLI:
/// cgroup v1 uses `total_inactive_file`, cgroup v2 uses `inactive_file`.
fn mem_cache(mem: &bollard::container::MemoryStats) -> u64 {
    use bollard::container::MemoryStatsStats;
    match mem.stats {
        Some(MemoryStatsStats::V1(v1)) => v1.total_inactive_file,
        Some(MemoryStatsStats::V2(v2)) => v2.inactive_file,
        None => 0,
    }
}

/// Saturating `u64 -> i64` so the JSON DTO never overflows on absurd values.
fn clamp_i64(v: u64) -> i64 {
    if v > i64::MAX as u64 {
        i64::MAX
    } else {
        v as i64
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// `docker inspect <id>` as pretty-printed JSON text.
#[tauri::command]
pub async fn container_inspect(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let client = state.get_client().await?;
    client
        .inspect_container_pretty(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Rename a container.
#[tauri::command]
pub async fn container_rename(
    state: tauri::State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .rename_container(&id, &name)
        .await
        .map_err(|e| e.to_string())
}

/// Pause a running container.
#[tauri::command]
pub async fn container_pause(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client.pause_container(&id).await.map_err(|e| e.to_string())
}

/// Unpause a paused container.
#[tauri::command]
pub async fn container_unpause(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .unpause_container(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Process table inside a container (`docker top`).
#[tauri::command]
pub async fn container_top(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ContainerTopDto, String> {
    let client = state.get_client().await?;
    client.top_container(&id).await.map_err(|e| e.to_string())
}

/// One resource-usage snapshot for a container (`docker stats --no-stream`).
#[tauri::command]
pub async fn container_stats(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ContainerStatsDto, String> {
    let client = state.get_client().await?;
    client.stats_once(&id).await.map_err(|e| e.to_string())
}

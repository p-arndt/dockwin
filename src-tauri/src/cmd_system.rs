//! SYSTEM domain commands: disk usage (`docker system df`), engine info
//! (`docker info`), and prune (`docker system prune`).
//!
//! Each `#[tauri::command]` is a thin async wrapper over a helper added to
//! [`crate::docker::DockerClient`] (in this file, via a separate `impl` block
//! that reaches the raw bollard handle through `self.inner()`). All DTOs are
//! plain serde-`Serialize` structs so the frontend never depends on bollard.

use std::collections::HashMap;

use bollard::image::PruneImagesOptions;
use serde::Serialize;

use crate::commands::AppState;
use crate::docker::DockerClient;

// ---------------------------------------------------------------------------
// DTOs (frontend-facing)
// ---------------------------------------------------------------------------

/// Usage for a single resource category in the disk-usage table.
#[derive(Debug, Clone, Serialize)]
pub struct UsageDto {
    /// Number of objects in this category.
    pub count: i64,
    /// Total size in bytes.
    pub size: i64,
    /// Best-effort reclaimable bytes (objects not currently in use).
    pub reclaimable: i64,
}

/// `docker system df` summary.
#[derive(Debug, Clone, Serialize)]
pub struct SystemDfDto {
    pub images: UsageDto,
    pub containers: UsageDto,
    pub volumes: UsageDto,
    pub build_cache: UsageDto,
}

/// `docker info` subset surfaced to the GUI.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfoDto {
    pub name: Option<String>,
    pub server_version: Option<String>,
    pub os: Option<String>,
    pub os_type: Option<String>,
    pub kernel_version: Option<String>,
    pub architecture: Option<String>,
    pub ncpu: Option<i64>,
    pub mem_total: Option<i64>,
    pub storage_driver: Option<String>,
    pub containers: Option<i64>,
    pub containers_running: Option<i64>,
    pub images: Option<i64>,
}

/// Aggregated result of a prune sweep.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PruneResultDto {
    pub containers_deleted: i64,
    pub images_deleted: i64,
    pub networks_deleted: i64,
    pub volumes_deleted: i64,
    pub space_reclaimed: i64,
}

// ---------------------------------------------------------------------------
// DockerClient helpers (separate impl block, per the backend contract)
// ---------------------------------------------------------------------------

impl DockerClient {
    /// `docker system df` — disk usage broken down by category with a
    /// best-effort "reclaimable" figure (objects with no active references).
    pub async fn disk_usage(&self) -> crate::docker::Result<SystemDfDto> {
        let df = self.inner().df().await?;

        // Images: reclaimable = size of images used by zero containers.
        // ImageSummary.containers is `-1` when unknown; treat that as in use.
        let images = {
            let list = df.images.unwrap_or_default();
            let count = list.len() as i64;
            let size: i64 = list.iter().map(|i| i.size).sum();
            let reclaimable: i64 = list
                .iter()
                .filter(|i| i.containers == 0)
                .map(|i| i.size)
                .sum();
            UsageDto {
                count,
                size,
                reclaimable,
            }
        };

        // Containers: size is the writable layer; reclaimable = stopped ones.
        let containers = {
            let list = df.containers.unwrap_or_default();
            let count = list.len() as i64;
            let size: i64 = list.iter().filter_map(|c| c.size_rw).sum();
            let reclaimable: i64 = list
                .iter()
                .filter(|c| c.state.as_deref() != Some("running"))
                .filter_map(|c| c.size_rw)
                .sum();
            UsageDto {
                count,
                size,
                reclaimable,
            }
        };

        // Volumes: size/ref_count come from usage_data (size `-1` == unknown).
        let volumes = {
            let list = df.volumes.unwrap_or_default();
            let count = list.len() as i64;
            let size: i64 = list
                .iter()
                .filter_map(|v| v.usage_data.as_ref())
                .map(|u| u.size.max(0))
                .sum();
            let reclaimable: i64 = list
                .iter()
                .filter_map(|v| v.usage_data.as_ref())
                .filter(|u| u.ref_count == 0)
                .map(|u| u.size.max(0))
                .sum();
            UsageDto {
                count,
                size,
                reclaimable,
            }
        };

        // Build cache: reclaimable = entries not currently in use.
        let build_cache = {
            let list = df.build_cache.unwrap_or_default();
            let count = list.len() as i64;
            let size: i64 = list.iter().filter_map(|b| b.size).sum();
            let reclaimable: i64 = list
                .iter()
                .filter(|b| !b.in_use.unwrap_or(false))
                .filter_map(|b| b.size)
                .sum();
            UsageDto {
                count,
                size,
                reclaimable,
            }
        };

        Ok(SystemDfDto {
            images,
            containers,
            volumes,
            build_cache,
        })
    }

    /// `docker info` mapped into the GUI-facing [`SystemInfoDto`].
    pub async fn system_info(&self) -> crate::docker::Result<SystemInfoDto> {
        let info = self.inner().info().await?;
        Ok(SystemInfoDto {
            name: info.name,
            server_version: info.server_version,
            os: info.operating_system,
            os_type: info.os_type,
            kernel_version: info.kernel_version,
            architecture: info.architecture,
            ncpu: info.ncpu,
            mem_total: info.mem_total,
            storage_driver: info.driver,
            containers: info.containers,
            containers_running: info.containers_running,
            images: info.images,
        })
    }

    /// `docker system prune`. Always removes stopped containers, dangling (or
    /// all unused, when `all_images`) images, and unused networks. Volumes are
    /// only pruned when `volumes` is true. Space reclaimed is summed across the
    /// individual prune calls.
    pub async fn system_prune(
        &self,
        all_images: bool,
        volumes: bool,
    ) -> crate::docker::Result<PruneResultDto> {
        let mut out = PruneResultDto::default();

        // Stopped containers.
        let c = self
            .inner()
            .prune_containers(None::<bollard::container::PruneContainersOptions<String>>)
            .await?;
        out.containers_deleted = c.containers_deleted.map(|v| v.len() as i64).unwrap_or(0);
        out.space_reclaimed += c.space_reclaimed.unwrap_or(0);

        // Images. Default prunes only dangling; the `dangling=false` filter
        // makes the daemon prune ALL unused images.
        let img_opts = if all_images {
            let mut filters: HashMap<String, Vec<String>> = HashMap::new();
            filters.insert("dangling".to_string(), vec!["false".to_string()]);
            Some(PruneImagesOptions { filters })
        } else {
            None
        };
        let i = self.inner().prune_images(img_opts).await?;
        out.images_deleted = i.images_deleted.map(|v| v.len() as i64).unwrap_or(0);
        out.space_reclaimed += i.space_reclaimed.unwrap_or(0);

        // Unused networks (no space_reclaimed field on this response).
        let n = self
            .inner()
            .prune_networks(None::<bollard::network::PruneNetworksOptions<String>>)
            .await?;
        out.networks_deleted = n.networks_deleted.map(|v| v.len() as i64).unwrap_or(0);

        // Volumes only on explicit opt-in (they can hold real data).
        if volumes {
            let v = self
                .inner()
                .prune_volumes(None::<bollard::volume::PruneVolumesOptions<String>>)
                .await?;
            out.volumes_deleted = v.volumes_deleted.map(|v| v.len() as i64).unwrap_or(0);
            out.space_reclaimed += v.space_reclaimed.unwrap_or(0);
        }

        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn system_df(state: tauri::State<'_, AppState>) -> Result<SystemDfDto, String> {
    let client = state.get_client().await?;
    client.disk_usage().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn system_info(state: tauri::State<'_, AppState>) -> Result<SystemInfoDto, String> {
    let client = state.get_client().await?;
    client.system_info().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn system_prune(
    state: tauri::State<'_, AppState>,
    all_images: Option<bool>,
    volumes: Option<bool>,
) -> Result<PruneResultDto, String> {
    let client = state.get_client().await?;
    client
        .system_prune(all_images.unwrap_or(false), volumes.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

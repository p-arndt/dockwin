//! Volumes domain: Tauri commands + bollard engine helpers for managing Docker
//! persistent volumes.
//!
//! This module follows the same shape as `commands.rs`: each `#[tauri::command]`
//! is a thin async wrapper that grabs the cached [`DockerClient`] from
//! [`AppState`] and delegates to an engine helper defined on `DockerClient`
//! below. The helpers borrow the raw bollard handle via `inner()` and return
//! frontend-facing, serde-serializable DTOs so the web layer never sees bollard
//! types directly.

use serde::Serialize;

use bollard::volume::{
    CreateVolumeOptions, ListVolumesOptions, PruneVolumesOptions, RemoveVolumeOptions,
};

use crate::commands::AppState;
use crate::docker::DockerClient;

// ---------------------------------------------------------------------------
// DTOs (serde-serializable, frontend-facing)
// ---------------------------------------------------------------------------

/// A single Docker volume, flattened for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct VolumeDto {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    /// "local" | "global" | "" (empty when the engine omits a scope).
    pub scope: String,
    /// Creation timestamp as reported by the engine (RFC3339-ish), if any.
    pub created: Option<String>,
    /// User-defined key/value metadata, as a stable, sorted list of pairs.
    pub labels: Vec<(String, String)>,
}

/// Result of pruning unused volumes.
#[derive(Debug, Clone, Serialize)]
pub struct VolumePruneResultDto {
    /// Names of the volumes that were removed.
    pub removed: Vec<String>,
    /// Disk space reclaimed, in bytes.
    pub space_reclaimed: i64,
}

// ---------------------------------------------------------------------------
// Engine helpers (extend DockerClient in this module)
// ---------------------------------------------------------------------------

impl DockerClient {
    /// List all volumes known to the engine.
    pub async fn list_volumes(&self) -> crate::docker::Result<Vec<VolumeDto>> {
        // No filters: list everything. The frontend does its own presentation.
        let resp = self
            .inner()
            .list_volumes(None::<ListVolumesOptions<String>>)
            .await?;

        let out = resp
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|v| {
                let mut labels: Vec<(String, String)> = v.labels.into_iter().collect();
                labels.sort_by(|a, b| a.0.cmp(&b.0));
                VolumeDto {
                    name: v.name,
                    driver: v.driver,
                    mountpoint: v.mountpoint,
                    scope: v.scope.map(|s| s.to_string()).unwrap_or_default(),
                    created: v.created_at.map(|d| d.to_string()),
                    labels,
                }
            })
            .collect();

        Ok(out)
    }

    /// Create a volume. Defaults the driver to "local" when none is given.
    pub async fn create_volume(
        &self,
        name: String,
        driver: Option<String>,
    ) -> crate::docker::Result<()> {
        let opts = CreateVolumeOptions::<String> {
            name,
            driver: driver.unwrap_or_else(|| "local".to_string()),
            ..Default::default()
        };
        self.inner().create_volume(opts).await?;
        Ok(())
    }

    /// Remove a volume by name (optionally forcing removal).
    pub async fn remove_volume(&self, name: &str, force: bool) -> crate::docker::Result<()> {
        let opts = RemoveVolumeOptions { force };
        self.inner().remove_volume(name, Some(opts)).await?;
        Ok(())
    }

    /// Prune unused volumes, returning what was removed and the space reclaimed.
    pub async fn prune_volumes(&self) -> crate::docker::Result<VolumePruneResultDto> {
        let resp = self
            .inner()
            .prune_volumes(None::<PruneVolumesOptions<String>>)
            .await?;
        Ok(VolumePruneResultDto {
            removed: resp.volumes_deleted.unwrap_or_default(),
            space_reclaimed: resp.space_reclaimed.unwrap_or(0),
        })
    }

    /// Inspect a volume, returning the raw engine response as pretty JSON.
    pub async fn inspect_volume(&self, name: &str) -> crate::docker::Result<String> {
        let resp = self.inner().inspect_volume(name).await?;
        // The bollard `Volume` model derives Serialize; serde_json is a direct
        // dependency of this crate. Fall back to debug formatting just in case.
        let pretty =
            serde_json::to_string_pretty(&resp).unwrap_or_else(|_| format!("{resp:#?}"));
        Ok(pretty)
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn volume_list(state: tauri::State<'_, AppState>) -> Result<Vec<VolumeDto>, String> {
    let client = state.get_client().await?;
    client.list_volumes().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn volume_create(
    state: tauri::State<'_, AppState>,
    name: String,
    driver: Option<String>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .create_volume(name, driver)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn volume_remove(
    state: tauri::State<'_, AppState>,
    name: String,
    force: Option<bool>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .remove_volume(&name, force.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn volume_prune(
    state: tauri::State<'_, AppState>,
) -> Result<VolumePruneResultDto, String> {
    let client = state.get_client().await?;
    client.prune_volumes().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn volume_inspect(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let client = state.get_client().await?;
    client.inspect_volume(&name).await.map_err(|e| e.to_string())
}

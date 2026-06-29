//! Networks domain: Tauri command layer + bollard engine helpers.
//!
//! Mirrors the structure of `commands.rs` (thin `#[tauri::command]` wrappers
//! returning `Result<T, String>`) and `docker.rs` (engine helpers in an
//! `impl DockerClient` block that borrow the raw bollard handle via
//! `self.inner()`). The frontend (`networksApi.ts`) talks ONLY to the commands
//! below and never sees bollard's types — every response is a serde DTO.

use bollard::network::{
    ConnectNetworkOptions, CreateNetworkOptions, DisconnectNetworkOptions, InspectNetworkOptions,
    ListNetworksOptions, PruneNetworksOptions,
};
use serde::Serialize;

use crate::commands::AppState;
use crate::docker::DockerClient;

// ---------------------------------------------------------------------------
// DTOs (serde-serializable, frontend-facing)
// ---------------------------------------------------------------------------

/// One row in the networks table.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkDto {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub internal: bool,
    /// Number of containers currently attached to this network.
    pub containers: usize,
    /// Creation timestamp (RFC 3339 string as reported by the engine), if any.
    pub created: Option<String>,
    /// True for Docker's predefined networks ("bridge", "host", "none"), which
    /// cannot be removed. The UI disables the Remove action for these.
    pub builtin: bool,
}

/// Result of pruning unused networks.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkPruneResultDto {
    /// Names (or ids) of the networks that were removed.
    pub removed: Vec<String>,
}

/// Names of Docker's predefined networks that can never be removed.
const BUILTIN_NETWORKS: [&str; 3] = ["bridge", "host", "none"];

// ---------------------------------------------------------------------------
// Engine helpers (bollard)
// ---------------------------------------------------------------------------

impl DockerClient {
    /// List all networks.
    pub async fn list_networks(&self) -> crate::docker::Result<Vec<NetworkDto>> {
        let networks = self
            .inner()
            .list_networks(None::<ListNetworksOptions<String>>)
            .await?;

        let out = networks
            .into_iter()
            .map(|n| {
                let name = n.name.unwrap_or_default();
                let builtin = BUILTIN_NETWORKS.contains(&name.as_str());
                NetworkDto {
                    id: n.id.unwrap_or_default(),
                    name,
                    driver: n.driver.unwrap_or_default(),
                    scope: n.scope.unwrap_or_default(),
                    internal: n.internal.unwrap_or(false),
                    containers: n.containers.map(|c| c.len()).unwrap_or(0),
                    // `created` is a String under bollard's default features, and a
                    // chrono/time type under the date features — `to_string()`
                    // yields a readable timestamp either way.
                    created: n.created.map(|c| c.to_string()),
                    builtin,
                }
            })
            .collect();

        Ok(out)
    }

    /// Create a new network. Defaults to the "bridge" driver.
    pub async fn create_network(
        &self,
        name: String,
        driver: Option<String>,
        internal: Option<bool>,
    ) -> crate::docker::Result<String> {
        let opts = CreateNetworkOptions {
            name,
            driver: driver.unwrap_or_else(|| "bridge".to_string()),
            internal: internal.unwrap_or(false),
            ..Default::default()
        };
        let resp = self.inner().create_network(opts).await?;
        Ok(resp.id)
    }

    /// Remove a network by id or name.
    pub async fn remove_network(&self, id: &str) -> crate::docker::Result<()> {
        self.inner().remove_network(id).await?;
        Ok(())
    }

    /// Prune unused networks; returns the removed network names.
    pub async fn prune_networks(&self) -> crate::docker::Result<NetworkPruneResultDto> {
        let resp = self
            .inner()
            .prune_networks(None::<PruneNetworksOptions<String>>)
            .await?;
        Ok(NetworkPruneResultDto {
            removed: resp.networks_deleted.unwrap_or_default(),
        })
    }

    /// Inspect a network, returning pretty-printed JSON for the GUI viewer.
    pub async fn inspect_network(&self, id: &str) -> crate::docker::Result<String> {
        let resp = self
            .inner()
            .inspect_network(id, None::<InspectNetworkOptions<String>>)
            .await?;
        let json = serde_json::to_string_pretty(&resp)
            .unwrap_or_else(|_| format!("{resp:#?}"));
        Ok(json)
    }

    /// Attach a container to a network.
    pub async fn connect_network(
        &self,
        network: &str,
        container: String,
    ) -> crate::docker::Result<()> {
        let opts = ConnectNetworkOptions {
            container,
            endpoint_config: Default::default(),
        };
        self.inner().connect_network(network, opts).await?;
        Ok(())
    }

    /// Detach a container from a network.
    pub async fn disconnect_network(
        &self,
        network: &str,
        container: String,
        force: Option<bool>,
    ) -> crate::docker::Result<()> {
        let opts = DisconnectNetworkOptions {
            container,
            force: force.unwrap_or(false),
        };
        self.inner().disconnect_network(network, opts).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn network_list(state: tauri::State<'_, AppState>) -> Result<Vec<NetworkDto>, String> {
    let client = state.get_client().await?;
    client.list_networks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_create(
    state: tauri::State<'_, AppState>,
    name: String,
    driver: Option<String>,
    internal: Option<bool>,
) -> Result<String, String> {
    let client = state.get_client().await?;
    client
        .create_network(name, driver, internal)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_remove(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.get_client().await?;
    client.remove_network(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_prune(
    state: tauri::State<'_, AppState>,
) -> Result<NetworkPruneResultDto, String> {
    let client = state.get_client().await?;
    client.prune_networks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_inspect(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let client = state.get_client().await?;
    client.inspect_network(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_connect(
    state: tauri::State<'_, AppState>,
    network: String,
    container: String,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .connect_network(&network, container)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_disconnect(
    state: tauri::State<'_, AppState>,
    network: String,
    container: String,
    force: Option<bool>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .disconnect_network(&network, container, force)
        .await
        .map_err(|e| e.to_string())
}

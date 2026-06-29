//! Extended IMAGES domain commands for the dockwin GUI.
//!
//! This module layers richer image operations (pull with live progress, remove,
//! prune, tag, inspect, history) on top of the existing `image_list` command in
//! `commands.rs`. It follows the same conventions as that file: each
//! `#[tauri::command]` is a thin async wrapper over [`crate::docker`] returning
//! `Result<T, String>` so errors surface as readable strings on the JS side.
//!
//! The new bollard calls live in a separate `impl crate::docker::DockerClient`
//! block that reaches the raw handle via `self.inner()`. Progress for the
//! long-running `docker pull` is forwarded to the frontend as the `image://pull`
//! Tauri event (see [`engine_provision`](crate::commands) for the same pattern).

use std::collections::HashMap;

use bollard::image::{
    CreateImageOptions, PruneImagesOptions, RemoveImageOptions, TagImageOptions,
};
use futures_util::stream::StreamExt;
use serde::Serialize;
use tauri::Emitter;

use crate::commands::AppState;
use crate::docker::{DockerClient, Result as DockerResult};

// ---------------------------------------------------------------------------
// DTOs (serde-serializable, frontend-facing)
// ---------------------------------------------------------------------------

/// One streamed update from `docker pull`, forwarded as the `image://pull` event.
/// `done=true` marks the terminal event (success or failure); on failure `error`
/// carries the message.
#[derive(Debug, Clone, Serialize)]
pub struct ImagePullEventDto {
    pub status: Option<String>,
    pub progress: Option<String>,
    pub id: Option<String>,
    pub done: bool,
    pub error: Option<String>,
}

/// Result of an image prune: how many images were deleted and bytes reclaimed.
#[derive(Debug, Clone, Serialize)]
pub struct ImagePruneResultDto {
    pub images_deleted: i64,
    pub space_reclaimed: i64,
}

/// One layer in an image's history (`docker history`).
#[derive(Debug, Clone, Serialize)]
pub struct ImageLayerDto {
    pub id: Option<String>,
    /// Unix seconds.
    pub created: i64,
    pub created_by: String,
    pub size: i64,
    pub comment: String,
}

// ---------------------------------------------------------------------------
// New DockerClient methods (raw bollard via self.inner())
// ---------------------------------------------------------------------------

impl DockerClient {
    /// Remove an image by id or reference. `force` removes even when referenced
    /// by stopped containers / other tags; `no_prune` keeps untagged parents.
    pub async fn remove_image(
        &self,
        name: &str,
        force: bool,
        no_prune: bool,
    ) -> DockerResult<()> {
        let opts = RemoveImageOptions {
            force,
            noprune: no_prune,
        };
        self.inner().remove_image(name, Some(opts), None).await?;
        Ok(())
    }

    /// Prune unused images. When `all` is true, every unused image is removed
    /// (`dangling=false`); otherwise only dangling (untagged) images are pruned.
    pub async fn prune_images(&self, all: bool) -> DockerResult<ImagePruneResultDto> {
        let mut filters: HashMap<String, Vec<String>> = HashMap::new();
        // dangling=true  -> only untagged/unused images
        // dangling=false -> ALL unused images
        filters.insert(
            "dangling".to_string(),
            vec![if all { "false" } else { "true" }.to_string()],
        );
        let resp = self
            .inner()
            .prune_images(Some(PruneImagesOptions { filters }))
            .await?;
        let images_deleted = resp.images_deleted.map(|v| v.len() as i64).unwrap_or(0);
        Ok(ImagePruneResultDto {
            images_deleted,
            space_reclaimed: resp.space_reclaimed.unwrap_or(0),
        })
    }

    /// Tag an existing image (`id`) into `repo:tag`.
    pub async fn tag_image(&self, id: &str, repo: &str, tag: &str) -> DockerResult<()> {
        let opts = TagImageOptions {
            repo: repo.to_string(),
            tag: tag.to_string(),
        };
        self.inner().tag_image(id, Some(opts)).await?;
        Ok(())
    }

    /// Low-level image inspect, returned as pretty JSON for display.
    pub async fn inspect_image_json(&self, id: &str) -> DockerResult<String> {
        let resp = self.inner().inspect_image(id).await?;
        Ok(serde_json::to_string_pretty(&resp).unwrap_or_else(|_| format!("{resp:#?}")))
    }

    /// Parent layers of an image (`docker history`).
    pub async fn image_history(&self, id: &str) -> DockerResult<Vec<ImageLayerDto>> {
        let layers = self.inner().image_history(id).await?;
        Ok(layers
            .into_iter()
            .map(|l| ImageLayerDto {
                id: if l.id.is_empty() || l.id == "<missing>" {
                    None
                } else {
                    Some(l.id)
                },
                created: l.created,
                created_by: l.created_by,
                size: l.size,
                comment: l.comment,
            })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Split a `reference` (e.g. `nginx`, `nginx:1.27`, `registry:5000/team/app:dev`)
/// into `(image, tag)`. Defaults the tag to `latest` when absent. A `:` is only
/// treated as a tag separator when it comes after the final `/` (so a registry
/// host:port like `registry:5000/app` is not mistaken for a tag).
fn split_reference(reference: &str) -> (String, String) {
    let r = reference.trim();
    match r.rsplit_once(':') {
        Some((image, tag)) if !tag.is_empty() && !tag.contains('/') => {
            (image.to_string(), tag.to_string())
        }
        _ => (r.to_string(), "latest".to_string()),
    }
}

/// `docker pull` for `reference`, streaming progress to the frontend via the
/// `image://pull` event. Emits a terminal `{done:true}` (with `error` set on
/// failure) and also returns `Err` on failure. bollard is async, so the stream
/// is driven directly in this command (no spawn_blocking needed).
#[tauri::command]
pub async fn image_pull(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    reference: String,
) -> Result<(), String> {
    let client = state.get_client().await?;
    let (from_image, tag) = split_reference(&reference);

    let options = CreateImageOptions {
        from_image: from_image.clone(),
        tag: tag.clone(),
        ..Default::default()
    };

    let mut stream = client.inner().create_image(Some(options), None, None);

    while let Some(item) = stream.next().await {
        match item {
            Ok(info) => {
                let _ = app.emit(
                    "image://pull",
                    ImagePullEventDto {
                        status: info.status,
                        progress: info.progress,
                        id: info.id,
                        done: false,
                        error: None,
                    },
                );
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = app.emit(
                    "image://pull",
                    ImagePullEventDto {
                        status: None,
                        progress: None,
                        id: None,
                        done: true,
                        error: Some(msg.clone()),
                    },
                );
                return Err(msg);
            }
        }
    }

    let _ = app.emit(
        "image://pull",
        ImagePullEventDto {
            status: Some(format!("Pulled {from_image}:{tag}")),
            progress: None,
            id: None,
            done: true,
            error: None,
        },
    );
    Ok(())
}

/// Remove an image by id or reference.
#[tauri::command]
pub async fn image_remove(
    state: tauri::State<'_, AppState>,
    id: String,
    force: Option<bool>,
    no_prune: Option<bool>,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .remove_image(&id, force.unwrap_or(false), no_prune.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

/// Prune unused images. `all=true` removes all unused images (not just dangling).
#[tauri::command]
pub async fn image_prune(
    state: tauri::State<'_, AppState>,
    all: Option<bool>,
) -> Result<ImagePruneResultDto, String> {
    let client = state.get_client().await?;
    client
        .prune_images(all.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

/// Tag an existing image into `repo:tag`.
#[tauri::command]
pub async fn image_tag(
    state: tauri::State<'_, AppState>,
    id: String,
    repo: String,
    tag: String,
) -> Result<(), String> {
    let client = state.get_client().await?;
    client
        .tag_image(&id, &repo, &tag)
        .await
        .map_err(|e| e.to_string())
}

/// Low-level image inspect, returned as pretty JSON.
#[tauri::command]
pub async fn image_inspect(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let client = state.get_client().await?;
    client
        .inspect_image_json(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Parent layers of an image (`docker history`).
#[tauri::command]
pub async fn image_history(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Vec<ImageLayerDto>, String> {
    let client = state.get_client().await?;
    client.image_history(&id).await.map_err(|e| e.to_string())
}

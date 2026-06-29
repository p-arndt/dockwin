// Frontend API for the VOLUMES domain. Thin wrappers around Tauri's invoke;
// command names must match the Rust #[tauri::command] handlers in
// src-tauri/src/cmd_volumes.rs. Field names mirror the snake_case Rust DTOs.
import { invoke } from "@tauri-apps/api/core";

// A single Docker volume (mirrors VolumeDto on the backend).
export interface Volume {
  name: string;
  driver: string;
  mountpoint: string;
  // "local" | "global" | "" (empty when the engine omits a scope).
  scope: string;
  // Creation timestamp string, or null when unknown.
  created: string | null;
  // User-defined labels as [key, value] pairs.
  labels: [string, string][];
}

// Result of pruning unused volumes (mirrors VolumePruneResultDto).
export interface VolumePruneResult {
  // Names of the volumes that were removed.
  removed: string[];
  // Disk space reclaimed, in bytes.
  space_reclaimed: number;
}

// List all volumes known to the engine.
export function volumeList(): Promise<Volume[]> {
  return invoke<Volume[]>("volume_list");
}

// Create a volume. Driver defaults to "local" on the backend when omitted.
export function volumeCreate(name: string, driver?: string): Promise<void> {
  return invoke("volume_create", { name, driver: driver ?? null });
}

// Remove a volume by name (optionally forcing removal).
export function volumeRemove(name: string, force = false): Promise<void> {
  return invoke("volume_remove", { name, force });
}

// Prune unused volumes.
export function volumePrune(): Promise<VolumePruneResult> {
  return invoke<VolumePruneResult>("volume_prune");
}

// Inspect a volume; returns pretty-printed JSON from the engine.
export function volumeInspect(name: string): Promise<string> {
  return invoke<string>("volume_inspect", { name });
}

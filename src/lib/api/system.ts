// SYSTEM domain API: disk usage, engine info, and prune. Thin wrappers around
// Tauri's invoke. Command names must match the Rust #[tauri::command] handlers
// in src-tauri/src/cmd_system.rs.
import { invoke } from "@tauri-apps/api/core";

// --- Types (mirror the Rust DTOs, snake_case) ---

export interface UsageDto {
  count: number;
  size: number;
  reclaimable: number;
}

export interface SystemDfDto {
  images: UsageDto;
  containers: UsageDto;
  volumes: UsageDto;
  build_cache: UsageDto;
}

export interface SystemInfoDto {
  name: string | null;
  server_version: string | null;
  os: string | null;
  os_type: string | null;
  kernel_version: string | null;
  architecture: string | null;
  ncpu: number | null;
  mem_total: number | null;
  storage_driver: string | null;
  containers: number | null;
  containers_running: number | null;
  images: number | null;
}

export interface PruneResultDto {
  containers_deleted: number;
  images_deleted: number;
  networks_deleted: number;
  volumes_deleted: number;
  space_reclaimed: number;
}

// --- Command wrappers ---

export function systemDf(): Promise<SystemDfDto> {
  return invoke<SystemDfDto>("system_df");
}

export function systemInfo(): Promise<SystemInfoDto> {
  return invoke<SystemInfoDto>("system_info");
}

export function systemPrune(
  allImages = false,
  volumes = false,
): Promise<PruneResultDto> {
  return invoke<PruneResultDto>("system_prune", { allImages, volumes });
}

// Force-remove EVERYTHING regardless of whether it's in use: stops & removes all
// containers, then all images, all volumes, and all user-defined networks.
// Irreversible — a full engine wipe, not a prune of unused resources.
export function systemWipe(): Promise<PruneResultDto> {
  return invoke<PruneResultDto>("system_wipe");
}

// --- Helpers ---

// Format a byte count as a short human-readable string (B/KB/MB/GB/TB).
export function humanBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = n;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  const out = i === 0 ? value : value.toFixed(value < 10 ? 1 : 0);
  return `${out} ${units[i]}`;
}

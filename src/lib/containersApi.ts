// Extended container commands (inspect / stats / top / rename / pause).
//
// Thin typed wrappers over the Tauri `#[tauri::command]`s defined in
// `cmd_containers_ext.rs`. The lifecycle basics (start/stop/restart/remove)
// live in `api.ts`; this module covers the single-container "details" surface.

import { invoke } from "@tauri-apps/api/core";

// --- DTOs (mirror the Rust serde structs, snake_case) ---------------------

// Output of `docker top` for one container.
export interface ContainerTopDto {
  titles: string[];
  processes: string[][];
}

// One resource-usage snapshot, already reduced for rendering.
export interface ContainerStatsDto {
  cpu_pct: number;
  mem_usage: number;
  mem_limit: number;
  mem_pct: number;
  net_rx: number;
  net_tx: number;
  blk_read: number;
  blk_write: number;
  pids: number;
}

// --- Command wrappers ------------------------------------------------------

// `docker inspect <id>` as pretty-printed JSON text.
export function containerInspect(id: string): Promise<string> {
  return invoke<string>("container_inspect", { id });
}

// Rename a container.
export function containerRename(id: string, name: string): Promise<void> {
  return invoke<void>("container_rename", { id, name });
}

// Pause a running container.
export function containerPause(id: string): Promise<void> {
  return invoke<void>("container_pause", { id });
}

// Unpause a paused container.
export function containerUnpause(id: string): Promise<void> {
  return invoke<void>("container_unpause", { id });
}

// In-container process table.
export function containerTop(id: string): Promise<ContainerTopDto> {
  return invoke<ContainerTopDto>("container_top", { id });
}

// One resource-usage snapshot.
export function containerStats(id: string): Promise<ContainerStatsDto> {
  return invoke<ContainerStatsDto>("container_stats", { id });
}

// --- Helpers ---------------------------------------------------------------

// Format a byte count as a human-readable string (binary units, e.g. "12.3 MB").
export function humanBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const exp = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / Math.pow(1024, exp);
  // No decimals for plain bytes; one decimal for KB+.
  const digits = exp === 0 ? 0 : value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[exp]}`;
}

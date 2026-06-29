// Images domain API: thin wrappers around Tauri's invoke for the extended
// IMAGES commands, plus the `image://pull` event subscription and a couple of
// pure formatting helpers. Command names must match the Rust #[tauri::command]
// handlers (image_list lives in commands.rs; the rest in cmd_images_ext.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ImageDto } from "./types";

// --- Types (mirror the Rust DTOs) ---

// One streamed update from `docker pull` (the `image://pull` event).
export interface ImagePullEvent {
  status: string | null;
  progress: string | null;
  id: string | null;
  done: boolean;
  error: string | null;
}

// Result of an image prune.
export interface ImagePruneResult {
  images_deleted: number;
  space_reclaimed: number;
}

// One layer in an image's history.
export interface ImageLayer {
  id: string | null;
  created: number;
  created_by: string;
  size: number;
  comment: string;
}

// --- Commands ---

// List images (reuses the existing image_list command in commands.rs).
export function imageList(all = true): Promise<ImageDto[]> {
  return invoke<ImageDto[]>("image_list", { all });
}

// `docker pull` for a reference (e.g. "nginx:latest"). Progress streams via the
// `image://pull` event (subscribe with onImagePull). Resolves on success and
// rejects with a readable string on failure.
export function imagePull(reference: string): Promise<void> {
  return invoke("image_pull", { reference });
}

// Remove an image by id or reference.
export function imageRemove(
  id: string,
  force = false,
  noPrune = false
): Promise<void> {
  return invoke("image_remove", { id, force, noPrune });
}

// Prune unused images. all=true removes all unused (not just dangling) images.
export function imagePrune(all = false): Promise<ImagePruneResult> {
  return invoke<ImagePruneResult>("image_prune", { all });
}

// Tag an existing image into repo:tag.
export function imageTag(id: string, repo: string, tag: string): Promise<void> {
  return invoke("image_tag", { id, repo, tag });
}

// Low-level image inspect, returned as pretty JSON.
export function imageInspect(id: string): Promise<string> {
  return invoke<string>("image_inspect", { id });
}

// Parent layers of an image (`docker history`).
export function imageHistory(id: string): Promise<ImageLayer[]> {
  return invoke<ImageLayer[]>("image_history", { id });
}

// --- Events ---

// Subscribe to pull progress (the `image://pull` event). Returns a promise of
// an unlisten function.
export function onImagePull(
  handler: (p: ImagePullEvent) => void
): Promise<UnlistenFn> {
  return listen<ImagePullEvent>("image://pull", (ev) => handler(ev.payload));
}

// --- Helpers ---

// Extract a readable error message from a thrown invoke error.
export function errText(e: unknown): string {
  if (e == null) return "Unknown error";
  if (typeof e === "string") return e;
  if (typeof e === "object" && "message" in e) {
    const msg = (e as { message?: unknown }).message;
    if (typeof msg === "string") return msg;
  }
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

// Human-readable byte size (1024-based).
export function humanBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let n = bytes;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  const val = i === 0 ? n : n.toFixed(n < 10 ? 1 : 0);
  return `${val} ${units[i]}`;
}

// Relative time from a unix-seconds timestamp (e.g. "3 days ago").
export function relativeTime(unixSecs: number): string {
  if (!Number.isFinite(unixSecs) || unixSecs <= 0) return "—";
  const ms = unixSecs * 1000;
  const diff = Date.now() - ms;
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} min ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} hour${hr === 1 ? "" : "s"} ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} day${day === 1 ? "" : "s"} ago`;
  return new Date(ms).toLocaleDateString();
}

// Full local date/time string from a unix-seconds timestamp.
export function fullDate(unixSecs: number): string {
  if (!Number.isFinite(unixSecs) || unixSecs <= 0) return "";
  return new Date(unixSecs * 1000).toLocaleString();
}

// Short 12-char image id (strips a leading "sha256:").
export function shortId(id: string): string {
  return (id ?? "").replace(/^sha256:/, "").slice(0, 12);
}

// Networks domain API. Thin wrappers around Tauri's invoke — the only place the
// Networks view talks to the backend. Command names must match the Rust
// #[tauri::command] handlers in `cmd_networks.rs`. Tauri converts camelCase
// argument keys to the snake_case the Rust handlers expect.
import { invoke } from "@tauri-apps/api/core";

// A network row as returned by `network_list` (mirrors the Rust NetworkDto).
export interface NetworkDto {
  id: string;
  name: string;
  driver: string;
  scope: string;
  internal: boolean;
  containers: number;
  created: string | null;
  // True for Docker's predefined networks ("bridge", "host", "none"); the UI
  // disables removal for these.
  builtin: boolean;
}

// Result of `network_prune`.
export interface NetworkPruneResultDto {
  removed: string[];
}

// List all networks.
export function networkList(): Promise<NetworkDto[]> {
  return invoke<NetworkDto[]>("network_list");
}

// Create a network; returns the new network id. Driver defaults to "bridge"
// in the backend when omitted.
export function networkCreate(
  name: string,
  driver?: string,
  internal?: boolean
): Promise<string> {
  return invoke<string>("network_create", { name, driver, internal });
}

// Remove a network by id (or name).
export function networkRemove(id: string): Promise<void> {
  return invoke("network_remove", { id });
}

// Prune unused networks; returns the removed network names.
export function networkPrune(): Promise<NetworkPruneResultDto> {
  return invoke<NetworkPruneResultDto>("network_prune");
}

// Inspect a network — returns pretty-printed JSON.
export function networkInspect(id: string): Promise<string> {
  return invoke<string>("network_inspect", { id });
}

// Attach a container to a network.
export function networkConnect(
  network: string,
  container: string
): Promise<void> {
  return invoke("network_connect", { network, container });
}

// Detach a container from a network.
export function networkDisconnect(
  network: string,
  container: string,
  force?: boolean
): Promise<void> {
  return invoke("network_disconnect", { network, container, force });
}

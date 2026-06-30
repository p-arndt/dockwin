// Thin wrapper around Tauri's invoke. The ONLY place the frontend talks to
// dockwin-core. Command names must match the Rust #[tauri::command] handlers.
import { invoke } from "@tauri-apps/api/core";
import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ComposeOutput,
  ContainerDto,
  EngineState,
  EngineStatusDto,
  EngineUpdateDto,
  ImageDto,
  LogChunkDto,
  NormalizedContainer,
  NormalizedPort,
  PortMappingDto,
  ProvisionProgress,
} from "../types";

// --- Engine lifecycle ---

// Map the backend's raw status enum to the frontend EngineState union.
// Also tolerates an already-mapped value or an unexpected string.
export function mapEngineStatus(raw: unknown): EngineState {
  switch (raw) {
    case "running":
      return "running";
    case "stopped":
      return "stopped";
    case "not_provisioned":
    case "not-provisioned":
      return "not-provisioned";
    case "broken":
      return "broken";
    case "incomplete":
      return "incomplete";
    case "unreachable":
    case "unknown":
      return "unknown";
    default:
      return "unknown";
  }
}

// Returns the engine state. The backend returns an EngineStatusDto whose
// `.status` field carries the raw enum; we map it to the EngineState union.
export async function engineStatus(): Promise<EngineState> {
  const dto = await invoke<EngineStatusDto>("engine_status");
  return mapEngineStatus(dto?.status);
}

export function engineStart(): Promise<void> {
  return invoke("engine_start");
}

export function engineStop(): Promise<void> {
  return invoke("engine_stop");
}

// Provision the dedicated WSL2 engine distro end-to-end (download/import rootfs,
// install dockerd, wire the docker context). Long-running — minutes.
export function engineProvision(enableTcp = false): Promise<void> {
  return invoke("engine_provision", { enableTcp });
}

// Tear down the engine distro (wsl --unregister) + remove docker context(s).
// `backup` exports the distro to a .tar first. Caller confirms beforehand.
export function engineTeardown(backup = false): Promise<void> {
  return invoke("engine_teardown", { backup });
}

// Reset a broken / dangling engine registration so it can be reprovisioned.
export function engineRepair(): Promise<void> {
  return invoke("engine_repair");
}

// Check whether a newer Docker Engine is available in the pinned apt repo.
// Cheap; returns empty fields when the engine isn't running (safe on launch).
export function engineUpdateCheck(): Promise<EngineUpdateDto> {
  return invoke<EngineUpdateDto>("engine_update_check");
}

// Upgrade the in-distro Docker Engine packages in place and restart dockerd.
// Long-running; live progress arrives via the `engine://update` event.
export function engineUpdate(): Promise<void> {
  return invoke("engine_update");
}

// Subscribe to provisioning progress (the `engine://provision` event).
export function onProvisionProgress(
  handler: (p: ProvisionProgress) => void
): Promise<UnlistenFn> {
  return listen<ProvisionProgress>("engine://provision", (ev) => handler(ev.payload));
}

// Subscribe to engine-update progress (the `engine://update` event). Same
// ProvisionProgress shape as provisioning, reused for the update bar + log.
export function onEngineUpdateProgress(
  handler: (p: ProvisionProgress) => void
): Promise<UnlistenFn> {
  return listen<ProvisionProgress>("engine://update", (ev) => handler(ev.payload));
}

// --- Compose ---

// `docker compose up` for a Windows compose-file path, run inside the dockwin
// engine. Detached by default. Streams output via onComposeOutput.
export function composeUp(path: string, foreground = false): Promise<void> {
  return invoke("compose_up", { path, foreground });
}

// `docker compose down` for a compose-file path, run inside the engine.
export function composeDown(path: string): Promise<void> {
  return invoke("compose_down", { path });
}

// `docker compose build` — rebuild service images.
export function composeBuild(path: string): Promise<void> {
  return invoke("compose_build", { path });
}

// `docker compose pull` — pull service images.
export function composePull(path: string): Promise<void> {
  return invoke("compose_pull", { path });
}

// `docker compose restart`.
export function composeRestart(path: string): Promise<void> {
  return invoke("compose_restart", { path });
}

// `docker compose logs` — bounded tail snapshot (streamed via onComposeOutput).
export function composeLogs(path: string, tail = 200): Promise<void> {
  return invoke("compose_logs", { path, tail });
}

// Subscribe to compose output lines (the `compose://output` event).
export function onComposeOutput(
  handler: (line: string) => void
): Promise<UnlistenFn> {
  return listen<ComposeOutput>("compose://output", (ev) => handler(ev.payload?.line ?? ""));
}

// --- Containers ---

// Returns the raw container list from dockwin-core (array of records).
export function containerList(all = true): Promise<ContainerDto[]> {
  return invoke<ContainerDto[]>("container_list", { all });
}

export function containerStart(id: string): Promise<void> {
  return invoke("container_start", { id });
}

// timeoutSecs (camelCase) is converted to timeout_secs by Tauri.
export function containerStop(id: string, timeoutSecs?: number): Promise<void> {
  return invoke("container_stop", { id, timeoutSecs });
}

export function containerRestart(
  id: string,
  timeoutSecs?: number
): Promise<void> {
  return invoke("container_restart", { id, timeoutSecs });
}

export function containerRemove(id: string, force = false): Promise<void> {
  return invoke("container_remove", { id, force });
}

// --- Images ---

export function imageList(all = true): Promise<ImageDto[]> {
  return invoke<ImageDto[]>("image_list", { all });
}

// --- Logs ---

export function containerLogs(id: string, tail?: number): Promise<LogChunkDto[]> {
  return invoke<LogChunkDto[]>("container_logs", { id, tail });
}

// --- Events ---
// Subscribe to a backend event. Returns a promise of an unlisten function.
// Safe no-op pattern: callers may ignore failures if the backend never emits.
export function on<T = unknown>(
  event: string,
  handler: (event: Event<T>) => void
): Promise<UnlistenFn> {
  return listen<T>(event, handler);
}

// --- Helpers (pure view-layer normalization) ---

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

// Normalize a container record (bollard field names vary, so be defensive).
export function normalizeContainer(c: ContainerDto): NormalizedContainer {
  const id = c.id ?? c.Id ?? c.ID ?? "";
  let name = c.name ?? c.Name ?? "";
  if (!name) {
    const names = c.names ?? c.Names;
    if (Array.isArray(names)) name = names[0] ?? "";
  }
  name = String(name).replace(/^\//, "");
  const image = c.image ?? c.Image ?? "";
  const state = (c.state ?? c.State ?? "").toString().toLowerCase();
  const status = c.status ?? c.Status ?? "";
  const ports = normalizePorts(c.ports ?? c.Ports ?? []);
  const shortId = id.slice(0, 12);
  const composeRaw = c.compose_project;
  const composeProject =
    typeof composeRaw === "string" && composeRaw.trim() !== ""
      ? composeRaw
      : null;
  const wdRaw = c.compose_working_dir;
  const composeWorkingDir =
    typeof wdRaw === "string" && wdRaw.trim() !== "" ? wdRaw : null;
  return {
    id,
    shortId,
    name: name || shortId,
    image,
    state,
    status,
    ports,
    running: state === "running",
    composeProject,
    composeWorkingDir,
  };
}

// Group normalized containers into Docker Compose stacks (projects). Containers
// without a compose project are omitted. Sorted by project name; services within
// a stack running-first then by name.
export function groupStacks(
  containers: import("../types").NormalizedContainer[]
): import("../types").Stack[] {
  const byProject = new Map<string, import("../types").NormalizedContainer[]>();
  for (const c of containers) {
    if (!c.composeProject) continue;
    const list = byProject.get(c.composeProject) ?? [];
    list.push(c);
    byProject.set(c.composeProject, list);
  }
  const stacks = [...byProject.entries()].map(([project, list]) => {
    list.sort((a, b) => {
      if (a.running !== b.running) return a.running ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    return {
      project,
      containers: list,
      running: list.filter((c) => c.running).length,
      total: list.length,
      workingDir: list.find((c) => c.composeWorkingDir)?.composeWorkingDir ?? null,
    };
  });
  stacks.sort((a, b) => a.project.localeCompare(b.project));
  return stacks;
}

// Returns [{ host, container, proto, ip, wildcard, url|null }].
// Wildcard / 0.0.0.0 publishes are forwarded to Windows localhost;
// 127.0.0.1-bound publishes are NOT (surfaced as a caveat in the UI).
export function normalizePorts(
  ports: PortMappingDto[] | undefined | null
): NormalizedPort[] {
  if (!Array.isArray(ports)) return [];
  // Docker publishes the same host port twice when bound to both IPv4 (0.0.0.0)
  // and IPv6 (::); they render identically, so dedup by host-port + protocol and
  // keep the forwarded (clickable) variant if either is.
  const seen = new Map<string, NormalizedPort>();
  for (const p of ports) {
    const publicPort = p.public_port ?? p.PublicPort ?? p.host ?? null;
    const privatePort = p.private_port ?? p.PrivatePort ?? p.container ?? null;
    const ip = (p.ip ?? p.IP ?? "").toString();
    const proto = (p.protocol ?? p.type ?? p.Type ?? p.proto ?? "tcp").toString();
    if (publicPort == null) continue; // unpublished port — skip
    const wildcard = ip === "" || ip === "0.0.0.0" || ip === "::";
    // Prefer the backend's own decision; fall back to the wildcard heuristic.
    const forwarded =
      typeof p.forwarded_to_localhost === "boolean"
        ? p.forwarded_to_localhost
        : wildcard;
    const url =
      forwarded && proto.toLowerCase() === "tcp"
        ? `http://localhost:${publicPort}`
        : null;
    const entry: NormalizedPort = {
      host: publicPort,
      container: privatePort,
      proto,
      ip,
      wildcard,
      url,
    };
    const key = `${publicPort}/${proto.toLowerCase()}`;
    const prev = seen.get(key);
    // First occurrence wins, unless a later one is forwarded and the kept one
    // isn't (prefer the clickable variant).
    if (!prev || (!prev.url && entry.url)) seen.set(key, entry);
  }
  return [...seen.values()];
}

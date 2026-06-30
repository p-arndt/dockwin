// Shared frontend types. The Rust DTOs use snake_case fields; we mirror them
// here and also tolerate bollard/Docker-style PascalCase via the normalizers.

// Engine lifecycle state used throughout the frontend (kebab-case union).
export type EngineState =
  | "running"
  | "stopped"
  | "not-provisioned"
  | "broken"
  | "unknown";

// Raw status string as emitted by the backend EngineStatusDto.
export type RawEngineStatus =
  | "running"
  | "stopped"
  | "not_provisioned"
  | "unreachable";

// Engine version info (all fields nullable).
export interface VersionDto {
  version: string | null;
  api_version: string | null;
  os: string | null;
  arch: string | null;
  kernel_version: string | null;
}

// Result of the engine_status command.
export interface EngineStatusDto {
  status: RawEngineStatus;
  version: VersionDto | null;
  detail: string | null;
}

// A line of container log output.
export interface LogChunkDto {
  stream: string;
  message: string;
}

// A local image record.
export interface ImageDto {
  id: string;
  tags: string[];
  size: number;
  created: number;
}

// Raw port mapping as it may arrive from the backend (snake_case from Rust DTO,
// with PascalCase / alt names tolerated defensively).
export interface PortMappingDto {
  private_port?: number | null;
  public_port?: number | null;
  ip?: string | null;
  protocol?: string | null;
  forwarded_to_localhost?: boolean | null;
  // Tolerated alternates.
  PrivatePort?: number | null;
  PublicPort?: number | null;
  IP?: string | null;
  type?: string | null;
  Type?: string | null;
  proto?: string | null;
  host?: number | null;
  container?: number | null;
}

// Raw container record from the backend (snake_case Rust DTO + tolerated alts).
export interface ContainerDto {
  id?: string;
  name?: string;
  image?: string;
  state?: string;
  status?: string;
  compose_project?: string | null;
  ports?: PortMappingDto[];
  // Tolerated alternates (bollard / Docker API casing).
  Id?: string;
  ID?: string;
  Name?: string;
  Names?: string[];
  names?: string[];
  Image?: string;
  State?: string;
  Status?: string;
  Ports?: PortMappingDto[];
}

// Normalized port ready for rendering.
export interface NormalizedPort {
  host: number;
  container: number | null;
  proto: string;
  ip: string;
  wildcard: boolean;
  url: string | null;
}

// Normalized container ready for rendering.
export interface NormalizedContainer {
  id: string;
  shortId: string;
  name: string;
  image: string;
  state: string;
  status: string;
  ports: NormalizedPort[];
  running: boolean;
  // Docker Compose project this container belongs to (null when standalone).
  composeProject: string | null;
}

// A group of containers belonging to one Docker Compose project.
export interface Stack {
  project: string;
  containers: NormalizedContainer[];
  running: number;
  total: number;
}

// A provisioning progress update pushed from the backend during engine setup
// (the `engine://provision` event). Drives the setup progress bar + live log.
export interface ProvisionProgress {
  phase: string;
  message: string;
  pct: number;
  // "step" | "info" | "warn" | "error".
  level: string;
  done: boolean;
  error: string | null;
}

// One line of `docker compose` output (the `compose://output` event).
export interface ComposeOutput {
  line: string;
}

// Aggregated, UI-facing provisioning state (built from ProvisionProgress events).
// Drives the setup progress bar + live log in the engine gate.
export interface ProvisionUi {
  pct: number;
  phase: string;
  message: string;
  log: string[];
}

// Compose controller: owns the docker-compose state (busy/log/panel/last file)
// and the up/down/pull/build/logs handlers, which are all the same flow with a
// different verb. Created once in App.svelte and passed to StacksView, so the
// state survives navigating away from and back to the Stacks screen.
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api";

export interface ComposeDeps {
  setFooter: (msg: string, isError?: boolean) => void;
  refreshAll: () => Promise<void>;
}

export function createCompose({ setFooter, refreshAll }: ComposeDeps) {
  let busy = $state(false);
  let log = $state<string[]>([]);
  let panelOpen = $state(false);
  let lastFile = $state<string | null>(null);

  async function pickFile(): Promise<string | null> {
    const sel = await open({
      multiple: false,
      directory: false,
      title: "Select a Docker Compose file",
      filters: [{ name: "Compose", extensions: ["yml", "yaml"] }],
    });
    return typeof sel === "string" ? sel : null;
  }

  // Shared flow for every compose verb. `pick` forces a fresh file picker (up);
  // otherwise the last file is reused. `resetLog` clears the panel first.
  async function run(
    label: string,
    exec: (file: string) => Promise<void>,
    { pick = false, resetLog = false } = {}
  ) {
    if (busy) return;
    const file = pick ? await pickFile() : (lastFile ?? (await pickFile()));
    if (!file) return;
    busy = true;
    panelOpen = true;
    lastFile = file;
    if (resetLog) log = [];
    setFooter(`compose ${label}: ${file}…`);
    try {
      await exec(file);
      setFooter(`Compose ${label} complete.`);
    } catch (e) {
      setFooter(`Compose ${label} failed: ${api.errText(e)}`, true);
    } finally {
      busy = false;
      await refreshAll();
    }
  }

  return {
    get busy() {
      return busy;
    },
    get log() {
      return log;
    },
    get panelOpen() {
      return panelOpen;
    },
    set panelOpen(v: boolean) {
      panelOpen = v;
    },
    get lastFile() {
      return lastFile;
    },
    // Appended from the compose-output event stream (wired in App.svelte).
    appendLog(line: string) {
      log = [...log, line].slice(-500);
    },
    up: () => run("up", (f) => api.composeUp(f, false), { pick: true, resetLog: true }),
    down: () => run("down", (f) => api.composeDown(f)),
    pull: () => run("pull", api.composePull, { resetLog: true }),
    build: () => run("build", api.composeBuild, { resetLog: true }),
    logs: () => run("logs", (f) => api.composeLogs(f), { resetLog: true }),
  };
}

export type ComposeController = ReturnType<typeof createCompose>;

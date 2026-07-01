// Root application controller. Owns the engine lifecycle, container data/actions,
// polling, compose state and the container-details drawer — everything that used
// to live in the old App.svelte. Created once in the root +layout.svelte and shared
// with every route via Svelte context (see get/setAppController below), so screens
// read live state instead of receiving it as props through a single page component.
//
// Talks to dockwin-core only through $lib/api (no raw invoke).
import { getContext, setContext } from "svelte";
import * as api from "$lib/api";
import { confirmDialog } from "$lib/state/confirm.svelte";
import { createCompose, type ComposeController } from "$lib/state/compose.svelte";
import type { View } from "$lib/components/AppSidebar.svelte";
import { NAV } from "$lib/components/AppSidebar.svelte";
import type {
  EngineState,
  NormalizedContainer,
  ProvisionProgress,
  ProvisionUi,
  Stack,
} from "$lib/types";

const POLL_MS = 3000;

type EngineAction = "start" | "stop" | "restart" | "remove";
type StackAction = "start" | "stop" | "restart";

export class AppController {
  // --- reactive state (Svelte 5 runes) ---
  engineState = $state<EngineState>("unknown");
  containers = $state<NormalizedContainer[]>([]);
  containersLoaded = $state(false); // true once the first containers fetch resolves
  pending = $state<Set<string>>(new Set()); // container ids with an in-flight action
  errorMsg = $state("");
  footer = $state("Ready.");
  footerErr = $state(false);
  engineBusy = $state(false);
  repairing = $state(false); // engine_repair (reset broken distro) in flight

  // Bumped on manual refresh so the open resource view reloads.
  imageRefreshKey = $state(0);

  // Setup / teardown (provisioning is long-running — minutes).
  working = $state(false); // provision or teardown in flight
  enableTcp = $state(false); // opt into the insecure loopback-TCP fallback
  proxy = $state(""); // optional HTTP(S) proxy for in-distro apt/docker (empty = auto)
  withBackup = $state(false); // export a .tar before teardown
  provision = $state<ProvisionUi | null>(null); // live provisioning progress

  // Container details: selected container + whether it's the full-width page
  // (true) or the right-side drawer (false).
  selectedContainer = $state<NormalizedContainer | null>(null);
  detailFull = $state(false);

  // Compose state + actions live in their own controller (created here so the
  // state survives navigating away from and back to the Stacks screen).
  readonly compose: ComposeController;

  private busy = false; // non-reactive guard against overlapping refreshes
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    this.compose = createCompose({
      setFooter: (msg, isError) => this.setFooter(msg, isError),
      refreshAll: () => this.refreshAll(),
    });
  }

  // --- derived ---
  // The management shell only mounts once the engine is running; every other
  // state is owned by the full-window EngineGate, so the engine lifecycle never
  // bleeds into the management UI.
  engineReady = $derived(this.engineState === "running");
  engineToggleDisabled = $derived(
    this.engineBusy || this.working || this.repairing || this.engineState === "unknown"
  );
  // Tone for the quiet status dots (top bar + engine pod).
  engineTone = $derived(
    this.engineState === "running" ? "live" : this.engineState === "stopped" ? "off" : "warn"
  );
  // Compact engine status line shown in the sidebar pod + engine-gate bar.
  engineLine = $derived(
    this.engineState === "running"
      ? "Engine running"
      : this.engineState === "stopped"
        ? "Engine stopped"
        : this.engineState === "not-provisioned"
          ? "Engine not provisioned"
          : this.engineState === "broken"
            ? "Engine broken"
            : this.engineState === "incomplete"
              ? "Engine setup incomplete"
              : "Engine unknown"
  );
  runningCount = $derived(this.containers.filter((c) => c.running).length);
  // Containers grouped into Docker Compose stacks (by project label).
  stacks = $derived(api.groupStacks(this.containers));
  // Per-view counts shown as nav badges (AppSidebar owns the nav model itself).
  navCounts = $derived<Partial<Record<View, number>>>({
    containers: this.containers.length,
    stacks: this.stacks.length,
  });

  // Human label for a nav view (used by the top-bar breadcrumb).
  viewLabel(view: View): string {
    return view === "settings" ? "Settings" : (NAV.find((n) => n.id === view)?.label ?? "");
  }

  setFooter = (msg: string, isError = false) => {
    this.footer = msg;
    this.footerErr = isError;
  };

  // --- container details ---
  selectContainer(c: NormalizedContainer) {
    this.selectedContainer = c;
    this.detailFull = false; // a fresh selection always opens as the drawer
  }
  closeDetail() {
    this.selectedContainer = null;
    this.detailFull = false;
  }
  toggleDetailFull() {
    this.detailFull = !this.detailFull;
  }

  // --- data flow ---
  async refreshEngine() {
    try {
      this.engineState = await api.engineStatus();
    } catch (e) {
      this.engineState = "unknown";
      this.setFooter(`Engine status failed: ${api.errText(e)}`, true);
    }
  }

  async refreshContainers() {
    // While provisioning/teardown is in flight the engine is intentionally in a
    // transitional state — don't overwrite the progress UI with a stale error.
    if (this.working) return;
    if (this.engineState !== "running") {
      this.containers = [];
      this.containersLoaded = false;
      if (this.engineState === "stopped") {
        this.errorMsg = "Engine is stopped. Start the engine to see containers.";
      } else if (this.engineState === "not-provisioned") {
        this.errorMsg = "Engine is not provisioned. Run `dockwin provision` first.";
      } else {
        this.errorMsg = "";
      }
      return;
    }
    try {
      const raw = await api.containerList(true);
      const list = Array.isArray(raw) ? raw.map(api.normalizeContainer) : [];
      // Running first, then by name.
      list.sort((a, b) => {
        if (a.running !== b.running) return a.running ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      this.containers = list;
      this.containersLoaded = true;
      this.errorMsg = "";
      this.setFooter(`Updated ${new Date().toLocaleTimeString()}.`);
    } catch (e) {
      this.errorMsg = `Failed to load containers: ${api.errText(e)}`;
    }
  }

  refreshAll = async () => {
    if (this.busy) return;
    this.busy = true;
    try {
      await this.refreshEngine();
      await this.refreshContainers();
    } finally {
      this.busy = false;
    }
  };

  // Explicit user-triggered refresh: also nudge the open resource view to reload.
  manualRefresh = async () => {
    this.imageRefreshKey++;
    await this.refreshAll();
  };

  // pending is a Set held in $state; reassign to trigger reactivity.
  private markPending(id: string, on: boolean) {
    const next = new Set(this.pending);
    if (on) next.add(id);
    else next.delete(id);
    this.pending = next;
  }

  async handleAction(action: EngineAction, c: NormalizedContainer) {
    if (this.pending.has(c.id)) return;
    if (action === "remove") {
      const ok = await confirmDialog({
        title: `Remove container "${c.name}"?`,
        description: c.running
          ? "It is running — it will be stopped and then deleted."
          : "This permanently removes the container.",
        destructive: true,
        confirmText: "Remove",
      });
      if (!ok) return;
    }

    this.markPending(c.id, true);
    this.setFooter(`${action} ${c.name}…`);
    try {
      switch (action) {
        case "start":
          await api.containerStart(c.id);
          break;
        case "stop":
          await api.containerStop(c.id);
          break;
        case "restart":
          await api.containerRestart(c.id);
          break;
        case "remove":
          await api.containerRemove(c.id, c.running);
          break;
      }
      this.setFooter(`${action} ${c.name} done.`);
    } catch (e) {
      this.setFooter(`${action} ${c.name} failed: ${api.errText(e)}`, true);
      this.errorMsg = `${action} failed: ${api.errText(e)}`;
    } finally {
      this.markPending(c.id, false);
      await this.refreshAll();
    }
  }

  // Apply a start/stop/remove to a set of selected containers at once (the
  // Containers screen's bulk-action bar). Mirrors handleStackAction's
  // filter-by-relevance + parallel-dispatch shape.
  async handleBulkAction(action: EngineAction, targets: NormalizedContainer[]) {
    const relevant = targets.filter((c) => {
      if (action === "start") return !c.running;
      if (action === "stop") return c.running;
      return true; // remove/restart: every selected container
    });
    if (relevant.length === 0) return;

    if (action === "remove") {
      const ok = await confirmDialog({
        title: `Remove ${relevant.length} container${relevant.length === 1 ? "" : "s"}?`,
        description: relevant.some((c) => c.running)
          ? "Running containers in the selection will be stopped and then deleted."
          : "This permanently removes the selected containers.",
        destructive: true,
        confirmText: "Remove",
      });
      if (!ok) return;
    }

    for (const c of relevant) this.markPending(c.id, true);
    this.setFooter(`${action} ${relevant.length} container${relevant.length === 1 ? "" : "s"}…`);
    try {
      await Promise.all(
        relevant.map((c) => {
          if (action === "start") return api.containerStart(c.id);
          if (action === "stop") return api.containerStop(c.id);
          if (action === "restart") return api.containerRestart(c.id);
          return api.containerRemove(c.id, c.running);
        })
      );
      this.setFooter(`${action} ${relevant.length} container${relevant.length === 1 ? "" : "s"} done.`);
    } catch (e) {
      this.setFooter(`${action} failed: ${api.errText(e)}`, true);
      this.errorMsg = `${action} failed: ${api.errText(e)}`;
    } finally {
      for (const c of relevant) this.markPending(c.id, false);
      await this.refreshAll();
    }
  }

  // Apply a start/stop/restart to every (relevant) container in a Compose stack.
  async handleStackAction(action: StackAction, stack: Stack) {
    const targets = stack.containers.filter((c) => {
      if (action === "start") return !c.running;
      if (action === "stop") return c.running;
      return true; // restart: every container in the stack
    });
    if (targets.length === 0) return;

    for (const c of targets) this.markPending(c.id, true);
    this.setFooter(`${action} stack ${stack.project} (${targets.length})…`);
    try {
      await Promise.all(
        targets.map((c) => {
          if (action === "start") return api.containerStart(c.id);
          if (action === "stop") return api.containerStop(c.id);
          return api.containerRestart(c.id);
        })
      );
      this.setFooter(`${action} stack ${stack.project} done.`);
    } catch (e) {
      this.setFooter(`${action} stack ${stack.project} failed: ${api.errText(e)}`, true);
      this.errorMsg = `Stack ${action} failed: ${api.errText(e)}`;
    } finally {
      for (const c of targets) this.markPending(c.id, false);
      await this.refreshAll();
    }
  }

  async toggleEngine() {
    if (this.engineBusy || this.working) return;
    this.engineBusy = true;
    try {
      if (this.engineState === "running") {
        this.setFooter("Stopping engine…");
        await api.engineStop();
      } else if (this.engineState === "stopped") {
        this.setFooter("Starting engine…");
        await api.engineStart();
      }
    } catch (e) {
      this.setFooter(`Engine action failed: ${api.errText(e)}`, true);
    } finally {
      this.engineBusy = false;
      await this.refreshAll();
    }
  }

  // Restart the running engine. There's no single backend command for this, so
  // sequence a stop then a start (sharing the engineBusy guard with toggleEngine).
  async restartEngine() {
    if (this.engineBusy || this.working) return;
    if (this.engineState !== "running") return;
    this.engineBusy = true;
    try {
      this.setFooter("Restarting engine…");
      await api.engineStop();
      await api.engineStart();
      this.setFooter("Engine restarted.");
    } catch (e) {
      this.setFooter(`Engine restart failed: ${api.errText(e)}`, true);
    } finally {
      this.engineBusy = false;
      await this.refreshAll();
    }
  }

  // Apply a live provisioning progress event to the UI.
  private handleProvision = (p: ProvisionProgress) => {
    if (!this.working) return; // ignore stray late events
    const prev = this.provision ?? { pct: 0, phase: p.phase, message: p.message, log: [] };
    this.provision = {
      pct: Math.min(100, Math.max(prev.pct, p.pct)), // monotonic
      phase: p.phase,
      message: p.message,
      log: [...prev.log, p.message].slice(-300),
    };
    if (p.done && p.error) this.errorMsg = p.error;
  };

  // Provision the dedicated WSL2 engine distro (downloads the rootfs, installs
  // dockerd). Long-running; live progress arrives via engine://provision.
  async provisionEngine() {
    if (this.working) return;
    this.working = true;
    this.errorMsg = "";
    this.provision = { pct: 0, phase: "preflight", message: "Starting setup…", log: [] };
    this.setFooter("Setting up engine… this can take several minutes.");
    try {
      await api.engineProvision(this.enableTcp, this.proxy);
      this.setFooter("Engine set up. Starting…");
    } catch (e) {
      this.setFooter(`Setup failed: ${api.errText(e)}`, true);
      this.errorMsg = `Setup failed: ${api.errText(e)}`;
    } finally {
      this.working = false;
      this.provision = null;
      await this.refreshAll();
    }
  }

  // Tear down the engine: permanently unregister the WSL distro + remove contexts.
  async teardownEngine() {
    if (this.working) return;
    const ok = await confirmDialog({
      title: "Remove the dockwin engine?",
      description:
        "This permanently deletes the WSL distro 'dockwin' and all its containers and images." +
        (this.withBackup ? " A .tar backup is exported to your user folder first." : ""),
      destructive: true,
      confirmText: "Remove engine",
    });
    if (!ok) return;
    this.working = true;
    this.setFooter("Removing engine…");
    try {
      await api.engineTeardown(this.withBackup);
      this.setFooter("Engine removed.");
    } catch (e) {
      this.setFooter(`Teardown failed: ${api.errText(e)}`, true);
      this.errorMsg = `Teardown failed: ${api.errText(e)}`;
    } finally {
      this.working = false;
      await this.refreshAll();
    }
  }

  // Reset a broken engine: the 'dockwin' WSL distro is registered but its disk
  // image is missing. Unregister the dangling distro so it can be reprovisioned.
  async repairEngine() {
    if (this.repairing || this.working) return;
    const ok = await confirmDialog({
      title: "Reset the broken engine?",
      description:
        "This unregisters the dangling WSL distro 'dockwin' so you can set it up again from scratch.",
      destructive: true,
      confirmText: "Reset",
    });
    if (!ok) return;
    this.repairing = true;
    this.errorMsg = "";
    this.setFooter("Resetting engine…");
    try {
      await api.engineRepair();
      this.setFooter("Engine reset. You can set it up again.");
    } catch (e) {
      this.setFooter(`Repair failed: ${api.errText(e)}`, true);
      this.errorMsg = `Repair failed: ${api.errText(e)}`;
    } finally {
      this.repairing = false;
      await this.refreshAll();
    }
  }

  // --- polling lifecycle ---
  private startPolling() {
    this.stopPolling();
    this.pollTimer = setInterval(this.refreshAll, POLL_MS);
  }
  private stopPolling() {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  // Wire up backend event listeners + polling. Call from the root layout's
  // onMount; the returned function tears everything down on unmount.
  mount(): () => void {
    const unlisteners: Array<() => void> = [];

    // Optional backend-pushed refresh / engine-state events. Safe if never emitted.
    api
      .on("dockwin://refresh", () => this.refreshAll())
      .then((u) => unlisteners.push(u))
      .catch(() => {});
    api
      .on<string | { status?: string; state?: string }>("dockwin://engine-state", (ev) => {
        const payload = ev?.payload;
        if (payload == null) return;
        const raw = typeof payload === "string" ? payload : payload.status ?? payload.state;
        this.engineState = api.mapEngineStatus(raw);
        this.refreshContainers();
      })
      .then((u) => unlisteners.push(u))
      .catch(() => {});

    // Live provisioning progress + compose output.
    api
      .onProvisionProgress(this.handleProvision)
      .then((u) => unlisteners.push(u))
      .catch(() => {});
    api
      .onComposeOutput((line) => this.compose.appendLog(line))
      .then((u) => unlisteners.push(u))
      .catch(() => {});

    const onVisibility = () => {
      if (document.hidden) {
        this.stopPolling();
      } else {
        this.refreshAll();
        this.startPolling();
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    // Boot.
    this.refreshAll();
    this.startPolling();

    return () => {
      this.stopPolling();
      document.removeEventListener("visibilitychange", onVisibility);
      for (const u of unlisteners) {
        try {
          u();
        } catch {
          /* ignore */
        }
      }
    };
  }
}

const KEY = Symbol("dockwin.app");

/** Create the controller and publish it on context. Call once in the root layout. */
export function setAppController(): AppController {
  const controller = new AppController();
  setContext(KEY, controller);
  return controller;
}

/** Read the shared controller from any descendant component / route. */
export function getAppController(): AppController {
  const controller = getContext<AppController | undefined>(KEY);
  if (!controller) throw new Error("AppController not found — is this rendered under the root layout?");
  return controller;
}

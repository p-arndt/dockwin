<script lang="ts">
  // Top-level view: full-width engine header + left sidebar nav + main content.
  // Talks to dockwin-core only through src/lib/api.ts. No raw invoke here.
  import { onMount, type Component } from "svelte";
  import Container from "@lucide/svelte/icons/container";
  import Boxes from "@lucide/svelte/icons/boxes";
  import Network from "@lucide/svelte/icons/network";
  import Layers from "@lucide/svelte/icons/layers";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import PlayCircle from "@lucide/svelte/icons/circle-play";
  import CircleStop from "@lucide/svelte/icons/circle-stop";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import HelpCircle from "@lucide/svelte/icons/circle-help";
  import Settings from "@lucide/svelte/icons/settings";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import DownloadCloud from "@lucide/svelte/icons/download-cloud";
  import FileUp from "@lucide/svelte/icons/file-up";
  import FileDown from "@lucide/svelte/icons/file-down";
  import Terminal from "@lucide/svelte/icons/terminal";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Waypoints from "@lucide/svelte/icons/waypoints";
  import Gauge from "@lucide/svelte/icons/gauge";
  import Hammer from "@lucide/svelte/icons/hammer";
  import Download from "@lucide/svelte/icons/download";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import { open } from "@tauri-apps/plugin-dialog";
  import * as api from "./lib/api";
  import type {
    EngineState,
    NormalizedContainer,
    ProvisionProgress,
    Stack,
  } from "./lib/types";
  import ContainerList from "./lib/ContainerList.svelte";
  import ImagesView from "./lib/ImagesView.svelte";
  import StackList from "./lib/StackList.svelte";
  import VolumesView from "./lib/VolumesView.svelte";
  import NetworksView from "./lib/NetworksView.svelte";
  import SystemView from "./lib/SystemView.svelte";
  import ContainerDetails from "./lib/ContainerDetails.svelte";

  const POLL_MS = 3000;

  type EngineAction = "start" | "stop" | "restart" | "remove";
  type StackAction = "start" | "stop" | "restart";
  type View =
    | "containers"
    | "stacks"
    | "images"
    | "volumes"
    | "networks"
    | "system"
    | "settings";

  // --- reactive state (Svelte 5 runes) ---
  let engineState = $state<EngineState>("unknown");
  let containers = $state<NormalizedContainer[]>([]);
  let pending = $state<Set<string>>(new Set()); // container ids with an in-flight action
  let errorMsg = $state("");
  let footer = $state("Ready.");
  let footerErr = $state(false);
  let engineBusy = $state(false);
  let repairing = $state(false); // engine_repair (reset broken distro) in flight

  // Active sidebar view.
  let activeView = $state<View>("containers");
  // Bumped on manual refresh so the Images view reloads while it's open.
  let imageRefreshKey = $state(0);

  // Setup / teardown (provisioning is long-running — minutes).
  let working = $state(false); // provision or teardown in flight
  let enableTcp = $state(false); // opt into the insecure loopback-TCP fallback
  let withBackup = $state(false); // export a .tar before teardown

  // Live provisioning progress (driven by the engine://provision event).
  interface ProvisionUi {
    pct: number;
    phase: string;
    message: string;
    log: string[];
  }
  let provision = $state<ProvisionUi | null>(null);

  // Compose (docker compose up/down inside the engine).
  let composeBusy = $state(false);
  let composeLog = $state<string[]>([]);
  let composeOpen = $state(false); // show the compose output panel
  let lastComposeFile = $state<string | null>(null);

  // Container details drawer (inspect / stats / top / rename / pause).
  let selectedContainer = $state<NormalizedContainer | null>(null);

  let busy = false; // non-reactive guard against overlapping refreshes
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  // --- derived engine presentation ---
  interface EnginePresentation {
    dot: string;
    icon: Component;
    label: string;
    btn: string;
  }
  const ENGINE: Record<EngineState, EnginePresentation> = {
    running: { dot: "dot-ok", icon: PlayCircle, label: "Engine: running", btn: "Stop" },
    stopped: { dot: "dot-off", icon: CircleStop, label: "Engine: stopped", btn: "Start" },
    "not-provisioned": {
      dot: "dot-warn",
      icon: TriangleAlert,
      label: "Engine: not provisioned",
      btn: "Set up",
    },
    broken: {
      dot: "dot-warn",
      icon: TriangleAlert,
      label: "Engine: broken",
      btn: "Repair",
    },
    unknown: { dot: "dot-unknown", icon: HelpCircle, label: "Engine: unknown", btn: "—" },
  };
  let engine = $derived(ENGINE[engineState] ?? ENGINE.unknown);
  let engineToggleDisabled = $derived(
    engineBusy || working || repairing || engineState === "unknown"
  );
  let countLabel = $derived(
    containers.length ? `${containers.length} total` : ""
  );
  // Containers grouped into Docker Compose stacks (by project label).
  let stacks = $derived(api.groupStacks(containers));

  // Sidebar nav model.
  interface NavItem {
    id: View;
    label: string;
    icon: Component;
  }
  const NAV: NavItem[] = [
    { id: "containers", label: "Containers", icon: Boxes },
    { id: "stacks", label: "Stacks", icon: Network },
    { id: "images", label: "Images", icon: Layers },
    { id: "volumes", label: "Volumes", icon: HardDrive },
    { id: "networks", label: "Networks", icon: Waypoints },
    { id: "system", label: "System", icon: Gauge },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  function setView(view: View) {
    activeView = view;
  }

  function setFooter(msg: string, isError = false) {
    footer = msg;
    footerErr = isError;
  }

  // --- data flow ---
  async function refreshEngine() {
    try {
      engineState = await api.engineStatus();
    } catch (e) {
      engineState = "unknown";
      setFooter(`Engine status failed: ${api.errText(e)}`, true);
    }
  }

  async function refreshContainers() {
    // While provisioning/teardown is in flight the engine is intentionally in a
    // transitional state — don't overwrite the progress UI with a stale error.
    if (working) return;
    if (engineState !== "running") {
      containers = [];
      if (engineState === "stopped") {
        errorMsg = "Engine is stopped. Start the engine to see containers.";
      } else if (engineState === "not-provisioned") {
        errorMsg = "Engine is not provisioned. Run `dockwin provision` first.";
      } else {
        errorMsg = "";
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
      containers = list;
      errorMsg = "";
      setFooter(`Updated ${new Date().toLocaleTimeString()}.`);
    } catch (e) {
      errorMsg = `Failed to load containers: ${api.errText(e)}`;
    }
  }

  async function refreshAll() {
    if (busy) return;
    busy = true;
    try {
      await refreshEngine();
      await refreshContainers();
    } finally {
      busy = false;
    }
  }

  // Explicit user-triggered refresh: also nudge the Images view to reload.
  async function manualRefresh() {
    imageRefreshKey++;
    await refreshAll();
  }

  // pending is a Set held in $state; reassign to trigger reactivity.
  function markPending(id: string, on: boolean) {
    const next = new Set(pending);
    if (on) next.add(id);
    else next.delete(id);
    pending = next;
  }

  async function handleAction(action: EngineAction, c: NormalizedContainer) {
    if (pending.has(c.id)) return;
    if (action === "remove") {
      const note = c.running
        ? `Remove running container "${c.name}"? It will be stopped and deleted.`
        : `Remove container "${c.name}"?`;
      if (!confirm(note)) return;
    }

    markPending(c.id, true);
    setFooter(`${action} ${c.name}…`);
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
      setFooter(`${action} ${c.name} done.`);
    } catch (e) {
      setFooter(`${action} ${c.name} failed: ${api.errText(e)}`, true);
      errorMsg = `${action} failed: ${api.errText(e)}`;
    } finally {
      markPending(c.id, false);
      await refreshAll();
    }
  }

  // Apply a start/stop/restart to every (relevant) container in a Compose stack.
  async function handleStackAction(action: StackAction, stack: Stack) {
    const targets = stack.containers.filter((c) => {
      if (action === "start") return !c.running;
      if (action === "stop") return c.running;
      return true; // restart: every container in the stack
    });
    if (targets.length === 0) return;

    for (const c of targets) markPending(c.id, true);
    setFooter(`${action} stack ${stack.project} (${targets.length})…`);
    try {
      await Promise.all(
        targets.map((c) => {
          if (action === "start") return api.containerStart(c.id);
          if (action === "stop") return api.containerStop(c.id);
          return api.containerRestart(c.id);
        })
      );
      setFooter(`${action} stack ${stack.project} done.`);
    } catch (e) {
      setFooter(`${action} stack ${stack.project} failed: ${api.errText(e)}`, true);
      errorMsg = `Stack ${action} failed: ${api.errText(e)}`;
    } finally {
      for (const c of targets) markPending(c.id, false);
      await refreshAll();
    }
  }

  // Header engine button dispatcher: set up when not provisioned, else start/stop.
  function onEngineButton() {
    if (engineState === "broken") return repairEngine();
    if (engineToggleDisabled) return;
    if (engineState === "not-provisioned") return provisionEngine();
    return toggleEngine();
  }

  async function toggleEngine() {
    if (engineBusy || working) return;
    engineBusy = true;
    try {
      if (engineState === "running") {
        setFooter("Stopping engine…");
        await api.engineStop();
      } else if (engineState === "stopped") {
        setFooter("Starting engine…");
        await api.engineStart();
      }
    } catch (e) {
      setFooter(`Engine action failed: ${api.errText(e)}`, true);
    } finally {
      engineBusy = false;
      await refreshAll();
    }
  }

  // Apply a live provisioning progress event to the UI.
  function handleProvision(p: ProvisionProgress) {
    if (!working) return; // ignore stray late events
    const prev = provision ?? { pct: 0, phase: p.phase, message: p.message, log: [] };
    provision = {
      pct: Math.min(100, Math.max(prev.pct, p.pct)), // monotonic
      phase: p.phase,
      message: p.message,
      log: [...prev.log, p.message].slice(-300),
    };
    if (p.done && p.error) errorMsg = p.error;
  }

  // Provision the dedicated WSL2 engine distro (downloads the rootfs, installs
  // dockerd). Long-running; live progress arrives via engine://provision.
  async function provisionEngine() {
    if (working) return;
    working = true;
    errorMsg = "";
    provision = { pct: 0, phase: "preflight", message: "Starting setup…", log: [] };
    setFooter("Setting up engine… this can take several minutes.");
    try {
      await api.engineProvision(enableTcp);
      setFooter("Engine set up. Starting…");
    } catch (e) {
      setFooter(`Setup failed: ${api.errText(e)}`, true);
      errorMsg = `Setup failed: ${api.errText(e)}`;
    } finally {
      working = false;
      provision = null;
      await refreshAll();
    }
  }

  // --- Compose (docker compose up/down inside the dockwin engine) ---
  async function pickComposeFile(): Promise<string | null> {
    const sel = await open({
      multiple: false,
      directory: false,
      title: "Select a Docker Compose file",
      filters: [{ name: "Compose", extensions: ["yml", "yaml"] }],
    });
    return typeof sel === "string" ? sel : null;
  }

  async function composeUp() {
    if (composeBusy) return;
    const file = await pickComposeFile();
    if (!file) return;
    composeBusy = true;
    composeLog = [];
    composeOpen = true;
    lastComposeFile = file;
    activeView = "stacks";
    setFooter(`compose up: ${file}…`);
    try {
      await api.composeUp(file, false);
      setFooter("Compose up complete.");
    } catch (e) {
      setFooter(`Compose up failed: ${api.errText(e)}`, true);
    } finally {
      composeBusy = false;
      await refreshAll();
    }
  }

  async function composeDown() {
    if (composeBusy) return;
    const file = lastComposeFile ?? (await pickComposeFile());
    if (!file) return;
    composeBusy = true;
    composeOpen = true;
    lastComposeFile = file;
    setFooter(`compose down: ${file}…`);
    try {
      await api.composeDown(file);
      setFooter("Compose down complete.");
    } catch (e) {
      setFooter(`Compose down failed: ${api.errText(e)}`, true);
    } finally {
      composeBusy = false;
      await refreshAll();
    }
  }

  // build / pull / restart / logs: same flow, different compose verb. Reuses the
  // last picked file (or prompts) and streams output into the compose panel.
  async function runComposeExtra(
    label: string,
    run: (file: string) => Promise<void>
  ) {
    if (composeBusy) return;
    const file = lastComposeFile ?? (await pickComposeFile());
    if (!file) return;
    composeBusy = true;
    composeOpen = true;
    lastComposeFile = file;
    composeLog = [];
    setFooter(`compose ${label}: ${file}…`);
    try {
      await run(file);
      setFooter(`Compose ${label} complete.`);
    } catch (e) {
      setFooter(`Compose ${label} failed: ${api.errText(e)}`, true);
    } finally {
      composeBusy = false;
      await refreshAll();
    }
  }

  // Tear down the engine: permanently unregister the WSL distro + remove contexts.
  async function teardownEngine() {
    if (working) return;
    const note =
      "Remove the dockwin engine? This permanently deletes the WSL distro 'dockwin' " +
      "and all its containers and images." +
      (withBackup ? " A .tar backup is exported to your user folder first." : "");
    if (!confirm(note)) return;
    working = true;
    setFooter("Removing engine…");
    try {
      await api.engineTeardown(withBackup);
      setFooter("Engine removed.");
    } catch (e) {
      setFooter(`Teardown failed: ${api.errText(e)}`, true);
      errorMsg = `Teardown failed: ${api.errText(e)}`;
    } finally {
      working = false;
      await refreshAll();
    }
  }

  // Reset a broken engine: the 'dockwin' WSL distro is registered but its disk
  // image is missing. Unregister the dangling distro so it can be reprovisioned.
  async function repairEngine() {
    if (repairing || working) return;
    const note =
      "Reset the broken dockwin engine? This unregisters the dangling WSL distro " +
      "'dockwin' so you can set it up again from scratch.";
    if (!confirm(note)) return;
    repairing = true;
    errorMsg = "";
    setFooter("Resetting engine…");
    try {
      await api.engineRepair();
      setFooter("Engine reset. You can set it up again.");
    } catch (e) {
      setFooter(`Repair failed: ${api.errText(e)}`, true);
      errorMsg = `Repair failed: ${api.errText(e)}`;
    } finally {
      repairing = false;
      await refreshAll();
    }
  }

  // --- polling lifecycle ---
  function startPolling() {
    stopPolling();
    pollTimer = setInterval(refreshAll, POLL_MS);
  }
  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  onMount(() => {
    const unlisteners: Array<() => void> = [];

    // Optional backend-pushed refresh / engine-state events. Safe if never emitted.
    api
      .on("dockwin://refresh", () => refreshAll())
      .then((u) => unlisteners.push(u))
      .catch(() => {});
    api
      .on<string | { status?: string; state?: string }>(
        "dockwin://engine-state",
        (ev) => {
          const payload = ev?.payload;
          if (payload == null) return;
          const raw =
            typeof payload === "string"
              ? payload
              : payload.status ?? payload.state;
          engineState = api.mapEngineStatus(raw);
          refreshContainers();
        }
      )
      .then((u) => unlisteners.push(u))
      .catch(() => {});

    // Live provisioning progress + compose output.
    api
      .onProvisionProgress(handleProvision)
      .then((u) => unlisteners.push(u))
      .catch(() => {});
    api
      .onComposeOutput((line) => {
        composeLog = [...composeLog, line].slice(-500);
      })
      .then((u) => unlisteners.push(u))
      .catch(() => {});

    const onVisibility = () => {
      if (document.hidden) {
        stopPolling();
      } else {
        refreshAll();
        startPolling();
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    // Boot.
    refreshAll();
    startPolling();

    return () => {
      stopPolling();
      document.removeEventListener("visibilitychange", onVisibility);
      for (const u of unlisteners) {
        try {
          u();
        } catch {
          /* ignore */
        }
      }
    };
  });

  const EngineIcon = $derived(engine.icon);
</script>

<!-- Full-width header: brand + engine status + start/stop + refresh -->
<header
  class="flex flex-none items-center justify-between border-b border-[#262b34] bg-[#171a21] px-4 py-2.5"
>
  <div class="flex items-center gap-2.5">
    <Container size={20} class="text-[#2f81f7]" aria-hidden="true" />
    <span class="text-base font-semibold tracking-wide">dockwin</span>
  </div>
  <div class="flex items-center gap-2.5">
    <span class="dot {engine.dot}" aria-hidden="true"></span>
    <span class="flex items-center gap-1.5 text-[13px] text-[#9aa3af]">
      <EngineIcon size={15} aria-hidden="true" />
      {engine.label}
    </span>
    <button
      class="min-w-[78px] cursor-pointer rounded-md border border-[#262b34] bg-[#21262d] px-3 py-[5px] text-[13px] text-[#e6e8eb] transition-colors hover:not-disabled:border-[#3a414b] hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-45"
      disabled={engineToggleDisabled || engine.btn === "—"}
      onclick={onEngineButton}>{engine.btn}</button
    >
    <button
      class="flex cursor-pointer items-center rounded-md border border-transparent bg-transparent px-2.5 py-[5px] text-[#e6e8eb] transition-colors hover:bg-[#21262d] disabled:opacity-45"
      title="Refresh"
      disabled={working}
      onclick={manualRefresh}
    >
      <RefreshCw size={15} aria-hidden="true" />
    </button>
  </div>
</header>

<!-- Body: fixed-width sidebar nav + scrollable main content -->
<div class="flex min-h-0 flex-1">
  <nav
    class="flex w-[188px] flex-none flex-col gap-1 border-r border-[#262b34] bg-[#171a21] p-2"
    aria-label="Primary"
  >
    {#each NAV as item (item.id)}
      {@const ItemIcon = item.icon}
      {@const active = activeView === item.id}
      <button
        class="flex cursor-pointer items-center gap-2.5 rounded-md border border-transparent px-3 py-2 text-left text-[13px] transition-colors {active
          ? 'bg-[#1f6feb1a] text-[#e6e8eb] border-[#2f81f74d]'
          : 'text-[#9aa3af] hover:bg-[#21262d] hover:text-[#e6e8eb]'}"
        aria-current={active ? "page" : undefined}
        onclick={() => setView(item.id)}
      >
        <ItemIcon size={16} aria-hidden="true" />
        <span class="font-medium">{item.label}</span>
      </button>
    {/each}
  </nav>

  <main class="flex min-w-0 flex-1 flex-col gap-4 overflow-auto p-4">
    {#if working && provision}
      <!-- Live provisioning progress (replaces the setup panel while running). -->
      <section class="rounded-md border border-[#262b34] bg-[#171a21] p-5">
        <div class="mb-3 flex items-center gap-2.5">
          <DownloadCloud size={20} class="text-[#2f81f7]" aria-hidden="true" />
          <h2 class="text-base font-semibold">Setting up the dockwin engine…</h2>
          <span class="ml-auto font-mono-app text-sm text-[#9aa3af]">
            {Math.round(provision.pct)}%
          </span>
        </div>
        <!-- Progress bar -->
        <div class="h-2.5 w-full overflow-hidden rounded-full bg-[#21262d]">
          <div
            class="provision-bar h-full rounded-full bg-[#1f6feb] transition-[width] duration-300 ease-out"
            style="width: {Math.max(2, provision.pct)}%"
          ></div>
        </div>
        <p class="mt-3 truncate text-[13px] text-[#c7ccd4]" title={provision.message}>
          {provision.message}
        </p>
        <!-- Live log (most recent at the bottom) -->
        {#if provision.log.length}
          <div
            class="mt-3 max-h-44 select-text overflow-auto rounded-md border border-[#262b34] bg-[#0d1117] p-2.5 font-mono-app text-[11.5px] leading-relaxed text-[#9aa3af]"
          >
            {#each provision.log.slice(-12) as line, i (i)}
              <div class="whitespace-pre-wrap break-all">{line}</div>
            {/each}
          </div>
        {/if}
        <p class="mt-3 text-xs text-[#6e7681]">
          Downloading the Ubuntu image and installing Docker. You can keep this
          window open — it'll finish on its own.
        </p>
      </section>
    {:else if engineState === "not-provisioned"}
      <!-- First-run setup: shown in every view so it can't be missed. -->
      <section class="rounded-md border border-[#262b34] bg-[#171a21] p-5">
        <div class="mb-2 flex items-center gap-2.5">
          <DownloadCloud size={20} class="text-[#2f81f7]" aria-hidden="true" />
          <h2 class="text-base font-semibold">Set up the dockwin engine</h2>
        </div>
        <p class="mb-3 max-w-prose text-[13px] leading-relaxed text-[#9aa3af]">
          dockwin runs Docker in a dedicated, isolated WSL2 distro — no Docker
          Desktop required. Setting up downloads a minimal Ubuntu image (~250&nbsp;MB)
          and installs the Docker Engine. This can take a few minutes; you can keep
          this window open.
        </p>
        <label class="mb-3 flex items-center gap-2 text-[13px] text-[#c7ccd4]">
          <input type="checkbox" bind:checked={enableTcp} disabled={working} />
          Also enable insecure loopback TCP (127.0.0.1:2375) — not recommended
        </label>
        <button
          class="flex items-center gap-2 rounded-md border border-[#2f81f7] bg-[#1f6feb] px-4 py-2 text-sm font-medium text-white transition-colors hover:not-disabled:bg-[#2f81f7] disabled:cursor-default disabled:opacity-60"
          disabled={working}
          onclick={provisionEngine}
        >
          <DownloadCloud size={16} aria-hidden="true" />
          {working ? "Setting up…" : "Set up engine"}
        </button>
      </section>
    {:else if engineState === "broken"}
      <!-- Broken engine: distro registered but its disk image is missing. -->
      <section
        class="rounded-md border border-[#f8514966] bg-[#f851491a] p-5"
      >
        <div class="mb-2 flex items-center gap-2.5">
          <TriangleAlert size={20} class="text-[#f85149]" aria-hidden="true" />
          <h2 class="text-base font-semibold text-[#ff9b95]">Engine is broken</h2>
        </div>
        <p class="mb-3 max-w-prose text-[13px] leading-relaxed text-[#ffb3ae]">
          The dockwin WSL distro is registered but its disk image is missing.
          Reset it to unregister the dangling distro, then set the engine up again
          to reprovision.
        </p>
        <button
          class="flex items-center gap-2 rounded-md border border-[#f8514966] bg-[#f851491a] px-4 py-2 text-sm font-medium text-[#ff9b95] transition-colors hover:not-disabled:bg-[#f8514926] disabled:cursor-default disabled:opacity-60"
          disabled={repairing || working}
          onclick={repairEngine}
        >
          <Hammer size={16} aria-hidden="true" />
          {repairing ? "Resetting…" : "Repair engine"}
        </button>
      </section>
    {/if}

    {#if activeView === "containers"}
      <!-- Containers -->
      <section
        class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]"
      >
        <div
          class="flex items-baseline gap-2.5 border-b border-[#262b34] px-3.5 py-3"
        >
          <h2 class="text-sm font-semibold">Containers</h2>
          <span class="text-xs text-[#9aa3af]">{countLabel}</span>
        </div>
        {#if errorMsg}
          <div
            class="mx-3.5 mt-3 select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
          >
            {errorMsg}
          </div>
        {/if}
        <ContainerList
          {containers}
          {pending}
          onAction={handleAction}
          onSelect={(c) => (selectedContainer = c)}
        />
      </section>
    {:else if activeView === "stacks"}
      <!-- Compose stacks (containers grouped by project) -->
      <div class="flex items-center gap-2.5 px-0.5">
        <h2 class="text-sm font-semibold">Stacks</h2>
        <span class="text-xs text-[#9aa3af]">
          {stacks.length ? `${stacks.length} project${stacks.length > 1 ? "s" : ""}` : ""}
        </span>
        <div class="ml-auto flex items-center gap-1.5">
          <button
            class="flex items-center gap-1.5 rounded-md border border-[#238636]/60 bg-[#2386361a] px-2.5 py-[5px] text-[12px] text-[#5ad17a] transition-colors hover:not-disabled:bg-[#23863626] disabled:cursor-default disabled:opacity-40"
            title="Pick a docker-compose.yml and run it on the dockwin engine"
            disabled={composeBusy || engineState !== "running"}
            onclick={composeUp}
          >
            <FileUp size={14} aria-hidden="true" />
            {composeBusy ? "Working…" : "Compose up…"}
          </button>
          <button
            class="flex items-center gap-1.5 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
            title="docker compose down for a compose file"
            disabled={composeBusy || engineState !== "running"}
            onclick={composeDown}
          >
            <FileDown size={14} aria-hidden="true" />
            Down…
          </button>
          <button
            class="flex items-center gap-1.5 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
            title="docker compose pull"
            disabled={composeBusy || engineState !== "running"}
            onclick={() => runComposeExtra("pull", api.composePull)}
          >
            <Download size={14} aria-hidden="true" /> Pull
          </button>
          <button
            class="flex items-center gap-1.5 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
            title="docker compose build"
            disabled={composeBusy || engineState !== "running"}
            onclick={() => runComposeExtra("build", api.composeBuild)}
          >
            <Hammer size={14} aria-hidden="true" /> Build
          </button>
          <button
            class="flex items-center gap-1.5 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
            title="docker compose logs (tail)"
            disabled={composeBusy || engineState !== "running"}
            onclick={() => runComposeExtra("logs", (f) => api.composeLogs(f))}
          >
            <ScrollText size={14} aria-hidden="true" /> Logs
          </button>
        </div>
      </div>
      {#if engineState === "running"}
        <p class="px-0.5 text-xs text-[#6e7681]">
          Tip: in a terminal you can also run
          <code class="text-[#9aa3af]">dockwin up</code> from a folder with a
          <code class="text-[#9aa3af]">docker-compose.yml</code> (use this instead of
          <code class="text-[#9aa3af]">docker compose</code>, which targets Docker Desktop).
        </p>
      {/if}
      {#if errorMsg}
        <div
          class="select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
        >
          {errorMsg}
        </div>
      {/if}
      {#if composeOpen && composeLog.length}
        <section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
          <div class="flex items-center gap-2 border-b border-[#262b34] px-3 py-2">
            <Terminal size={14} class="text-[#9aa3af]" aria-hidden="true" />
            <span class="text-[12px] font-semibold">Compose output</span>
            {#if lastComposeFile}
              <span class="truncate font-mono-app text-[11px] text-[#6e7681]" title={lastComposeFile}>
                {lastComposeFile}
              </span>
            {/if}
            <button
              class="ml-auto cursor-pointer rounded px-1.5 py-0.5 text-[11px] text-[#9aa3af] hover:bg-[#21262d]"
              onclick={() => (composeOpen = false)}
            >
              Hide
            </button>
          </div>
          <div
            class="max-h-56 select-text overflow-auto bg-[#0d1117] p-2.5 font-mono-app text-[11.5px] leading-relaxed text-[#9aa3af]"
          >
            {#each composeLog as line, i (i)}
              <div class="whitespace-pre-wrap break-all">{line}</div>
            {/each}
          </div>
        </section>
      {/if}
      <StackList {stacks} {pending} onStackAction={handleStackAction} />
    {:else if activeView === "images"}
      <!-- Images (pull / remove / prune / tag / history / inspect) -->
      <ImagesView {engineState} refreshKey={imageRefreshKey} />
    {:else if activeView === "volumes"}
      <VolumesView {engineState} refreshKey={imageRefreshKey} />
    {:else if activeView === "networks"}
      <NetworksView {engineState} refreshKey={imageRefreshKey} />
    {:else if activeView === "system"}
      <SystemView {engineState} refreshKey={imageRefreshKey} />
    {:else if activeView === "settings"}
      <!-- Settings / teardown -->
      <section class="rounded-md border border-[#262b34] bg-[#171a21] p-3.5">
        <div class="mb-2 flex items-center gap-2">
          <Settings size={16} class="text-[#9aa3af]" aria-hidden="true" />
          <h2 class="text-sm font-semibold">Settings</h2>
        </div>
        <div class="flex flex-col gap-3">
          <p class="max-w-prose text-[13px] leading-relaxed text-[#9aa3af]">
            The engine listens on the Windows named pipe by default. The insecure
            loopback-TCP endpoint (127.0.0.1:2375) is only enabled if you opted in
            during setup — it is not recommended for normal use.
          </p>
          <label class="flex items-center gap-2 text-[13px] text-[#c7ccd4]">
            <input type="checkbox" bind:checked={withBackup} />
            Export a <code class="text-[#9aa3af]">.tar</code> backup before removing
          </label>
          <div class="flex items-center gap-3">
            <button
              class="flex items-center gap-1.5 rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-[6px] text-[13px] text-[#ff9b95] transition-colors hover:not-disabled:bg-[#f8514926] disabled:cursor-default disabled:opacity-45"
              disabled={working || engineState === "not-provisioned" || engineState === "unknown"}
              onclick={teardownEngine}
            >
              <Trash2 size={15} aria-hidden="true" />
              Remove engine
            </button>
            {#if engineState === "not-provisioned"}
              <span class="text-xs text-[#9aa3af]">Nothing to remove — engine not set up.</span>
            {/if}
          </div>
        </div>
      </section>
    {/if}
  </main>
</div>

<!-- Container details drawer (fixed; self-positioned). Open via a name click. -->
{#if selectedContainer}
  <ContainerDetails
    id={selectedContainer.id}
    name={selectedContainer.name}
    running={selectedContainer.running}
    {engineState}
    onClose={() => (selectedContainer = null)}
    onChanged={refreshAll}
  />
{/if}

<footer
  class="flex-none border-t border-[#262b34] bg-[#171a21] px-4 py-1.5 text-xs"
>
  <span class={footerErr ? "text-[#f85149]" : "text-[#9aa3af]"}>{footer}</span>
</footer>

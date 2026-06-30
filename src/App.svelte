<script lang="ts">
  // Top-level shell: branded sidebar (Workloads / Resources + counts + active
  // rail + engine pod) and a slim top ctx bar (engine status, theme + accent
  // toggles). Talks to dockwin-core only through src/lib/api.ts. No raw invoke.
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
  import FileUp from "@lucide/svelte/icons/file-up";
  import FileDown from "@lucide/svelte/icons/file-down";
  import Terminal from "@lucide/svelte/icons/terminal";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Waypoints from "@lucide/svelte/icons/waypoints";
  import Gauge from "@lucide/svelte/icons/gauge";
  import Hammer from "@lucide/svelte/icons/hammer";
  import Download from "@lucide/svelte/icons/download";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import Moon from "@lucide/svelte/icons/moon";
  import Sun from "@lucide/svelte/icons/sun";
  import Search from "@lucide/svelte/icons/search";
  import { open } from "@tauri-apps/plugin-dialog";
  import * as api from "./lib/api";
  import { theme, ACCENT_SHADES } from "./lib/theme.svelte";
  import type {
    EngineState,
    NormalizedContainer,
    ProvisionProgress,
    ProvisionUi,
    Stack,
  } from "./lib/types";
  import EngineGate from "./lib/EngineGate.svelte";
  import ContainerList from "./lib/ContainerList.svelte";
  import ImagesView from "./lib/ImagesView.svelte";
  import StackList from "./lib/StackList.svelte";
  import VolumesView from "./lib/VolumesView.svelte";
  import NetworksView from "./lib/NetworksView.svelte";
  import SystemView from "./lib/SystemView.svelte";
  import ContainerDetails from "./lib/ContainerDetails.svelte";
  import UpdateBanner from "./lib/UpdateBanner.svelte";

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
  let provision = $state<ProvisionUi | null>(null);

  // Compose (docker compose up/down inside the engine).
  let composeBusy = $state(false);
  let composeLog = $state<string[]>([]);
  let composeOpen = $state(false); // show the compose output panel
  let lastComposeFile = $state<string | null>(null);

  // Container details: selected container + whether it's shown as the full-width
  // page (true) or the right-side drawer (false). FULL-DETAIL routing.
  let selectedContainer = $state<NormalizedContainer | null>(null);
  let detailFull = $state(false);

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
  // The management shell (sidebar + resource views) only mounts once the engine
  // is running. Every other state is owned by the full-window EngineGate, so the
  // engine lifecycle never bleeds into the management UI.
  let engineReady = $derived(engineState === "running");
  let engineToggleDisabled = $derived(
    engineBusy || working || repairing || engineState === "unknown"
  );
  // Tone for the quiet status dots (ctx bar + engine pod).
  let engineTone = $derived(
    engineState === "running" ? "live" : engineState === "stopped" ? "off" : "warn"
  );
  let runningCount = $derived(containers.filter((c) => c.running).length);
  // Containers grouped into Docker Compose stacks (by project label).
  let stacks = $derived(api.groupStacks(containers));

  // Sidebar nav model (grouped into Workloads / Resources sections).
  type NavSection = "Workloads" | "Resources";
  interface NavItem {
    id: View;
    label: string;
    icon: Component;
    section: NavSection;
  }
  const NAV: NavItem[] = [
    { id: "containers", label: "Containers", icon: Boxes, section: "Workloads" },
    { id: "stacks", label: "Stacks", icon: Network, section: "Workloads" },
    { id: "images", label: "Images", icon: Layers, section: "Workloads" },
    { id: "volumes", label: "Volumes", icon: HardDrive, section: "Resources" },
    { id: "networks", label: "Networks", icon: Waypoints, section: "Resources" },
    { id: "system", label: "System", icon: Gauge, section: "Resources" },
  ];
  const NAV_SECTIONS: NavSection[] = ["Workloads", "Resources"];

  // Counts shown on the rail (null = not tracked here).
  function navCount(id: View): number | null {
    if (id === "containers") return containers.length;
    if (id === "stacks") return stacks.length;
    return null;
  }

  function setView(view: View) {
    activeView = view;
  }

  function setFooter(msg: string, isError = false) {
    footer = msg;
    footerErr = isError;
  }

  // --- container details routing ---
  function selectContainer(c: NormalizedContainer) {
    selectedContainer = c;
    detailFull = false; // a fresh selection always opens as the drawer
  }
  function closeDetail() {
    selectedContainer = null;
    detailFull = false;
  }
  function toggleDetailFull() {
    detailFull = !detailFull;
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
  // "Engine: running" → "Engine running" for the compact status lines.
  let engineLine = $derived(engine.label.replace(": ", " "));
</script>

{#snippet themeControls()}
  <div class="seg" aria-label="Theme">
    <button
      aria-pressed={theme.theme === "dark"}
      onclick={() => theme.setTheme("dark")}
    >
      <Moon aria-hidden="true" />Dark
    </button>
    <button
      aria-pressed={theme.theme === "light"}
      onclick={() => theme.setTheme("light")}
    >
      <Sun aria-hidden="true" />Light
    </button>
  </div>
  <div class="sw" title="Accent shade">
    {#each ACCENT_SHADES as _shade, i (i)}
      <button
        class={"l" + (i + 1)}
        class:a={theme.accent === i}
        aria-label={`Accent shade ${i + 1}`}
        onclick={() => theme.setAccent(i)}
      ></button>
    {/each}
  </div>
{/snippet}

{#if !engineReady}
  <!-- Engine lifecycle: a slim branded bar (so theme is still toggleable) + the
       full-window EngineGate, which owns set-up / progress / start / unreachable. -->
  <div class="solo">
    <div class="ctx">
      <span class="brand" style="padding:0;gap:9px">
        <span class="logo" style="width:26px;height:26px;border-radius:8px">
          <Container size={15} aria-hidden="true" />
        </span>
        <span class="bt" style="font-size:14px">dockwin</span>
      </span>
      <span class="sep"></span>
      <span class="live {engineTone}"><span class="d"></span>{engineLine}</span>
      <span class="sp"></span>
      {@render themeControls()}
      <span class="sep"></span>
      <button
        class="btn btn-icon"
        title="Refresh"
        disabled={working}
        onclick={manualRefresh}
      >
        <RefreshCw aria-hidden="true" />
      </button>
    </div>
    <EngineGate
      {engineState}
      {working}
      {provision}
      {engineBusy}
      {repairing}
      bind:enableTcp
      onProvision={provisionEngine}
      onStart={toggleEngine}
      onRepair={repairEngine}
      onRetry={manualRefresh}
    />
  </div>
{:else}
  <div class="app">
    <!-- ===== SIDEBAR ===== -->
    <aside class="side">
      <div class="brand">
        <span class="logo"><Container size={19} aria-hidden="true" /></span>
        <div>
          <div class="bt">dockwin</div>
          <div class="bs">Docker workspace</div>
        </div>
      </div>

      {#each NAV_SECTIONS as section (section)}
        <div class="navsec">{section}</div>
        <nav class="nav" aria-label={section}>
          {#each NAV.filter((n) => n.section === section) as item (item.id)}
            {@const ItemIcon = item.icon}
            {@const count = navCount(item.id)}
            <button
              class:on={activeView === item.id}
              aria-current={activeView === item.id ? "page" : undefined}
              onclick={() => setView(item.id)}
            >
              <ItemIcon aria-hidden="true" />
              {item.label}
              {#if count !== null}<span class="ct">{count}</span>{/if}
            </button>
          {/each}
        </nav>
      {/each}

      <div class="eng">
        <div class="row">
          <span class="dot {engineTone}"></span>
          <div>
            <div class="et">{engineLine}</div>
            <div class="es">WSL2 backend</div>
          </div>
        </div>
      </div>
    </aside>

    <!-- ===== MAIN ===== -->
    <main class="main">
      <!-- slim ctx bar -->
      <div class="ctx">
        <span class="live {engineTone}"><span class="d"></span>{engineLine}</span>
        <span class="sep"></span>
        <span class="num">{containers.length} containers</span>
        <span class="sep"></span>
        <span>WSL2 backend</span>
        <span class="sp"></span>
        {@render themeControls()}
        <span class="sep"></span>
        <button
          class="btn btn-icon"
          class:on={activeView === "settings"}
          title="Settings"
          onclick={() => setView("settings")}
        >
          <Settings aria-hidden="true" />
        </button>
        <button
          class="btn btn-icon"
          title="Refresh"
          disabled={working}
          onclick={manualRefresh}
        >
          <RefreshCw aria-hidden="true" />
        </button>
        {#if engineState === "running" || engineState === "stopped"}
          <button
            class="btn btn-soft"
            style="min-width:74px;justify-content:center"
            disabled={engineToggleDisabled}
            onclick={toggleEngine}
          >
            {#if engineState === "running"}
              <CircleStop aria-hidden="true" />{engineBusy ? "Stopping…" : "Stop"}
            {:else}
              <PlayCircle aria-hidden="true" />{engineBusy ? "Starting…" : "Start"}
            {/if}
          </button>
        {/if}
      </div>

      {#if activeView === "containers"}
        {#if selectedContainer && detailFull}
          <!-- FULL-DETAIL: the list is hidden, ContainerDetails fills the pane. -->
          <div class="detail-full">
            <ContainerDetails
              container={selectedContainer}
              full={true}
              onClose={closeDetail}
              onToggleFull={toggleDetailFull}
            />
          </div>
        {:else}
          <div class="head">
            <h1>Containers</h1>
            <span class="chip">
              <span class="d"></span><b class="num">{runningCount}</b> running
              <span class="x">·</span><b class="num">{containers.length}</b> total
            </span>
            <span class="sp"></span>
            <div class="search" aria-hidden="true">
              <Search aria-hidden="true" /><span>Search</span><kbd>Ctrl K</kbd>
            </div>
          </div>
          <div class="body" class:split={!!selectedContainer}>
            <div class="list">
              {#if errorMsg}
                <div class="banner err" style="margin-bottom:14px">
                  <TriangleAlert aria-hidden="true" />{errorMsg}
                </div>
              {/if}
              <ContainerList
                {containers}
                {pending}
                onAction={handleAction}
                onSelect={selectContainer}
              />
            </div>
            {#if selectedContainer}
              <ContainerDetails
                container={selectedContainer}
                full={false}
                onClose={closeDetail}
                onToggleFull={toggleDetailFull}
              />
            {/if}
          </div>
        {/if}
      {:else if activeView === "stacks"}
        <div class="head">
          <h1>Stacks</h1>
          <span class="chip">
            <b class="num">{stacks.length}</b>
            {stacks.length === 1 ? "project" : "projects"}
          </span>
          <span class="sp"></span>
          <span class="btn-split">
            <button
              class="btn btn-pri"
              title="Pick a docker-compose.yml and run it on the dockwin engine"
              disabled={composeBusy || engineState !== "running"}
              onclick={composeUp}
            >
              <FileUp aria-hidden="true" />
              {composeBusy ? "Working…" : "Compose up"}
            </button>
          </span>
          <button
            class="btn btn-soft"
            title="docker compose down"
            disabled={composeBusy || engineState !== "running"}
            onclick={composeDown}
          >
            <FileDown aria-hidden="true" />Down
          </button>
          <button
            class="btn btn-soft"
            title="docker compose pull"
            disabled={composeBusy || engineState !== "running"}
            onclick={() => runComposeExtra("pull", api.composePull)}
          >
            <Download aria-hidden="true" />Pull
          </button>
          <button
            class="btn btn-soft"
            title="docker compose build"
            disabled={composeBusy || engineState !== "running"}
            onclick={() => runComposeExtra("build", api.composeBuild)}
          >
            <Hammer aria-hidden="true" />Build
          </button>
          <button
            class="btn btn-soft"
            title="docker compose logs (tail)"
            disabled={composeBusy || engineState !== "running"}
            onclick={() => runComposeExtra("logs", (f) => api.composeLogs(f))}
          >
            <ScrollText aria-hidden="true" />Logs
          </button>
        </div>
        <div class="body">
          <div class="page" style="padding-top:0">
            {#if engineState === "running"}
              <p class="prose">
                Tip: in a terminal you can also run
                <code class="code">dockwin up</code> from a folder with a
                <code class="code">docker-compose.yml</code> (use this instead of
                <code class="code">docker compose</code>, which targets Docker Desktop).
              </p>
            {/if}
            {#if errorMsg}
              <div class="banner err"><TriangleAlert aria-hidden="true" />{errorMsg}</div>
            {/if}
            {#if composeOpen && composeLog.length}
              <div class="outpane">
                <div class="bar">
                  <Terminal aria-hidden="true" />
                  <span style="font-weight:600;color:var(--text)">Compose output</span>
                  {#if lastComposeFile}
                    <span class="mono" style="font-size:11px;color:var(--text-4);overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title={lastComposeFile}>
                      {lastComposeFile}
                    </span>
                  {/if}
                  <button
                    class="btn btn-soft sm"
                    style="margin-left:auto"
                    onclick={() => (composeOpen = false)}
                  >
                    Hide
                  </button>
                </div>
                <div class="body-out">
                  {#each composeLog as line, i (i)}
                    <div style="white-space:pre-wrap;word-break:break-all">{line}</div>
                  {/each}
                </div>
              </div>
            {/if}
            <StackList {stacks} {pending} onStackAction={handleStackAction} />
          </div>
        </div>
      {:else if activeView === "images"}
        <div class="body"><ImagesView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "volumes"}
        <div class="body"><VolumesView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "networks"}
        <div class="body"><NetworksView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "system"}
        <div class="body"><SystemView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "settings"}
        <div class="head"><h1>Settings</h1></div>
        <div class="body">
          <div class="page">
            <div class="card card-pad" style="max-width:60ch">
              <div class="section-title" style="margin-bottom:12px">Engine</div>
              <div style="display:flex;flex-direction:column;gap:14px">
                <p class="prose" style="margin:0">
                  The engine listens on the Windows named pipe by default. The
                  insecure loopback-TCP endpoint (127.0.0.1:2375) is only enabled
                  if you opted in during setup — it is not recommended for normal
                  use.
                </p>
                <label class="field">
                  <input type="checkbox" bind:checked={withBackup} />
                  Export a <code class="code">.tar</code> backup before removing
                </label>
                <div>
                  <button
                    class="btn btn-danger"
                    disabled={working}
                    onclick={teardownEngine}
                  >
                    <Trash2 aria-hidden="true" />Remove engine
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      {/if}

      <div class="statusbar" class:err={footerErr}>{footer}</div>
    </main>
  </div>
{/if}

<!-- In-app update toast (app + engine). Fixed-position; checks on mount. -->
<UpdateBanner />

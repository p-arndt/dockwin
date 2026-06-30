<script lang="ts">
  // Root shell + screen router. Owns the engine lifecycle, container data/actions
  // and polling, then delegates each screen to a view component. The engine status
  // is anchored in the sidebar pod; the top bar holds the active context + actions.
  // Talks to dockwin-core only through src/lib/api (no raw invoke).
  import { onMount } from "svelte";
  import Container from "@lucide/svelte/icons/container";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import * as api from "./lib/api";
  import type {
    EngineState,
    NormalizedContainer,
    ProvisionProgress,
    ProvisionUi,
    Stack,
  } from "./lib/types";
  import { confirmDialog } from "./lib/state/confirm.svelte";
  import { createCompose } from "./lib/state/compose.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import AppSidebar, { NAV, type View } from "./lib/components/AppSidebar.svelte";
  import ThemeToggle from "./lib/components/ThemeToggle.svelte";
  import ConfirmHost from "./lib/components/ConfirmHost.svelte";
  import EngineGate from "./lib/views/EngineGate.svelte";
  import TopBar from "./lib/views/TopBar.svelte";
  import ContainersView from "./lib/views/ContainersView.svelte";
  import StacksView from "./lib/views/StacksView.svelte";
  import ImagesView from "./lib/views/ImagesView.svelte";
  import VolumesView from "./lib/views/VolumesView.svelte";
  import NetworksView from "./lib/views/NetworksView.svelte";
  import SystemView from "./lib/views/SystemView.svelte";
  import SettingsView from "./lib/views/SettingsView.svelte";
  import UpdateBanner from "./lib/views/UpdateBanner.svelte";

  const POLL_MS = 3000;

  type EngineAction = "start" | "stop" | "restart" | "remove";
  type StackAction = "start" | "stop" | "restart";

  // --- reactive state (Svelte 5 runes) ---
  let engineState = $state<EngineState>("unknown");
  let containers = $state<NormalizedContainer[]>([]);
  let pending = $state<Set<string>>(new Set()); // container ids with an in-flight action
  let errorMsg = $state("");
  let footer = $state("Ready.");
  let footerErr = $state(false);
  let engineBusy = $state(false);
  let repairing = $state(false); // engine_repair (reset broken distro) in flight

  let activeView = $state<View>("containers");
  // Bumped on manual refresh so the open resource view reloads.
  let imageRefreshKey = $state(0);

  // Setup / teardown (provisioning is long-running — minutes).
  let working = $state(false); // provision or teardown in flight
  let enableTcp = $state(false); // opt into the insecure loopback-TCP fallback
  let withBackup = $state(false); // export a .tar before teardown
  let provision = $state<ProvisionUi | null>(null); // live provisioning progress

  // Container details: selected container + whether it's the full-width page
  // (true) or the right-side drawer (false).
  let selectedContainer = $state<NormalizedContainer | null>(null);
  let detailFull = $state(false);

  let busy = false; // non-reactive guard against overlapping refreshes
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  // --- derived ---
  // The management shell only mounts once the engine is running; every other
  // state is owned by the full-window EngineGate, so the engine lifecycle never
  // bleeds into the management UI.
  let engineReady = $derived(engineState === "running");
  let engineToggleDisabled = $derived(
    engineBusy || working || repairing || engineState === "unknown"
  );
  // Tone for the quiet status dots (top bar + engine pod).
  let engineTone = $derived(
    engineState === "running" ? "live" : engineState === "stopped" ? "off" : "warn"
  );
  // Compact engine status line shown in the sidebar pod + engine-gate bar.
  let engineLine = $derived(
    engineState === "running"
      ? "Engine running"
      : engineState === "stopped"
        ? "Engine stopped"
        : engineState === "not-provisioned"
          ? "Engine not provisioned"
          : engineState === "broken"
            ? "Engine broken"
            : "Engine unknown"
  );
  let runningCount = $derived(containers.filter((c) => c.running).length);
  // Containers grouped into Docker Compose stacks (by project label).
  let stacks = $derived(api.groupStacks(containers));
  // Per-view counts shown as nav badges (AppSidebar owns the nav model itself).
  let navCounts = $derived<Partial<Record<View, number>>>({
    containers: containers.length,
    stacks: stacks.length,
  });
  // Active-view label for the top-bar breadcrumb.
  let activeViewLabel = $derived(
    activeView === "settings"
      ? "Settings"
      : (NAV.find((n) => n.id === activeView)?.label ?? "")
  );

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

  // Explicit user-triggered refresh: also nudge the open resource view to reload.
  async function manualRefresh() {
    imageRefreshKey++;
    await refreshAll();
  }

  // Compose state + actions live in their own controller (created here so the
  // state survives navigating away from and back to the Stacks screen).
  const compose = createCompose({ setFooter, refreshAll });

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

  // Tear down the engine: permanently unregister the WSL distro + remove contexts.
  async function teardownEngine() {
    if (working) return;
    const ok = await confirmDialog({
      title: "Remove the dockwin engine?",
      description:
        "This permanently deletes the WSL distro 'dockwin' and all its containers and images." +
        (withBackup ? " A .tar backup is exported to your user folder first." : ""),
      destructive: true,
      confirmText: "Remove engine",
    });
    if (!ok) return;
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
    const ok = await confirmDialog({
      title: "Reset the broken engine?",
      description:
        "This unregisters the dangling WSL distro 'dockwin' so you can set it up again from scratch.",
      destructive: true,
      confirmText: "Reset",
    });
    if (!ok) return;
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
      .on<string | { status?: string; state?: string }>("dockwin://engine-state", (ev) => {
        const payload = ev?.payload;
        if (payload == null) return;
        const raw = typeof payload === "string" ? payload : payload.status ?? payload.state;
        engineState = api.mapEngineStatus(raw);
        refreshContainers();
      })
      .then((u) => unlisteners.push(u))
      .catch(() => {});

    // Live provisioning progress + compose output.
    api
      .onProvisionProgress(handleProvision)
      .then((u) => unlisteners.push(u))
      .catch(() => {});
    api
      .onComposeOutput((line) => compose.appendLog(line))
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
</script>

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
      <ThemeToggle />
      <span class="sep"></span>
      <Button variant="outline" size="icon" title="Refresh" disabled={working} onclick={manualRefresh}>
        <RefreshCw aria-hidden="true" />
      </Button>
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
  <Sidebar.Provider class="h-screen overflow-hidden">
    <AppSidebar {activeView} counts={navCounts} {engineTone} {engineLine} onSelect={setView} />

    <Sidebar.Inset class="main">
      <TopBar
        label={activeViewLabel}
        {engineState}
        settingsActive={activeView === "settings"}
        {engineBusy}
        {working}
        {engineToggleDisabled}
        onSettings={() => setView("settings")}
        onRefresh={manualRefresh}
        onToggleEngine={toggleEngine}
      />

      {#if activeView === "containers"}
        <ContainersView
          {containers}
          {pending}
          {runningCount}
          {errorMsg}
          selected={selectedContainer}
          {detailFull}
          onAction={handleAction}
          onSelect={selectContainer}
          onCloseDetail={closeDetail}
          onToggleFull={toggleDetailFull}
        />
      {:else if activeView === "stacks"}
        <StacksView {stacks} {pending} {engineState} {errorMsg} {compose} onStackAction={handleStackAction} />
      {:else if activeView === "images"}
        <div class="body"><ImagesView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "volumes"}
        <div class="body"><VolumesView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "networks"}
        <div class="body"><NetworksView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "system"}
        <div class="body"><SystemView {engineState} refreshKey={imageRefreshKey} /></div>
      {:else if activeView === "settings"}
        <SettingsView {working} bind:withBackup onTeardown={teardownEngine} />
      {/if}

      <div class="statusbar" class:err={footerErr}>{footer}</div>
    </Sidebar.Inset>
  </Sidebar.Provider>
{/if}

<!-- In-app update toast (app + engine). Fixed-position; checks on mount. -->
<UpdateBanner />

<!-- Single mounted host for the shared confirm dialog (confirmDialog()). -->
<ConfirmHost />

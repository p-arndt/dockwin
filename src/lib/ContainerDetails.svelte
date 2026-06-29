<script lang="ts">
  // Right-side drawer showing details for a single container: a live resource
  // "Stats" snapshot (polled), the full "Inspect" JSON, and the in-container
  // process "Top" table, plus pause/unpause + rename actions. Stateless toward
  // its parent: all engine calls go through containersApi; results bubble back
  // via onChanged so the parent can refresh its list. Svelte 5 runes.
  import X from "@lucide/svelte/icons/x";
  import Activity from "@lucide/svelte/icons/activity";
  import FileText from "@lucide/svelte/icons/file-text";
  import List from "@lucide/svelte/icons/list";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Pencil from "@lucide/svelte/icons/pencil";
  import type { EngineState } from "./types";
  import {
    containerInspect,
    containerRename,
    containerPause,
    containerUnpause,
    containerTop,
    containerStats,
    humanBytes,
    type ContainerStatsDto,
    type ContainerTopDto,
  } from "./containersApi";

  interface Props {
    id: string;
    name: string;
    running: boolean;
    engineState: EngineState;
    onClose: () => void;
    onChanged?: () => void;
  }

  let { id, name, running, engineState, onClose, onChanged }: Props = $props();

  type Tab = "stats" | "inspect" | "top";
  let activeTab = $state<Tab>("stats");

  const shortId = $derived(id.slice(0, 12));
  const engineReady = $derived(engineState === "running");

  // Refined from the inspect payload (a container can be running-but-paused).
  let paused = $state(false);

  // Action row (pause/unpause/rename).
  let actionError = $state<string | null>(null);
  let busyAction = $state(false);
  let renaming = $state(false);
  // Seeded from `name` by the id-change effect below (kept out of the initializer
  // so it doesn't capture only the first prop value).
  let renameValue = $state("");

  // Stats tab.
  let stats = $state<ContainerStatsDto | null>(null);
  let statsError = $state<string | null>(null);

  // Inspect tab.
  let inspectText = $state<string | null>(null);
  let inspectError = $state<string | null>(null);
  let inspectLoading = $state(false);

  // Top tab.
  let top = $state<ContainerTopDto | null>(null);
  let topError = $state<string | null>(null);
  let topLoading = $state(false);

  // Reset cached per-container state when the selected container changes. Starts
  // empty so the effect also runs once on mount to seed renameValue from `name`.
  let lastId = $state("");
  $effect(() => {
    if (id !== lastId) {
      lastId = id;
      stats = null;
      statsError = null;
      inspectText = null;
      inspectError = null;
      top = null;
      topError = null;
      paused = false;
      renaming = false;
      renameValue = name;
      actionError = null;
    }
  });

  // Load inspect once per container: feeds the Inspect tab AND tells us whether
  // the container is currently paused (so the action button shows correctly).
  $effect(() => {
    if (engineReady) loadInspect(id);
  });

  async function loadInspect(cid: string) {
    inspectLoading = true;
    inspectError = null;
    try {
      const text = await containerInspect(cid);
      if (cid !== id) return;
      inspectText = text;
      try {
        const parsed = JSON.parse(text);
        if (parsed?.State?.Paused != null) paused = !!parsed.State.Paused;
      } catch {
        // Non-JSON debug fallback; leave paused as-is.
      }
    } catch (e) {
      if (cid === id) inspectError = String(e);
    } finally {
      if (cid === id) inspectLoading = false;
    }
  }

  // Stats polling: only while the Stats tab is open, the container is running,
  // and the engine is reachable. The interval is torn down in the cleanup.
  $effect(() => {
    if (activeTab !== "stats" || !running || !engineReady) {
      return;
    }
    const cid = id;
    let cancelled = false;
    const load = async () => {
      try {
        const s = await containerStats(cid);
        if (!cancelled && cid === id) {
          stats = s;
          statsError = null;
        }
      } catch (e) {
        if (!cancelled && cid === id) statsError = String(e);
      }
    };
    load();
    const interval = setInterval(load, 2000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  });

  // Lazily load the process table when the Top tab is opened.
  $effect(() => {
    if (
      activeTab === "top" &&
      top === null &&
      !topLoading &&
      running &&
      engineReady
    ) {
      loadTop(id);
    }
  });

  async function loadTop(cid: string) {
    topLoading = true;
    topError = null;
    try {
      const result = await containerTop(cid);
      if (cid === id) top = result;
    } catch (e) {
      if (cid === id) topError = String(e);
    } finally {
      if (cid === id) topLoading = false;
    }
  }

  async function togglePause() {
    busyAction = true;
    actionError = null;
    try {
      if (paused) {
        await containerUnpause(id);
        paused = false;
      } else {
        await containerPause(id);
        paused = true;
      }
      onChanged?.();
    } catch (e) {
      actionError = String(e);
    } finally {
      busyAction = false;
    }
  }

  function startRename() {
    renameValue = name;
    actionError = null;
    renaming = true;
  }

  async function commitRename() {
    const next = renameValue.trim();
    if (!next || next === name) {
      renaming = false;
      return;
    }
    busyAction = true;
    actionError = null;
    try {
      await containerRename(id, next);
      renaming = false;
      onChanged?.();
    } catch (e) {
      actionError = String(e);
    } finally {
      busyAction = false;
    }
  }

  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename();
    } else if (e.key === "Escape") {
      e.preventDefault();
      renaming = false;
    }
  }

  const TABS: { key: Tab; label: string; icon: typeof Activity }[] = [
    { key: "stats", label: "Stats", icon: Activity },
    { key: "inspect", label: "Inspect", icon: FileText },
    { key: "top", label: "Top", icon: List },
  ];

  const clampPct = (n: number) => Math.max(0, Math.min(100, n));
  const fmtPct = (n: number) =>
    Number.isFinite(n) ? `${n.toFixed(1)}%` : "0.0%";
</script>

<!-- Backdrop -->
<div
  class="fixed inset-0 z-40 bg-black/40"
  onclick={onClose}
  role="presentation"
></div>

<!-- Drawer -->
<aside
  class="fixed right-0 top-0 z-50 flex h-full w-[460px] flex-col border-l border-[#262b34] bg-[#171a21] text-[#e6e8eb] shadow-2xl"
  aria-label="Container details"
>
  <!-- Header -->
  <header
    class="flex items-center gap-2.5 border-b border-[#262b34] px-4 py-3"
  >
    <div class="min-w-0 flex-1">
      <div class="truncate text-sm font-semibold" title={name}>{name}</div>
      <div class="font-mono-app text-[11px] text-[#9aa3af]" title={id}>
        {shortId}
      </div>
    </div>
    <button
      class="flex cursor-pointer items-center justify-center rounded-md border border-[#262b34] p-1.5 text-[#9aa3af] transition-colors hover:bg-[#21262d] hover:text-[#e6e8eb]"
      onclick={onClose}
      aria-label="Close"
      title="Close"
    >
      <X size={16} aria-hidden="true" />
    </button>
  </header>

  <!-- Action row -->
  <div
    class="flex flex-wrap items-center gap-2 border-b border-[#262b34] px-4 py-2.5"
  >
    {#if renaming}
      <input
        class="font-mono-app min-w-0 flex-1 rounded-md border border-[#262b34] bg-[#0e1116] px-2 py-1 text-[13px] text-[#e6e8eb] outline-none focus:border-[#2f81f7]"
        bind:value={renameValue}
        onkeydown={onRenameKey}
        disabled={busyAction}
        placeholder="new-name"
        spellcheck="false"
        autocomplete="off"
      />
      <button
        class="cursor-pointer rounded-md border border-[#2f81f7]/60 bg-[#2f81f71a] px-2.5 py-1 text-[12px] text-[#2f81f7] transition-colors hover:not-disabled:bg-[#2f81f726] disabled:cursor-default disabled:opacity-40"
        onclick={commitRename}
        disabled={busyAction}
      >
        Save
      </button>
      <button
        class="cursor-pointer rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-1 text-[12px] text-[#9aa3af] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
        onclick={() => (renaming = false)}
        disabled={busyAction}
      >
        Cancel
      </button>
    {:else}
      <button
        class="inline-flex cursor-pointer items-center gap-1 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-1 text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
        onclick={togglePause}
        disabled={busyAction || !engineReady || (!running && !paused)}
        title={paused ? "Resume container" : "Pause container"}
      >
        {#if paused}
          <Play size={13} aria-hidden="true" /> Unpause
        {:else}
          <Pause size={13} aria-hidden="true" /> Pause
        {/if}
      </button>
      <button
        class="inline-flex cursor-pointer items-center gap-1 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-1 text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
        onclick={startRename}
        disabled={busyAction || !engineReady}
        title="Rename container"
      >
        <Pencil size={13} aria-hidden="true" /> Rename
      </button>
      {#if paused}
        <span
          class="ml-auto rounded-full border border-[#d2992280] bg-[#d299221a] px-2 py-0.5 text-[11px] text-[#d29922]"
          >paused</span
        >
      {/if}
    {/if}
  </div>

  {#if actionError}
    <div
      class="mx-4 mt-2.5 rounded-md border border-[#f8514980] bg-[#f851491a] px-3 py-2 text-[12px] text-[#f85149]"
    >
      {actionError}
    </div>
  {/if}

  <!-- Tabs -->
  <nav class="flex gap-1 border-b border-[#262b34] px-3 pt-2">
    {#each TABS as t (t.key)}
      {@const Icon = t.icon}
      <button
        class="inline-flex cursor-pointer items-center gap-1.5 rounded-t-md border-b-2 px-3 py-1.5 text-[13px] transition-colors {activeTab ===
        t.key
          ? 'border-[#2f81f7] text-[#e6e8eb]'
          : 'border-transparent text-[#9aa3af] hover:text-[#e6e8eb]'}"
        onclick={() => (activeTab = t.key)}
      >
        <Icon size={14} aria-hidden="true" />
        {t.label}
      </button>
    {/each}
  </nav>

  <!-- Body -->
  <div class="min-h-0 flex-1 overflow-auto">
    {#if activeTab === "stats"}
      <div class="p-4">
        {#if !running}
          <div class="py-6 text-center text-[13px] text-[#9aa3af]">
            Container is not running — no live stats.
          </div>
        {:else if statsError}
          <div
            class="rounded-md border border-[#f8514980] bg-[#f851491a] px-3 py-2 text-[12px] text-[#f85149]"
          >
            {statsError}
          </div>
        {:else if !stats}
          <div class="py-6 text-center text-[13px] text-[#9aa3af]">
            Loading stats…
          </div>
        {:else}
          <div class="flex flex-col gap-3">
            <!-- CPU -->
            <div class="rounded-md border border-[#262b34] bg-[#0e1116] p-3">
              <div class="flex items-baseline justify-between">
                <span class="text-[12px] text-[#9aa3af]">CPU</span>
                <span class="font-mono-app text-[13px] text-[#e6e8eb]"
                  >{fmtPct(stats.cpu_pct)}</span
                >
              </div>
              <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-[#21262d]">
                <div
                  class="h-full rounded-full bg-[#2f81f7] transition-all"
                  style="width: {clampPct(stats.cpu_pct)}%"
                ></div>
              </div>
            </div>

            <!-- Memory -->
            <div class="rounded-md border border-[#262b34] bg-[#0e1116] p-3">
              <div class="flex items-baseline justify-between">
                <span class="text-[12px] text-[#9aa3af]">Memory</span>
                <span class="font-mono-app text-[13px] text-[#e6e8eb]"
                  >{fmtPct(stats.mem_pct)}</span
                >
              </div>
              <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-[#21262d]">
                <div
                  class="h-full rounded-full bg-[#3fb950] transition-all"
                  style="width: {clampPct(stats.mem_pct)}%"
                ></div>
              </div>
              <div class="font-mono-app mt-1.5 text-[11px] text-[#9aa3af]">
                {humanBytes(stats.mem_usage)} / {humanBytes(stats.mem_limit)}
              </div>
            </div>

            <!-- Net / Block / PIDs -->
            <div class="grid grid-cols-2 gap-3">
              <div class="rounded-md border border-[#262b34] bg-[#0e1116] p-3">
                <div class="text-[12px] text-[#9aa3af]">Net I/O</div>
                <div class="font-mono-app mt-1 text-[13px] text-[#e6e8eb]">
                  ↓ {humanBytes(stats.net_rx)}
                </div>
                <div class="font-mono-app text-[13px] text-[#e6e8eb]">
                  ↑ {humanBytes(stats.net_tx)}
                </div>
              </div>
              <div class="rounded-md border border-[#262b34] bg-[#0e1116] p-3">
                <div class="text-[12px] text-[#9aa3af]">Block I/O</div>
                <div class="font-mono-app mt-1 text-[13px] text-[#e6e8eb]">
                  R {humanBytes(stats.blk_read)}
                </div>
                <div class="font-mono-app text-[13px] text-[#e6e8eb]">
                  W {humanBytes(stats.blk_write)}
                </div>
              </div>
            </div>
            <div class="rounded-md border border-[#262b34] bg-[#0e1116] p-3">
              <div class="flex items-baseline justify-between">
                <span class="text-[12px] text-[#9aa3af]">PIDs</span>
                <span class="font-mono-app text-[13px] text-[#e6e8eb]"
                  >{stats.pids}</span
                >
              </div>
            </div>
          </div>
        {/if}
      </div>
    {:else if activeTab === "inspect"}
      <div class="flex h-full flex-col p-4">
        {#if inspectLoading && inspectText === null}
          <div class="py-6 text-center text-[13px] text-[#9aa3af]">
            Loading inspect…
          </div>
        {:else if inspectError}
          <div
            class="rounded-md border border-[#f8514980] bg-[#f851491a] px-3 py-2 text-[12px] text-[#f85149]"
          >
            {inspectError}
          </div>
          <button
            class="mt-3 self-start cursor-pointer rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-1 text-[12px] text-[#e6e8eb] hover:bg-[#2b3138]"
            onclick={() => loadInspect(id)}
          >
            Retry
          </button>
        {:else if inspectText !== null}
          <pre
            class="font-mono-app m-0 flex-1 overflow-auto rounded-md border border-[#262b34] bg-[#0e1116] p-3 text-[11px] leading-relaxed text-[#c7ccd4] select-text whitespace-pre">{inspectText}</pre>
        {:else}
          <button
            class="self-start cursor-pointer rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-1 text-[12px] text-[#e6e8eb] hover:bg-[#2b3138]"
            onclick={() => loadInspect(id)}
          >
            Load inspect
          </button>
        {/if}
      </div>
    {:else if activeTab === "top"}
      <div class="p-4">
        {#if !running}
          <div class="py-6 text-center text-[13px] text-[#9aa3af]">
            Container is not running — no processes.
          </div>
        {:else if topLoading && top === null}
          <div class="py-6 text-center text-[13px] text-[#9aa3af]">
            Loading processes…
          </div>
        {:else if topError}
          <div
            class="rounded-md border border-[#f8514980] bg-[#f851491a] px-3 py-2 text-[12px] text-[#f85149]"
          >
            {topError}
          </div>
        {:else if top && top.processes.length > 0}
          <div class="overflow-x-auto">
            <table class="font-mono-app w-full border-collapse text-[11px]">
              <thead>
                <tr>
                  {#each top.titles as title, i (i)}
                    <th
                      class="whitespace-nowrap border-b border-[#262b34] px-2 py-1.5 text-left font-medium text-[#9aa3af]"
                      >{title}</th
                    >
                  {/each}
                </tr>
              </thead>
              <tbody>
                {#each top.processes as row, ri (ri)}
                  <tr class="hover:bg-[#1b1f27]">
                    {#each row as cell, ci (ci)}
                      <td
                        class="whitespace-nowrap border-b border-[#1f242c] px-2 py-1.5 align-top text-[#c7ccd4]"
                        >{cell}</td
                      >
                    {/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else}
          <div class="py-6 text-center text-[13px] text-[#9aa3af]">
            No processes reported.
          </div>
        {/if}
      </div>
    {/if}
  </div>
</aside>

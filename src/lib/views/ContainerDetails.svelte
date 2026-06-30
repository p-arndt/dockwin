<script lang="ts">
  // Container detail surface in the crafted v2 language. Renders either as a
  // right-side drawer (full=false) or a full-width two-column page (full=true).
  // Tabs: Overview (live stat cards polled every 2s + parsed details), Inspect
  // (raw JSON), Top (in-container processes). Engine calls go through
  // containersApi; the parent polls every 3s, so the list stays fresh without an
  // onChanged callback. Svelte 5 runes.
  import X from "@lucide/svelte/icons/x";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Minimize2 from "@lucide/svelte/icons/minimize-2";
  import Box from "@lucide/svelte/icons/box";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Cpu from "@lucide/svelte/icons/cpu";
  import MemoryStick from "@lucide/svelte/icons/memory-stick";
  import Activity from "@lucide/svelte/icons/activity";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Network from "@lucide/svelte/icons/network";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ArrowDownToLine from "@lucide/svelte/icons/arrow-down-to-line";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import type { NormalizedContainer, NormalizedPort } from "../types";
  import { openExternal } from "../api/external";
  import {
    containerLogsStart,
    containerLogsStop,
    onLogLine,
    onLogEnd,
  } from "../api";
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
  } from "../api/containers";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";

  interface Props {
    container: NormalizedContainer;
    full: boolean;
    onClose: () => void;
    onToggleFull: () => void;
  }

  let { container, full, onClose, onToggleFull }: Props = $props();

  // Convenience reads off the live container object.
  const id = $derived(container.id);
  const shortId = $derived(container.shortId);
  const image = $derived(container.image);
  const status = $derived(container.status);
  const running = $derived(container.running);

  type Tab = "overview" | "logs" | "inspect" | "top";
  let activeTab = $state<Tab>("overview");

  // Locally-tracked name so a rename shows immediately (the parent's selected
  // snapshot only updates on its next poll).
  let localName = $state("");
  const name = $derived(localName || container.name);

  // A container can be running-but-paused (refined from the inspect payload).
  let paused = $state(false);

  // Action row (pause/unpause/rename).
  let actionError = $state<string | null>(null);
  let busyAction = $state(false);
  let renaming = $state(false);
  let renameValue = $state("");

  // Live stats (Overview).
  let stats = $state<ContainerStatsDto | null>(null);
  let statsError = $state<string | null>(null);
  // Small CPU history buffer feeding the one focused chart on this screen.
  let cpuHistory = $state<number[]>([]);

  // Inspect (raw JSON + parsed detail fields).
  let inspectText = $state<string | null>(null);
  let inspectError = $state<string | null>(null);
  let inspectLoading = $state(false);

  // Top (process table).
  let top = $state<ContainerTopDto | null>(null);
  let topError = $state<string | null>(null);
  let topLoading = $state(false);

  // Logs (live stream). Chunks keep their stream tag so stderr renders distinctly.
  let logChunks = $state<{ stream: string; message: string }[]>([]);
  let logStreaming = $state(false);
  let logEnded = $state(false);
  let logError = $state<string | null>(null);
  let logEl = $state<HTMLDivElement | null>(null);
  // Stick to the bottom while the user hasn't scrolled up to read history.
  let logFollow = $state(true);
  const LOG_CAP = 5000;

  // Show-advanced disclosure in the details list.
  let showAdv = $state(false);

  // Copy-to-clipboard feedback.
  let copied = $state(false);

  // Reset cached per-container state when the selection changes. Starts empty so
  // the effect also runs once on mount to seed localName from the container.
  let lastId = $state("");
  $effect(() => {
    if (id !== lastId) {
      lastId = id;
      stats = null;
      statsError = null;
      cpuHistory = [];
      inspectText = null;
      inspectError = null;
      top = null;
      topError = null;
      logChunks = [];
      logStreaming = false;
      logEnded = false;
      logError = null;
      logFollow = true;
      paused = false;
      renaming = false;
      renameValue = container.name;
      localName = container.name;
      actionError = null;
      showAdv = false;
      copied = false;
    }
  });

  // Load inspect once per container: feeds the Inspect tab, the parsed Details
  // block, AND the paused flag.
  $effect(() => {
    void id;
    loadInspect(id);
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

  // Parsed inspect fields for the Details block (defensive — fields vary).
  interface ParsedInfo {
    created: string | null;
    command: string | null;
    entrypoint: string | null;
    workingDir: string | null;
    restart: string | null;
    networks: string[];
    mounts: { name: string; dest: string }[];
    envCount: number;
  }
  const info = $derived.by<ParsedInfo | null>(() => {
    if (!inspectText) return null;
    let p: any;
    try {
      p = JSON.parse(inspectText);
    } catch {
      return null;
    }
    const created = typeof p?.Created === "string" ? p.Created : null;
    const cmd = Array.isArray(p?.Config?.Cmd) ? p.Config.Cmd.join(" ") : null;
    const pathCmd =
      typeof p?.Path === "string"
        ? [p.Path, ...(Array.isArray(p?.Args) ? p.Args : [])].join(" ")
        : null;
    const ep = Array.isArray(p?.Config?.Entrypoint)
      ? p.Config.Entrypoint.join(" ")
      : typeof p?.Config?.Entrypoint === "string"
        ? p.Config.Entrypoint
        : null;
    const networks =
      p?.NetworkSettings?.Networks &&
      typeof p.NetworkSettings.Networks === "object"
        ? Object.keys(p.NetworkSettings.Networks)
        : [];
    const mounts = Array.isArray(p?.Mounts)
      ? p.Mounts.map((m: any) => ({
          name: String(m?.Name ?? m?.Source ?? "volume"),
          dest: String(m?.Destination ?? ""),
        }))
      : [];
    const restart =
      typeof p?.HostConfig?.RestartPolicy?.Name === "string" &&
      p.HostConfig.RestartPolicy.Name
        ? p.HostConfig.RestartPolicy.Name
        : null;
    return {
      created,
      command: pathCmd || cmd,
      entrypoint: ep,
      workingDir:
        typeof p?.Config?.WorkingDir === "string" && p.Config.WorkingDir
          ? p.Config.WorkingDir
          : null,
      restart,
      networks,
      mounts,
      envCount: Array.isArray(p?.Config?.Env) ? p.Config.Env.length : 0,
    };
  });

  // Stats polling: only while Overview is open and the container is running.
  $effect(() => {
    if (activeTab !== "overview" || !running) return;
    const cid = id;
    let cancelled = false;
    const load = async () => {
      try {
        const s = await containerStats(cid);
        if (!cancelled && cid === id) {
          stats = s;
          statsError = null;
          const v = Number.isFinite(s.cpu_pct) ? s.cpu_pct : 0;
          cpuHistory = [...cpuHistory, v].slice(-32);
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
    if (activeTab === "top" && top === null && !topLoading && running) {
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

  // Live log streaming: active only while the Logs tab is open. Seeds with the
  // last 500 lines from the backend, then follows new output. Tearing down
  // (tab switch, container change, panel close) aborts the backend stream.
  $effect(() => {
    if (activeTab !== "logs") return;
    const cid = id;
    let cancelled = false;
    let unlistenLine: (() => void) | undefined;
    let unlistenEnd: (() => void) | undefined;

    // Fresh start each time the tab opens (the backend re-seeds the tail).
    logChunks = [];
    logEnded = false;
    logError = null;
    logStreaming = true;
    logFollow = true;

    (async () => {
      unlistenLine = await onLogLine((l) => {
        if (cancelled || l.id !== cid) return;
        const next = [...logChunks, { stream: l.stream, message: l.message }];
        logChunks = next.length > LOG_CAP ? next.slice(-LOG_CAP) : next;
      });
      unlistenEnd = await onLogEnd((e) => {
        if (cancelled || e.id !== cid) return;
        logStreaming = false;
        logEnded = true;
        if (e.error) logError = e.error;
      });
      try {
        await containerLogsStart(cid, 500);
      } catch (e) {
        if (!cancelled) {
          logStreaming = false;
          logError = String(e);
        }
      }
    })();

    return () => {
      cancelled = true;
      unlistenLine?.();
      unlistenEnd?.();
      void containerLogsStop();
    };
  });

  // Keep the log viewport pinned to the bottom while following.
  $effect(() => {
    void logChunks.length;
    if (logFollow && logEl) {
      logEl.scrollTop = logEl.scrollHeight;
    }
  });

  // Drop out of follow-mode when the user scrolls up; re-arm at the bottom.
  function onLogScroll() {
    if (!logEl) return;
    const atBottom =
      logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 24;
    logFollow = atBottom;
  }

  function clearLogs() {
    logChunks = [];
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
      localName = next;
      renaming = false;
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

  async function copyId() {
    try {
      await navigator.clipboard?.writeText(id);
      copied = true;
      setTimeout(() => (copied = false), 1200);
    } catch {
      // Clipboard unavailable — ignore.
    }
  }

  function openPort(url: string) {
    openExternal(url);
  }

  function portLabel(p: NormalizedPort): string {
    return p.container != null
      ? `:${p.host}→${p.container}`
      : `:${p.host}`;
  }
  function portTitle(p: NormalizedPort): string {
    if (p.url) return `Open ${p.url} (forwarded to Windows localhost)`;
    if (p.wildcard) return `${p.host}:${p.container}/${p.proto}`;
    return `Bound to ${p.ip} — NOT forwarded to Windows localhost`;
  }

  const clampPct = (n: number) => Math.max(0, Math.min(100, n));
  const fmtPct = (n: number) => (Number.isFinite(n) ? `${n.toFixed(1)}%` : "0.0%");
  const cpuRounded = $derived(stats ? Math.round(clampPct(stats.cpu_pct)) : 0);
  const memRounded = $derived(stats ? Math.round(clampPct(stats.mem_pct)) : 0);

  function fmtCreated(iso: string | null): string | null {
    if (!iso) return null;
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleString();
  }

  // Build the focused-chart spark paths (the one accent chart on this screen).
  const SPARK_W = 120;
  const SPARK_H = 34;
  const spark = $derived.by(() => {
    const vals = cpuHistory;
    if (vals.length < 2) return { line: "", fill: "" };
    const min = Math.min(...vals);
    const max = Math.max(...vals);
    const span = max - min || 1;
    const stepX = SPARK_W / (vals.length - 1);
    const coords = vals.map((v, i) => {
      const x = i * stepX;
      const y = SPARK_H - ((v - min) / span) * (SPARK_H - 4) - 2;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    const line = "M" + coords.join(" L");
    return { line, fill: `${line} L${SPARK_W},${SPARK_H} L0,${SPARK_H} Z` };
  });

  const TABS: { key: Tab; label: string }[] = [
    { key: "overview", label: "Overview" },
    { key: "logs", label: "Logs" },
    { key: "inspect", label: "Inspect" },
    { key: "top", label: "Top" },
  ];
</script>

<aside class="flex min-w-0 flex-1 min-h-0 flex-col overflow-auto bg-card" aria-label="Container details">
  <!-- ===== header ===== -->
  <div class="border-b border-border px-5 pt-4 pb-4">
    <div class="flex items-center gap-3">
      <span class="grid size-[38px] shrink-0 place-items-center rounded-[10px] border border-border bg-muted text-muted-foreground"><Box class="size-[19px]" aria-hidden="true" /></span>
      <div class="min-w-0">
        <div class="text-base font-[680] tracking-[-0.3px] leading-[1.15]" title={name}>{name}</div>
        <div class="mt-[3px] flex flex-wrap items-center gap-2 text-[11.5px] text-muted-foreground">
          {#if running}
            <span class="inline-flex items-center gap-[5px] font-semibold text-chart-2"><span class="size-[6px] rounded-full bg-chart-2"></span>{paused ? "Paused" : "Running"}</span>
          {:else}
            <span class="inline-flex items-center gap-[5px] font-semibold text-muted-foreground"><span class="size-[6px] rounded-full bg-chart-5"></span>Stopped</span>
          {/if}
          {#if status}<span>·</span><span>{status}</span>{/if}
          <span class="font-mono text-muted-foreground/70" title={id}>{shortId}</span>
        </div>
      </div>
      <div class="ml-auto flex gap-[7px]">
        <Button
          variant="outline"
          size="icon-sm"
          type="button"
          onclick={onToggleFull}
          title={full ? "Narrow panel" : "Widen panel"}
          aria-label={full ? "Narrow panel" : "Widen panel"}
        >
          {#if full}<Minimize2 aria-hidden="true" />{:else}<Maximize2 aria-hidden="true" />{/if}
        </Button>
        <Button variant="outline" size="icon-sm" type="button" onclick={onClose} title="Close" aria-label="Close">
          <X aria-hidden="true" />
        </Button>
      </div>
    </div>

    <!-- action row (pause/unpause + rename) -->
    {#if renaming}
      <div class="mt-4 flex flex-wrap gap-[7px]">
        <Input
          class="flex-1 font-mono"
          bind:value={renameValue}
          onkeydown={onRenameKey}
          disabled={busyAction}
          placeholder="new-name"
          spellcheck="false"
          autocomplete="off"
        />
        <Button variant="outline" size="sm" class="flex-1" type="button" onclick={commitRename} disabled={busyAction}>
          Save
        </Button>
        <Button variant="outline" size="sm" class="flex-1" type="button" onclick={() => (renaming = false)} disabled={busyAction}>
          Cancel
        </Button>
      </div>
    {:else}
      <div class="mt-4 flex flex-wrap gap-[7px]">
        <Button
          variant="outline"
          size="sm"
          class="flex-1"
          type="button"
          onclick={togglePause}
          disabled={busyAction || (!running && !paused)}
          title={paused ? "Resume container" : "Pause container"}
        >
          {#if paused}<Play aria-hidden="true" />Unpause{:else}<Pause aria-hidden="true" />Pause{/if}
        </Button>
        <Button variant="outline" size="sm" class="flex-1" type="button" onclick={startRename} disabled={busyAction} title="Rename container">
          <Pencil aria-hidden="true" />Rename
        </Button>
      </div>
    {/if}
  </div>

  {#if actionError}
    <Alert.Root variant="destructive" class="mx-5 mt-3">
      <TriangleAlert aria-hidden="true" />
      <Alert.Description>{actionError}</Alert.Description>
    </Alert.Root>
  {/if}

  <!-- ===== tabs ===== -->
  <Tabs.Root bind:value={activeTab} class="px-5 pt-1">
    <Tabs.List>
      {#each TABS as t (t.key)}
        <Tabs.Trigger value={t.key} class="data-active:bg-card dark:data-active:bg-foreground/10">{t.label}</Tabs.Trigger>
      {/each}
    </Tabs.List>
  </Tabs.Root>

  <!-- ===== body ===== -->
  <div class="flex flex-col gap-4 px-5 pt-4 pb-6" style:max-width={full ? "1120px" : undefined} style:width={full ? "100%" : undefined} style:margin={full ? "0 auto" : undefined}>
    {#if activeTab === "overview"}
      <div class="ov" class:ov-full={full && running}>
        <!-- live stat cards — only while running; a stopped container has no
             stats, so we drop the section entirely and let Details be the content -->
        {#if running}
        <div class="ov-stats">
          {#if statsError}
            <Alert.Root variant="destructive">
              <TriangleAlert aria-hidden="true" />
              <Alert.Description>{statsError}</Alert.Description>
            </Alert.Root>
          {:else if !stats}
            <div class="rounded-[10px] border border-border bg-muted/20 px-[18px] py-6 text-center text-[13px] text-muted-foreground">Loading stats…</div>
          {:else}
            <div class="grid grid-cols-2 gap-[12px]">
              <!-- CPU — the one focused chart on this screen -->
              <div class="relative overflow-hidden rounded-[9px] border border-border bg-card px-[14px] py-[13px]">
                <div class="flex items-center gap-[6px] text-[11px] font-medium text-muted-foreground"><Cpu class="size-[13px] text-muted-foreground/70" aria-hidden="true" />CPU</div>
                <div class="mt-2 text-[21px] font-[680] tracking-[-0.5px] tabular-nums">{cpuRounded}<small class="ml-[2px] text-[12px] font-medium text-muted-foreground">%</small></div>
                <div class="mt-[2px] text-[11px] tabular-nums text-muted-foreground/70">{stats.pids} PIDs</div>
                {#if spark.line}
                  <svg class="absolute inset-x-0 bottom-0 h-[34px] opacity-90" viewBox="0 0 {SPARK_W} {SPARK_H}" preserveAspectRatio="none" aria-hidden="true">
                    <defs>
                      <linearGradient id="cpuspark" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" stop-color="var(--primary)" stop-opacity="0.35" />
                        <stop offset="1" stop-color="var(--primary)" stop-opacity="0" />
                      </linearGradient>
                    </defs>
                    <path d={spark.fill} fill="url(#cpuspark)" />
                    <path d={spark.line} fill="none" stroke="var(--primary)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
                  </svg>
                {/if}
              </div>

              <!-- Memory -->
              <div class="relative overflow-hidden rounded-[9px] border border-border bg-card px-[14px] py-[13px]">
                <div class="flex items-center gap-[6px] text-[11px] font-medium text-muted-foreground"><MemoryStick class="size-[13px] text-muted-foreground/70" aria-hidden="true" />Memory</div>
                <div class="mt-2 text-[21px] font-[680] tracking-[-0.5px] tabular-nums">{memRounded}<small class="ml-[2px] text-[12px] font-medium text-muted-foreground">%</small></div>
                <div class="mt-[2px] text-[11px] tabular-nums text-muted-foreground/70">{humanBytes(stats.mem_usage)} / {humanBytes(stats.mem_limit)}</div>
                <div class="relative mt-[9px] h-1 overflow-hidden rounded-[3px] bg-muted"><i class="absolute inset-y-0 left-0 rounded-[3px] bg-muted-foreground" style="width:{clampPct(stats.mem_pct)}%"></i></div>
              </div>

              <!-- Network I/O -->
              <div class="relative overflow-hidden rounded-[9px] border border-border bg-card px-[14px] py-[13px]">
                <div class="flex items-center gap-[6px] text-[11px] font-medium text-muted-foreground"><Activity class="size-[13px] text-muted-foreground/70" aria-hidden="true" />Network I/O</div>
                <div class="mt-2 flex flex-col gap-[5px]">
                  <span class="flex items-center gap-[7px] text-[12.5px] tabular-nums text-muted-foreground"><ArrowDown class="size-[12px] text-muted-foreground/70" aria-hidden="true" /><b class="font-[650] text-foreground">{humanBytes(stats.net_rx)}</b> rx</span>
                  <span class="flex items-center gap-[7px] text-[12.5px] tabular-nums text-muted-foreground"><ArrowUp class="size-[12px] text-muted-foreground/70" aria-hidden="true" /><b class="font-[650] text-foreground">{humanBytes(stats.net_tx)}</b> tx</span>
                </div>
              </div>

              <!-- Block I/O -->
              <div class="relative overflow-hidden rounded-[9px] border border-border bg-card px-[14px] py-[13px]">
                <div class="flex items-center gap-[6px] text-[11px] font-medium text-muted-foreground"><HardDrive class="size-[13px] text-muted-foreground/70" aria-hidden="true" />Block I/O</div>
                <div class="mt-2 flex flex-col gap-[5px]">
                  <span class="flex items-center gap-[7px] text-[12.5px] tabular-nums text-muted-foreground">R <b class="font-[650] text-foreground">{humanBytes(stats.blk_read)}</b></span>
                  <span class="flex items-center gap-[7px] text-[12.5px] tabular-nums text-muted-foreground">W <b class="font-[650] text-foreground">{humanBytes(stats.blk_write)}</b></span>
                </div>
              </div>
            </div>
          {/if}
        </div>
        {/if}

        <!-- parsed details -->
        <div class="ov-details" class:ov-solo={full && !running}>
          <div class="flex flex-col">
            <div class="pt-1 pb-[9px] text-[12px] font-semibold text-muted-foreground">Details</div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Image</span>
              <span class="text-left text-[12.5px] break-words text-foreground">
                <span class="font-mono">{image || "—"}</span>
              </span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Container ID</span>
              <span class="text-left text-[12.5px] break-words text-foreground">
                <Button variant="ghost" size="icon-xs" class="text-muted-foreground w-auto gap-1 px-1 font-mono" type="button" onclick={copyId} title="Copy full ID">
                  {id.slice(0, 16)}
                  {#if copied}<Check aria-hidden="true" />{:else}<Copy aria-hidden="true" />{/if}
                </Button>
              </span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Status</span>
              <span class="text-left text-[12.5px] break-words text-foreground">{status || "—"}</span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Created</span>
              <span class="text-left text-[12.5px] break-words text-foreground tabular-nums">{fmtCreated(info?.created ?? null) ?? "—"}</span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Command</span>
              <span class="text-left break-words text-foreground font-mono text-[11.5px]">{info?.command || "—"}</span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Ports</span>
              <span class="text-left text-[12.5px] break-words text-foreground">
                {#if container.ports.length === 0}
                  <span class="text-muted-foreground/70">—</span>
                {:else}
                  <span class="flex flex-wrap justify-start gap-[5px]">
                    {#each container.ports as p, i (i)}
                      {#if p.url}
                        <Button variant="outline" size="xs" class="h-6 gap-1 px-2 font-mono text-[11px]" type="button" title={portTitle(p)} onclick={() => openPort(p.url!)}>
                          {portLabel(p)}<ExternalLink aria-hidden="true" />
                        </Button>
                      {:else}
                        <Badge variant="outline" class="h-6 px-2 font-mono text-[11px] font-normal" title={portTitle(p)}>{portLabel(p)}</Badge>
                      {/if}
                    {/each}
                  </span>
                {/if}
              </span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Networks</span>
              <span class="text-left text-[12.5px] break-words text-foreground">
                {#if info && info.networks.length}
                  <span class="flex flex-wrap justify-start gap-[5px]">
                    {#each info.networks as n (n)}
                      <span class="inline-flex items-center gap-[5px] rounded-[5px] border border-border bg-muted px-[7px] py-[2px] font-mono text-[11px] text-muted-foreground"><Network class="size-[11px] text-muted-foreground/70" aria-hidden="true" />{n}</span>
                    {/each}
                  </span>
                {:else}
                  <span class="text-muted-foreground/70">—</span>
                {/if}
              </span>
            </div>
            <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
              <span class="text-[12.5px] text-muted-foreground">Volumes</span>
              <span class="text-left text-[12.5px] break-words text-foreground">
                {#if info && info.mounts.length}
                  <span class="flex flex-wrap justify-start gap-[5px]">
                    {#each info.mounts as m, i (i)}
                      <span class="inline-flex items-center gap-[5px] rounded-[5px] border border-border bg-muted px-[7px] py-[2px] font-mono text-[11px] text-muted-foreground" title={m.dest}>{m.name}</span>
                    {/each}
                  </span>
                {:else}
                  <span class="text-muted-foreground/70">—</span>
                {/if}
              </span>
            </div>
            {#if container.composeProject}
              <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
                <span class="text-[12.5px] text-muted-foreground">Compose</span>
                <span class="text-left break-words text-foreground font-mono text-[11.5px]">{container.composeProject}</span>
              </div>
            {/if}

            {#if showAdv && info}
              <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
                <span class="text-[12.5px] text-muted-foreground">Entrypoint</span>
                <span class="text-left break-words text-foreground font-mono text-[11.5px]">{info.entrypoint || "—"}</span>
              </div>
              <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
                <span class="text-[12.5px] text-muted-foreground">Working dir</span>
                <span class="text-left break-words text-foreground font-mono text-[11.5px]">{info.workingDir || "—"}</span>
              </div>
              <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
                <span class="text-[12.5px] text-muted-foreground">Restart policy</span>
                <span class="text-left text-[12.5px] break-words text-foreground">{info.restart || "no"}</span>
              </div>
              <div class="grid grid-cols-[120px_1fr] items-start gap-[10px] border-t border-border py-2">
                <span class="text-[12.5px] text-muted-foreground">Env vars</span>
                <span class="text-left text-[12.5px] break-words text-foreground tabular-nums">{info.envCount}</span>
              </div>
            {/if}
          </div>
          {#if info}
            <Button variant="ghost" size="sm" class="h-auto self-start gap-1 px-1.5 py-0.5 text-muted-foreground" type="button" onclick={() => (showAdv = !showAdv)}>
              <ChevronDown aria-hidden="true" style={showAdv ? "transform:rotate(180deg)" : undefined} />
              {showAdv ? "Hide advanced information" : "Show advanced information"}
            </Button>
          {/if}
        </div>
      </div>
    {:else if activeTab === "logs"}
      <!-- toolbar: stream status + clear -->
      <div class="flex items-center gap-2">
        {#if logStreaming}
          <span class="inline-flex items-center gap-[5px] text-[11.5px] font-semibold text-chart-2"><span class="size-[6px] animate-pulse rounded-full bg-chart-2"></span>Streaming</span>
        {:else if logEnded}
          <span class="inline-flex items-center gap-[5px] text-[11.5px] font-semibold text-muted-foreground"><span class="size-[6px] rounded-full bg-chart-5"></span>Stream ended</span>
        {/if}
        <span class="text-[11.5px] tabular-nums text-muted-foreground/70">{logChunks.length} line{logChunks.length === 1 ? "" : "s"}</span>
        <div class="ml-auto flex gap-[7px]">
          {#if !logFollow}
            <Button variant="outline" size="xs" type="button" onclick={() => { logFollow = true; if (logEl) logEl.scrollTop = logEl.scrollHeight; }} title="Jump to latest and resume following">
              <ArrowDownToLine aria-hidden="true" />Follow
            </Button>
          {/if}
          <Button variant="outline" size="xs" type="button" onclick={clearLogs} disabled={logChunks.length === 0} title="Clear the log view">
            <Trash2 aria-hidden="true" />Clear
          </Button>
        </div>
      </div>

      {#if logError}
        <Alert.Root variant="destructive">
          <TriangleAlert aria-hidden="true" />
          <Alert.Description>{logError}</Alert.Description>
        </Alert.Root>
      {/if}

      <div
        bind:this={logEl}
        onscroll={onLogScroll}
        class="flex-1 min-h-[280px] overflow-auto bg-card border border-border rounded-[9px] p-[12px] text-[11px] leading-[1.55] select-text font-mono whitespace-pre-wrap break-all"
        style:max-height={full ? "calc(100vh - 320px)" : "calc(100vh - 360px)"}
      >
        {#if logChunks.length > 0}
          {#each logChunks as c, i (i)}<span class={c.stream === "stderr" ? "text-chart-5" : "text-muted-foreground"}>{c.message}</span>{/each}
        {:else if logStreaming}
          <div class="grid h-full place-items-center text-[12.5px] text-muted-foreground">Waiting for output…</div>
        {:else}
          <div class="flex h-full flex-col items-center justify-center gap-2 text-[12.5px] text-muted-foreground">
            <ScrollText class="size-5 text-muted-foreground/60" aria-hidden="true" />
            This container hasn't logged anything.
          </div>
        {/if}
      </div>
    {:else if activeTab === "inspect"}
      {#if inspectLoading && inspectText === null}
        <div class="rounded-[10px] border border-border bg-muted/20 px-[18px] py-6 text-center text-[13px] text-muted-foreground">Loading inspect…</div>
      {:else if inspectError}
        <Alert.Root variant="destructive">
          <TriangleAlert aria-hidden="true" />
          <Alert.Description>{inspectError}</Alert.Description>
        </Alert.Root>
        <Button variant="outline" size="sm" type="button" style="align-self:flex-start" onclick={() => loadInspect(id)}>
          Retry
        </Button>
      {:else if inspectText !== null}
        <pre
          class="m-0 flex-1 overflow-auto bg-card border border-border rounded-[9px] p-[12px] text-[11px] leading-[1.55] text-muted-foreground whitespace-pre select-text font-mono"
        >{inspectText}</pre>
      {:else}
        <Button variant="outline" size="sm" type="button" style="align-self:flex-start" onclick={() => loadInspect(id)}>
          Load inspect
        </Button>
      {/if}
    {:else if activeTab === "top"}
      {#if !running}
        <div class="rounded-[10px] border border-border bg-muted/20 px-[18px] py-6 text-center text-[13px] text-muted-foreground">Container is not running — no processes.</div>
      {:else if topLoading && top === null}
        <div class="rounded-[10px] border border-border bg-muted/20 px-[18px] py-6 text-center text-[13px] text-muted-foreground">Loading processes…</div>
      {:else if topError}
        <Alert.Root variant="destructive">
          <TriangleAlert aria-hidden="true" />
          <Alert.Description>{topError}</Alert.Description>
        </Alert.Root>
      {:else if top && top.processes.length > 0}
        <div class="rounded-xl border border-border bg-card shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] overflow-hidden">
          <Table.Root class="table-fixed">
            <Table.Header>
              <Table.Row class="hover:bg-transparent">
                {#each top.titles as title, i (i)}
                  <Table.Head class="h-9 text-[10.5px] font-semibold uppercase tracking-wider">{title}</Table.Head>
                {/each}
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each top.processes as row, ri (ri)}
                <Table.Row class="hover:bg-transparent">
                  {#each row as cell, ci (ci)}
                    <Table.Cell class="truncate font-mono text-[11px]">{cell}</Table.Cell>
                  {/each}
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        </div>
      {:else}
        <div class="rounded-[10px] border border-border bg-muted/20 px-[18px] py-6 text-center text-[13px] text-muted-foreground">No processes reported.</div>
      {/if}
    {/if}
  </div>
</aside>

<style>
  /* Local-only layout glue — colours/surfaces all come from app.css tokens. */
  .ov {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  /* full page: stats + details side by side */
  .ov-full {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 24px;
    align-items: start;
  }
  .ov-stats,
  .ov-details {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  /* Expanded view of a stopped container: no stats column, so keep the lone
     Details block at a comfortable reading width instead of stretching it
     across the full page (which left a broken-looking half-empty grid). */
  .ov-solo {
    max-width: 560px;
    margin-inline: auto;
    width: 100%;
  }
</style>

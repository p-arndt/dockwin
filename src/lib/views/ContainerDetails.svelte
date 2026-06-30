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
  import type { NormalizedContainer, NormalizedPort } from "../types";
  import { openExternal } from "../api/external";
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
  import { Button } from "$lib/components/ui/button/index.js";
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

  type Tab = "overview" | "inspect" | "top";
  let activeTab = $state<Tab>("overview");

  // Locally-tracked name so a rename shows immediately (the parent's selected
  // snapshot only updates on its next poll).
  let localName = $state("");
  const name = $derived(localName || container.name);

  // Heuristic: an "official" image has no registry/namespace prefix (e.g. nginx,
  // postgres) — surfaces the small accent badge. Anything user/registry-scoped
  // ("me/app", "ghcr.io/...") is not.
  const isOfficial = $derived.by(() => {
    if (!image) return false;
    return !image.split(":")[0].includes("/");
  });

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
    { key: "inspect", label: "Inspect" },
    { key: "top", label: "Top" },
  ];
</script>

<aside class="detail" class:full aria-label="Container details">
  <!-- ===== header ===== -->
  <div class="dt-head">
    <div class="dt-top">
      <span class="dt-av"><Box aria-hidden="true" /></span>
      <div style="min-width:0">
        <div class="dt-name" title={name}>{name}</div>
        <div class="dt-sub">
          {#if running}
            <span class="run"><span class="d"></span>{paused ? "Paused" : "Running"}</span>
          {:else}
            <span class="off"><span class="d"></span>Stopped</span>
          {/if}
          {#if status}<span>·</span><span>{status}</span>{/if}
          <span class="mono" title={id}>{shortId}</span>
        </div>
      </div>
      <div class="dt-head-acts">
        <button
          class="dt-x"
          type="button"
          onclick={onToggleFull}
          title={full ? "Collapse to drawer" : "Expand to full page"}
          aria-label={full ? "Collapse to drawer" : "Expand to full page"}
        >
          {#if full}<Minimize2 aria-hidden="true" />{:else}<Maximize2 aria-hidden="true" />{/if}
        </button>
        <button class="dt-x" type="button" onclick={onClose} title="Close" aria-label="Close">
          <X aria-hidden="true" />
        </button>
      </div>
    </div>

    <!-- action row (pause/unpause + rename) -->
    {#if renaming}
      <div class="dt-acts">
        <input
          class="rename-input mono"
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
      <div class="dt-acts">
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
    <div class="banner err" style="margin:12px 20px 0">{actionError}</div>
  {/if}

  <!-- ===== tabs ===== -->
  <Tabs.Root bind:value={activeTab}>
    <Tabs.List variant="line" class="mx-5">
      {#each TABS as t (t.key)}
        <Tabs.Trigger value={t.key} class="after:bg-[var(--lime)]">{t.label}</Tabs.Trigger>
      {/each}
    </Tabs.List>
  </Tabs.Root>

  <!-- ===== body ===== -->
  <div class="dt-body" style:max-width={full ? "1120px" : undefined} style:width={full ? "100%" : undefined} style:margin={full ? "0 auto" : undefined}>
    {#if activeTab === "overview"}
      <div class="ov" class:ov-full={full}>
        <!-- live stat cards -->
        <div class="ov-stats">
          {#if !running}
            <div class="empty">Container is not running — no live stats.</div>
          {:else if statsError}
            <div class="banner err">{statsError}</div>
          {:else if !stats}
            <div class="empty">Loading stats…</div>
          {:else}
            <div class="statgrid">
              <!-- CPU — the one focused chart on this screen -->
              <div class="stat focus">
                <div class="k"><Cpu aria-hidden="true" />CPU</div>
                <div class="big num">{cpuRounded}<small>%</small></div>
                <div class="sub2">{stats.pids} PIDs</div>
                {#if spark.line}
                  <svg class="spark" viewBox="0 0 {SPARK_W} {SPARK_H}" preserveAspectRatio="none" aria-hidden="true">
                    <defs>
                      <linearGradient id="cpuspark" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" stop-color="var(--lime)" stop-opacity="0.35" />
                        <stop offset="1" stop-color="var(--lime)" stop-opacity="0" />
                      </linearGradient>
                    </defs>
                    <path d={spark.fill} fill="url(#cpuspark)" />
                    <path d={spark.line} fill="none" stroke="var(--lime)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
                  </svg>
                {/if}
              </div>

              <!-- Memory -->
              <div class="stat">
                <div class="k"><MemoryStick aria-hidden="true" />Memory</div>
                <div class="big num">{memRounded}<small>%</small></div>
                <div class="sub2">{humanBytes(stats.mem_usage)} / {humanBytes(stats.mem_limit)}</div>
                <div class="mbar"><i style="width:{clampPct(stats.mem_pct)}%"></i></div>
              </div>

              <!-- Network I/O -->
              <div class="stat">
                <div class="k"><Activity aria-hidden="true" />Network I/O</div>
                <div class="pair">
                  <span><ArrowDown aria-hidden="true" /><b>{humanBytes(stats.net_rx)}</b> rx</span>
                  <span><ArrowUp aria-hidden="true" /><b>{humanBytes(stats.net_tx)}</b> tx</span>
                </div>
              </div>

              <!-- Block I/O -->
              <div class="stat">
                <div class="k"><HardDrive aria-hidden="true" />Block I/O</div>
                <div class="pair">
                  <span>R <b>{humanBytes(stats.blk_read)}</b></span>
                  <span>W <b>{humanBytes(stats.blk_write)}</b></span>
                </div>
              </div>
            </div>
          {/if}
        </div>

        <!-- parsed details -->
        <div class="ov-details">
          <div class="kv">
            <div class="sec">Details</div>
            <div class="r">
              <span class="k">Image</span>
              <span class="v">
                <span class="mono">{image || "—"}</span>
                {#if isOfficial}
                  <span class="official"><Check aria-hidden="true" />Official</span>
                {/if}
              </span>
            </div>
            <div class="r">
              <span class="k">Container ID</span>
              <span class="v">
                <button class="copy mono" type="button" onclick={copyId} title="Copy full ID">
                  {id.slice(0, 16)}
                  {#if copied}<Check aria-hidden="true" />{:else}<Copy aria-hidden="true" />{/if}
                </button>
              </span>
            </div>
            <div class="r">
              <span class="k">Status</span>
              <span class="v">{status || "—"}</span>
            </div>
            <div class="r">
              <span class="k">Created</span>
              <span class="v num">{fmtCreated(info?.created ?? null) ?? "—"}</span>
            </div>
            <div class="r">
              <span class="k">Command</span>
              <span class="v mono">{info?.command || "—"}</span>
            </div>
            <div class="r">
              <span class="k">Ports</span>
              <span class="v">
                {#if container.ports.length === 0}
                  <span class="muted">—</span>
                {:else}
                  <span class="chips">
                    {#each container.ports as p, i (i)}
                      {#if p.url}
                        <button class="port" type="button" style="cursor:pointer" title={portTitle(p)} onclick={() => openPort(p.url!)}>
                          {portLabel(p)}<ExternalLink aria-hidden="true" />
                        </button>
                      {:else}
                        <span class="port" title={portTitle(p)}>{portLabel(p)}</span>
                      {/if}
                    {/each}
                  </span>
                {/if}
              </span>
            </div>
            <div class="r">
              <span class="k">Networks</span>
              <span class="v">
                {#if info && info.networks.length}
                  <span class="chips">
                    {#each info.networks as n (n)}
                      <span class="net"><Network aria-hidden="true" />{n}</span>
                    {/each}
                  </span>
                {:else}
                  <span class="muted">—</span>
                {/if}
              </span>
            </div>
            <div class="r">
              <span class="k">Volumes</span>
              <span class="v">
                {#if info && info.mounts.length}
                  <span class="chips">
                    {#each info.mounts as m, i (i)}
                      <span class="net" title={m.dest}>{m.name}</span>
                    {/each}
                  </span>
                {:else}
                  <span class="muted">—</span>
                {/if}
              </span>
            </div>
            {#if container.composeProject}
              <div class="r">
                <span class="k">Compose</span>
                <span class="v mono">{container.composeProject}</span>
              </div>
            {/if}

            {#if showAdv && info}
              <div class="r">
                <span class="k">Entrypoint</span>
                <span class="v mono">{info.entrypoint || "—"}</span>
              </div>
              <div class="r">
                <span class="k">Working dir</span>
                <span class="v mono">{info.workingDir || "—"}</span>
              </div>
              <div class="r">
                <span class="k">Restart policy</span>
                <span class="v">{info.restart || "no"}</span>
              </div>
              <div class="r">
                <span class="k">Env vars</span>
                <span class="v num">{info.envCount}</span>
              </div>
            {/if}
          </div>
          {#if info}
            <button class="show-adv" type="button" onclick={() => (showAdv = !showAdv)}>
              <ChevronDown aria-hidden="true" style={showAdv ? "transform:rotate(180deg)" : undefined} />
              {showAdv ? "Hide advanced information" : "Show advanced information"}
            </button>
          {/if}
        </div>
      </div>
    {:else if activeTab === "inspect"}
      {#if inspectLoading && inspectText === null}
        <div class="empty">Loading inspect…</div>
      {:else if inspectError}
        <div class="banner err">{inspectError}</div>
        <Button variant="outline" size="sm" type="button" style="align-self:flex-start" onclick={() => loadInspect(id)}>
          Retry
        </Button>
      {:else if inspectText !== null}
        <pre class="inspect-pre mono">{inspectText}</pre>
      {:else}
        <Button variant="outline" size="sm" type="button" style="align-self:flex-start" onclick={() => loadInspect(id)}>
          Load inspect
        </Button>
      {/if}
    {:else if activeTab === "top"}
      {#if !running}
        <div class="empty">Container is not running — no processes.</div>
      {:else if topLoading && top === null}
        <div class="empty">Loading processes…</div>
      {:else if topError}
        <div class="banner err">{topError}</div>
      {:else if top && top.processes.length > 0}
        <div class="table">
          <div class="thead" style="--cols:repeat({top.titles.length},minmax(0,1fr))">
            {#each top.titles as title, i (i)}
              <span>{title}</span>
            {/each}
          </div>
          {#each top.processes as row, ri (ri)}
            <div class="toprow" style="--cols:repeat({top.titles.length},minmax(0,1fr))">
              {#each row as cell, ci (ci)}
                <span class="mono">{cell}</span>
              {/each}
            </div>
          {/each}
        </div>
      {:else}
        <div class="empty">No processes reported.</div>
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

  .rename-input {
    flex: 1;
    min-width: 0;
    background: var(--s2);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    padding: 7px 10px;
    color: var(--text);
    font-size: 12.5px;
    box-shadow: inset 0 1px 0 var(--hi);
    outline: none;
  }
  .rename-input:focus {
    border-color: var(--lime-line);
  }

  /* copy button styled as the kv copy affordance */
  .copy {
    background: transparent;
    border: 0;
    cursor: pointer;
    padding: 0;
  }

  .inspect-pre {
    margin: 0;
    flex: 1;
    overflow: auto;
    background: var(--s1);
    border: 1px solid var(--line);
    border-radius: var(--r);
    padding: 12px;
    font-size: 11px;
    line-height: 1.55;
    color: var(--text-2);
    white-space: pre;
    user-select: text;
    box-shadow: inset 0 1px 0 var(--hi);
  }

  /* process table rows (Top) reuse the table chrome from app.css */
  .toprow {
    display: grid;
    grid-template-columns: var(--cols);
    gap: 10px;
    align-items: center;
    padding: 8px 18px;
    border-bottom: 1px solid var(--line-soft);
    font-size: 11px;
    color: var(--text-2);
  }
  .toprow:last-child {
    border-bottom: 0;
  }
  .toprow span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>

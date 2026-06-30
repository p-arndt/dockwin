<script lang="ts">
  // Volumes view. Owns its own fetch lifecycle: loads on mount, when the engine
  // becomes running, and on an explicit refresh request from the parent. Talks
  // to the backend only through volumesApi.ts. Svelte 5 runes API.
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Search from "@lucide/svelte/icons/search";
  import Plus from "@lucide/svelte/icons/plus";
  import Eraser from "@lucide/svelte/icons/eraser";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Info from "@lucide/svelte/icons/info";
  import X from "@lucide/svelte/icons/x";
  import { errText } from "./api";
  import {
    volumeList,
    volumeCreate,
    volumeRemove,
    volumePrune,
    volumeInspect,
    type Volume,
  } from "./volumesApi";
  import type { EngineState } from "./types";

  interface Props {
    engineState?: EngineState;
    // Monotonically increasing token; bump it from the parent to force a reload.
    refreshKey?: number;
  }

  let { engineState = "unknown", refreshKey = 0 }: Props = $props();

  let volumes = $state<Volume[]>([]);
  let errorMsg = $state("");
  let loading = $state(false);

  // UI-only presentation state.
  let filter = $state("");
  let showCreate = $state(false);

  // Create form state.
  let newName = $state("");
  let newDriver = $state("");
  let creating = $state(false);

  // Prune state.
  let pruning = $state(false);
  let pruneMsg = $state("");

  // Per-volume action state.
  let busy = $state<Set<string>>(new Set());
  let forceRemove = $state(false);
  let inspectName = $state<string | null>(null);
  let inspectJson = $state("");
  let inspecting = $state(false);

  let loadGuard = false; // non-reactive guard against overlapping loads

  // Filtered, presentation-only view of the volumes.
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return volumes;
    return volumes.filter(
      (v) =>
        v.name.toLowerCase().includes(q) ||
        (v.driver ?? "").toLowerCase().includes(q) ||
        (v.mountpoint ?? "").toLowerCase().includes(q)
    );
  });

  function setBusy(name: string, on: boolean) {
    const next = new Set(busy);
    if (on) next.add(name);
    else next.delete(name);
    busy = next;
  }

  async function load() {
    if (engineState !== "running") {
      volumes = [];
      if (engineState === "stopped") {
        errorMsg = "Engine is stopped. Start the engine to see volumes.";
      } else if (engineState === "not-provisioned") {
        errorMsg = "Engine is not provisioned. Set up the engine first.";
      } else {
        errorMsg = "";
      }
      return;
    }
    if (loadGuard) return;
    loadGuard = true;
    loading = true;
    try {
      const raw = await volumeList();
      const list = Array.isArray(raw) ? raw : [];
      list.sort((a, b) => a.name.localeCompare(b.name));
      volumes = list;
      errorMsg = "";
    } catch (e) {
      errorMsg = `Failed to load volumes: ${errText(e)}`;
    } finally {
      loading = false;
      loadGuard = false;
    }
  }

  // Reload on mount and whenever the engine state or refresh token changes.
  $effect(() => {
    void engineState;
    void refreshKey;
    load();
  });

  async function onCreate(e: SubmitEvent) {
    e.preventDefault();
    const name = newName.trim();
    if (!name || creating) return;
    creating = true;
    errorMsg = "";
    try {
      const driver = newDriver.trim();
      await volumeCreate(name, driver ? driver : undefined);
      newName = "";
      newDriver = "";
      showCreate = false;
      await load();
    } catch (err) {
      errorMsg = `Failed to create volume: ${errText(err)}`;
    } finally {
      creating = false;
    }
  }

  async function onRemove(v: Volume) {
    if (busy.has(v.name)) return;
    const msg = forceRemove
      ? `Force-remove volume "${v.name}"? This deletes its data even if in use.`
      : `Remove volume "${v.name}"? This permanently deletes its data.`;
    if (!confirm(msg)) return;
    setBusy(v.name, true);
    errorMsg = "";
    try {
      await volumeRemove(v.name, forceRemove);
      if (inspectName === v.name) {
        inspectName = null;
        inspectJson = "";
      }
      await load();
    } catch (err) {
      errorMsg = `Failed to remove volume "${v.name}": ${errText(err)}`;
    } finally {
      setBusy(v.name, false);
    }
  }

  async function onInspect(v: Volume) {
    // Toggle closed if already showing this volume.
    if (inspectName === v.name) {
      inspectName = null;
      inspectJson = "";
      return;
    }
    inspecting = true;
    inspectName = v.name;
    inspectJson = "";
    errorMsg = "";
    try {
      inspectJson = await volumeInspect(v.name);
    } catch (err) {
      errorMsg = `Failed to inspect volume "${v.name}": ${errText(err)}`;
      inspectName = null;
    } finally {
      inspecting = false;
    }
  }

  async function onPrune() {
    if (pruning) return;
    if (!confirm("Remove all unused (dangling) volumes? This cannot be undone.")) {
      return;
    }
    pruning = true;
    pruneMsg = "";
    errorMsg = "";
    try {
      const res = await volumePrune();
      const count = res.removed?.length ?? 0;
      pruneMsg =
        count === 0
          ? "No unused volumes to remove."
          : `Removed ${count} volume${count === 1 ? "" : "s"}, reclaimed ${humanSize(res.space_reclaimed)}.`;
      await load();
    } catch (err) {
      errorMsg = `Failed to prune volumes: ${errText(err)}`;
    } finally {
      pruning = false;
    }
  }

  // --- pure formatting helpers ---
  function humanSize(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let n = bytes;
    let i = 0;
    while (n >= 1024 && i < units.length - 1) {
      n /= 1024;
      i++;
    }
    const val = i === 0 ? n : n.toFixed(n < 10 ? 1 : 0);
    return `${val} ${units[i]}`;
  }

  function fmtCreated(created: string | null): string {
    if (!created) return "—";
    const d = new Date(created);
    if (Number.isNaN(d.getTime())) return created;
    return d.toLocaleString();
  }

  // Shared grid template for header + rows (Name / Driver / Mountpoint / Created).
  const COLS = "minmax(180px,1.6fr) 110px minmax(200px,2fr) 170px";
</script>

<div class="page">
  <div class="head">
    <h1>Volumes</h1>
    {#if volumes.length}
      <span class="chip"><b class="num">{volumes.length}</b> total</span>
    {/if}
    <span class="sp"></span>

    <label class="search" style="width:220px">
      <Search aria-hidden="true" />
      <input
        type="text"
        placeholder="Filter volumes"
        bind:value={filter}
        aria-label="Filter volumes"
      />
    </label>

    <label class="field" title="Force removal even when a volume is in use">
      <input type="checkbox" bind:checked={forceRemove} />
      Force remove
    </label>

    <button
      class="btn btn-danger"
      disabled={pruning || engineState !== "running"}
      onclick={onPrune}
      title="Remove all unused volumes"
    >
      <Eraser aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune unused"}
    </button>

    <button
      class="btn btn-soft"
      class:on={showCreate}
      disabled={engineState !== "running"}
      onclick={() => (showCreate = !showCreate)}
    >
      <Plus aria-hidden="true" />
      New volume
    </button>
  </div>

  {#if errorMsg}
    <div class="banner err">
      <CircleAlert aria-hidden="true" />
      <span>{errorMsg}</span>
    </div>
  {/if}

  {#if pruneMsg}
    <div class="banner">
      <Info aria-hidden="true" />
      <span>{pruneMsg}</span>
    </div>
  {/if}

  {#if showCreate}
    <form class="card card-pad" onsubmit={onCreate}>
      <div class="section-title" style="margin-bottom:12px">New volume</div>
      <div style="display:flex;flex-wrap:wrap;align-items:center;gap:10px">
        <label class="search" style="flex:1;min-width:200px">
          <HardDrive aria-hidden="true" />
          <input
            type="text"
            placeholder="volume name"
            bind:value={newName}
            disabled={creating || engineState !== "running"}
            aria-label="New volume name"
          />
        </label>
        <label class="search" style="width:170px">
          <input
            type="text"
            placeholder="driver (local)"
            bind:value={newDriver}
            disabled={creating || engineState !== "running"}
            aria-label="Volume driver (optional)"
          />
        </label>
        <button
          class="btn btn-pri"
          type="submit"
          disabled={creating || engineState !== "running" || newName.trim() === ""}
        >
          <Plus aria-hidden="true" />
          {creating ? "Creating…" : "Create volume"}
        </button>
        <button
          class="btn btn-soft"
          type="button"
          onclick={() => (showCreate = false)}
        >
          Cancel
        </button>
      </div>
    </form>
  {/if}

  <div class="table">
    <div class="thead" style="--cols:{COLS}">
      <span>Name</span>
      <span>Driver</span>
      <span>Mountpoint</span>
      <span>Created</span>
    </div>

    {#if shown.length === 0}
      <div class="empty">
        {#if loading}
          Loading volumes…
        {:else if engineState !== "running"}
          Engine not running.
        {:else if filter.trim()}
          No volumes match “{filter.trim()}”.
        {:else}
          No volumes yet.
        {/if}
      </div>
    {:else}
      {#each shown as v (v.name)}
        {@const acting = busy.has(v.name)}
        {@const open = inspectName === v.name}
        <div
          class="trow"
          class:sel={open}
          style="--cols:{COLS};{acting ? 'opacity:.55' : ''}"
          role="button"
          tabindex="0"
          onclick={() => onInspect(v)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onInspect(v);
            }
          }}
        >
          <div class="cell-name">
            <span class="av"><HardDrive aria-hidden="true" /></span>
            <div style="min-width:0">
              <div class="nm" title={v.name}>{v.name}</div>
              {#if v.scope}
                <div class="id">{v.scope}</div>
              {/if}
            </div>
          </div>

          <span class="text-2">{v.driver || "—"}</span>

          <span
            class="mono text-3"
            title={v.mountpoint}
            style="min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap"
            >{v.mountpoint || "—"}</span
          >

          <span class="num text-3">{fmtCreated(v.created)}</span>

          <div class="rowact">
            <button
              title={open ? "Hide inspect" : "Inspect"}
              disabled={acting}
              onclick={(e) => {
                e.stopPropagation();
                onInspect(v);
              }}
            >
              <Search aria-hidden="true" />
            </button>
            <button
              class="dng"
              title="Remove volume"
              disabled={acting}
              onclick={(e) => {
                e.stopPropagation();
                onRemove(v);
              }}
            >
              <Trash2 aria-hidden="true" />
            </button>
          </div>
        </div>

        {#if open}
          <div style="padding:12px 18px;border-bottom:1px solid var(--line-soft)">
            <div class="outpane">
              <div class="bar">
                <Search aria-hidden="true" />
                <span>Inspect · <span class="mono">{v.name}</span></span>
                <span style="flex:1"></span>
                <button
                  class="btn btn-icon sm"
                  title="Close"
                  onclick={() => onInspect(v)}
                >
                  <X aria-hidden="true" />
                </button>
              </div>
              <pre class="body-out" style="white-space:pre">{inspecting
                  ? "Loading inspect…"
                  : inspectJson}</pre>
            </div>
          </div>
        {/if}
      {/each}
    {/if}
  </div>
</div>

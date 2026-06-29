<script lang="ts">
  // Volumes view. Owns its own fetch lifecycle: loads on mount, when the engine
  // becomes running, and on an explicit refresh request from the parent. Talks
  // to the backend only through volumesApi.ts. Svelte 5 runes API.
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Search from "@lucide/svelte/icons/search";
  import Plus from "@lucide/svelte/icons/plus";
  import Eraser from "@lucide/svelte/icons/eraser";
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

  const BTN_BASE =
    "inline-flex cursor-pointer items-center gap-1 rounded-md border px-2 py-[3px] text-xs transition-colors disabled:cursor-default disabled:opacity-45";
  const INPUT =
    "rounded-md border border-[#262b34] bg-[#0e1117] px-2.5 py-[5px] text-[13px] text-[#e6e8eb] placeholder:text-[#6b7280] focus:border-[#2f81f7] focus:outline-none";
</script>

<section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
  <div class="flex flex-wrap items-center gap-2.5 border-b border-[#262b34] px-3.5 py-3">
    <HardDrive size={16} class="text-[#9aa3af]" aria-hidden="true" />
    <h2 class="text-sm font-semibold">Volumes</h2>
    <span class="text-xs text-[#9aa3af]">
      {volumes.length ? `${volumes.length} total` : ""}
    </span>
    <div class="ml-auto flex items-center gap-2">
      <button
        class="{BTN_BASE} border-[#d2992280] text-[#d29922] hover:not-disabled:bg-[#d299221f]"
        disabled={pruning || engineState !== "running"}
        onclick={onPrune}
        title="Remove all unused volumes"
      >
        <Eraser size={13} aria-hidden="true" />
        {pruning ? "Pruning…" : "Prune unused"}
      </button>
    </div>
  </div>

  <!-- Create volume form -->
  <form
    class="flex flex-wrap items-center gap-2 border-b border-[#262b34] px-3.5 py-2.5"
    onsubmit={onCreate}
  >
    <input
      class={INPUT}
      type="text"
      placeholder="volume name"
      bind:value={newName}
      disabled={creating || engineState !== "running"}
      aria-label="New volume name"
    />
    <input
      class="{INPUT} w-28"
      type="text"
      placeholder="driver (local)"
      bind:value={newDriver}
      disabled={creating || engineState !== "running"}
      aria-label="Volume driver (optional)"
    />
    <button
      class="{BTN_BASE} border-[#3fb95080] text-[#3fb950] hover:not-disabled:bg-[#3fb9501f]"
      type="submit"
      disabled={creating || engineState !== "running" || newName.trim() === ""}
    >
      <Plus size={13} aria-hidden="true" />
      {creating ? "Creating…" : "Create volume"}
    </button>
    <label class="ml-auto flex select-none items-center gap-1.5 text-xs text-[#9aa3af]">
      <input type="checkbox" bind:checked={forceRemove} class="accent-[#f85149]" />
      Force remove
    </label>
  </form>

  {#if pruneMsg}
    <div
      class="mx-3.5 mt-3 select-text rounded-md border border-[#3fb95066] bg-[#3fb9501a] px-3 py-2 text-[13px] text-[#7ee787]"
    >
      {pruneMsg}
    </div>
  {/if}

  {#if errorMsg}
    <div
      class="mx-3.5 mt-3 select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
    >
      {errorMsg}
    </div>
  {/if}

  {#if volumes.length === 0}
    <div class="flex flex-col items-center gap-2 px-3.5 py-9 text-center text-[#9aa3af]">
      <HardDrive size={22} class="opacity-60" aria-hidden="true" />
      <span>
        {#if loading}
          Loading volumes…
        {:else if engineState === "running"}
          No volumes.
        {:else}
          Engine not running.
        {/if}
      </span>
    </div>
  {:else}
    <div class="overflow-x-auto">
      <table class="w-full border-collapse text-[13px]">
        <thead>
          <tr>
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Name</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Driver</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Mountpoint</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Created</th
            >
            <th
              class="sticky top-0 w-[1%] whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Actions</th
            >
          </tr>
        </thead>
        <tbody>
          {#each volumes as v (v.name)}
            {@const acting = busy.has(v.name)}
            <tr class="hover:bg-[#1b1f27] {acting ? 'opacity-60' : ''}">
              <td
                class="max-w-[260px] overflow-hidden text-ellipsis whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle font-medium"
                title={v.name}>{v.name}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{v.driver}</td
              >
              <td
                class="font-mono-app max-w-[320px] overflow-hidden text-ellipsis whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-xs text-[#9aa3af]"
                title={v.mountpoint}>{v.mountpoint}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{fmtCreated(v.created)}</td
              >
              <td class="border-b border-[#262b34] px-3 py-2.5 align-middle">
                <div class="flex justify-end gap-1.5">
                  <button
                    class="{BTN_BASE} border-[#262b34] text-[#e6e8eb] hover:not-disabled:bg-[#21262d]"
                    disabled={acting}
                    onclick={() => onInspect(v)}
                  >
                    <Search size={13} aria-hidden="true" />
                    {inspectName === v.name ? "Hide" : "Inspect"}
                  </button>
                  <button
                    class="{BTN_BASE} border-[#f8514980] text-[#f85149] hover:not-disabled:bg-[#f851491f]"
                    disabled={acting}
                    onclick={() => onRemove(v)}
                  >
                    <Trash2 size={13} aria-hidden="true" />Remove
                  </button>
                </div>
              </td>
            </tr>
            {#if inspectName === v.name}
              <tr>
                <td colspan="5" class="border-b border-[#262b34] bg-[#0e1117] px-3.5 py-3">
                  {#if inspecting}
                    <span class="text-xs text-[#9aa3af]">Loading inspect…</span>
                  {:else}
                    <pre
                      class="font-mono-app max-h-80 select-text overflow-auto rounded-md border border-[#262b34] bg-[#0a0c10] p-3 text-xs leading-relaxed text-[#e6e8eb]">{inspectJson}</pre>
                  {/if}
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

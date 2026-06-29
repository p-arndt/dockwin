<script lang="ts">
  // SYSTEM view: disk usage, reclaim-space (prune), and engine info. Owns its own
  // fetch lifecycle: loads on mount, when the engine state changes, and on an
  // explicit refresh request from the parent. Talks to the backend only through
  // systemApi.ts.
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Info from "@lucide/svelte/icons/info";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Cpu from "@lucide/svelte/icons/cpu";
  import {
    systemDf,
    systemInfo,
    systemPrune,
    humanBytes,
    type SystemDfDto,
    type SystemInfoDto,
  } from "./systemApi";
  import type { EngineState } from "./types";

  interface Props {
    engineState?: EngineState;
    // Monotonically increasing token; bump it from the parent to force a reload.
    refreshKey?: number;
  }

  let { engineState = "unknown", refreshKey = 0 }: Props = $props();

  let df = $state<SystemDfDto | null>(null);
  let info = $state<SystemInfoDto | null>(null);
  let errorMsg = $state("");
  let loading = $state(false);

  // Prune controls + state.
  let allImages = $state(false);
  let pruneVolumes = $state(false);
  let pruning = $state(false);
  let pruneResult = $state("");

  let busy = false; // non-reactive guard against overlapping loads

  function errText(e: unknown): string {
    if (e == null) return "Unknown error";
    if (typeof e === "string") return e;
    if (typeof e === "object" && "message" in e) {
      const msg = (e as { message?: unknown }).message;
      if (typeof msg === "string") return msg;
    }
    return String(e);
  }

  async function load() {
    if (engineState !== "running") {
      df = null;
      info = null;
      if (engineState === "stopped") {
        errorMsg = "Engine is stopped. Start the engine to see system info.";
      } else if (engineState === "not-provisioned") {
        errorMsg = "Engine is not provisioned. Set up the engine first.";
      } else {
        errorMsg = "";
      }
      return;
    }
    if (busy) return;
    busy = true;
    loading = true;
    try {
      const [dfRes, infoRes] = await Promise.all([systemDf(), systemInfo()]);
      df = dfRes;
      info = infoRes;
      errorMsg = "";
    } catch (e) {
      errorMsg = `Failed to load system info: ${errText(e)}`;
    } finally {
      loading = false;
      busy = false;
    }
  }

  // Reload on mount and whenever the engine state or refresh token changes.
  $effect(() => {
    void engineState;
    void refreshKey;
    load();
  });

  async function runPrune() {
    if (pruning || engineState !== "running") return;
    const parts = ["stopped containers", allImages ? "all unused images" : "dangling images", "unused networks"];
    if (pruneVolumes) parts.push("unused volumes");
    const ok = confirm(`This will permanently remove: ${parts.join(", ")}.\n\nContinue?`);
    if (!ok) return;
    pruning = true;
    pruneResult = "";
    errorMsg = "";
    try {
      const res = await systemPrune(allImages, pruneVolumes);
      pruneResult = `Reclaimed ${humanBytes(res.space_reclaimed)} · ${res.containers_deleted} container(s), ${res.images_deleted} image(s), ${res.networks_deleted} network(s), ${res.volumes_deleted} volume(s) removed.`;
      await load(); // refresh the disk-usage table
    } catch (e) {
      errorMsg = `Prune failed: ${errText(e)}`;
    } finally {
      pruning = false;
    }
  }

  // Rows for the disk-usage table.
  const dfRows = $derived(
    df
      ? [
          { label: "Images", u: df.images },
          { label: "Containers", u: df.containers },
          { label: "Volumes", u: df.volumes },
          { label: "Build cache", u: df.build_cache },
        ]
      : [],
  );

  // Key/value rows for the engine-info card.
  const infoRows = $derived(
    info
      ? [
          { k: "Server version", v: info.server_version ?? "—" },
          { k: "OS", v: info.os ?? "—" },
          { k: "OS type", v: info.os_type ?? "—" },
          { k: "Kernel", v: info.kernel_version ?? "—" },
          { k: "Architecture", v: info.architecture ?? "—" },
          { k: "CPUs", v: info.ncpu != null ? String(info.ncpu) : "—" },
          { k: "Memory", v: info.mem_total != null ? humanBytes(info.mem_total) : "—" },
          { k: "Storage driver", v: info.storage_driver ?? "—" },
          {
            k: "Containers",
            v:
              info.containers != null
                ? `${info.containers} (${info.containers_running ?? 0} running)`
                : "—",
          },
          { k: "Images", v: info.images != null ? String(info.images) : "—" },
        ]
      : [],
  );
</script>

<div class="flex flex-col gap-4">
  {#if errorMsg}
    <div
      class="select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
    >
      {errorMsg}
    </div>
  {/if}

  <!-- (a) Disk usage -->
  <section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
    <div class="flex items-center gap-2 border-b border-[#262b34] px-3.5 py-3">
      <HardDrive size={16} class="text-[#9aa3af]" aria-hidden="true" />
      <h2 class="text-sm font-semibold">Disk usage</h2>
      {#if loading}
        <span class="text-xs text-[#9aa3af]">Loading…</span>
      {/if}
    </div>

    {#if dfRows.length === 0}
      <div class="px-3.5 py-7 text-center text-[#9aa3af]">
        {#if loading}Loading…{:else if engineState === "running"}No data.{:else}Engine not running.{/if}
      </div>
    {:else}
      <table class="w-full border-collapse text-[13px]">
        <thead>
          <tr>
            <th class="border-b border-[#262b34] px-3.5 py-2.5 text-left font-medium text-[#9aa3af]">Type</th>
            <th class="border-b border-[#262b34] px-3.5 py-2.5 text-right font-medium text-[#9aa3af]">Count</th>
            <th class="border-b border-[#262b34] px-3.5 py-2.5 text-right font-medium text-[#9aa3af]">Size</th>
            <th class="border-b border-[#262b34] px-3.5 py-2.5 text-right font-medium text-[#9aa3af]">Reclaimable</th>
          </tr>
        </thead>
        <tbody>
          {#each dfRows as row (row.label)}
            <tr class="hover:bg-[#1b1f27]">
              <td class="border-b border-[#262b34] px-3.5 py-2.5 align-middle font-medium">{row.label}</td>
              <td class="font-mono-app border-b border-[#262b34] px-3.5 py-2.5 text-right align-middle text-[#9aa3af]">{row.u.count}</td>
              <td class="font-mono-app border-b border-[#262b34] px-3.5 py-2.5 text-right align-middle text-[#9aa3af]">{humanBytes(row.u.size)}</td>
              <td class="font-mono-app border-b border-[#262b34] px-3.5 py-2.5 text-right align-middle text-[#9aa3af]">{humanBytes(row.u.reclaimable)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <!-- (b) Reclaim space -->
  <section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
    <div class="flex items-center gap-2 border-b border-[#262b34] px-3.5 py-3">
      <Trash2 size={16} class="text-[#9aa3af]" aria-hidden="true" />
      <h2 class="text-sm font-semibold">Reclaim space</h2>
    </div>
    <div class="flex flex-col gap-3 px-3.5 py-3.5">
      <p class="text-[13px] text-[#9aa3af]">
        Removes stopped containers, unused images, and unused networks.
      </p>
      <label class="flex cursor-pointer items-center gap-2 text-[13px] text-[#e6e8eb]">
        <input
          type="checkbox"
          class="accent-[#2f81f7]"
          bind:checked={allImages}
          disabled={pruning || engineState !== "running"}
        />
        Remove ALL unused images, not just dangling
      </label>
      <label class="flex cursor-pointer items-center gap-2 text-[13px] text-[#e6e8eb]">
        <input
          type="checkbox"
          class="accent-[#2f81f7]"
          bind:checked={pruneVolumes}
          disabled={pruning || engineState !== "running"}
        />
        Also remove unused volumes
      </label>
      <div class="flex items-center gap-3">
        <button
          class="flex items-center gap-1.5 rounded-md border border-[#2f81f7]/50 bg-[#2f81f71a] px-3 py-[6px] text-[13px] text-[#2f81f7] transition-colors hover:not-disabled:bg-[#2f81f726] disabled:cursor-default disabled:opacity-40"
          disabled={pruning || engineState !== "running"}
          onclick={runPrune}
        >
          <Trash2 size={14} aria-hidden="true" />
          {pruning ? "Pruning…" : "Prune unused"}
        </button>
        {#if pruneResult}
          <span class="text-[13px] text-[#5ad17a]">{pruneResult}</span>
        {/if}
      </div>
    </div>
  </section>

  <!-- (c) Engine info -->
  <section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
    <div class="flex items-center gap-2 border-b border-[#262b34] px-3.5 py-3">
      <Info size={16} class="text-[#9aa3af]" aria-hidden="true" />
      <h2 class="text-sm font-semibold">Engine info</h2>
      {#if info?.name}
        <span class="flex items-center gap-1 text-xs text-[#9aa3af]">
          <Cpu size={12} aria-hidden="true" />{info.name}
        </span>
      {/if}
    </div>

    {#if infoRows.length === 0}
      <div class="px-3.5 py-7 text-center text-[#9aa3af]">
        {#if loading}Loading…{:else if engineState === "running"}No data.{:else}Engine not running.{/if}
      </div>
    {:else}
      <dl class="divide-y divide-[#262b34]">
        {#each infoRows as row (row.k)}
          <div class="flex items-baseline gap-3 px-3.5 py-2">
            <dt class="w-36 shrink-0 text-[13px] text-[#9aa3af]">{row.k}</dt>
            <dd class="font-mono-app select-text text-[13px] text-[#e6e8eb]">{row.v}</dd>
          </div>
        {/each}
      </dl>
    {/if}
  </section>
</div>

<script lang="ts">
  // SYSTEM view: disk usage, reclaim-space (prune), and engine info. Owns its own
  // fetch lifecycle: loads on mount, when the engine state changes, and on an
  // explicit refresh request from the parent. Talks to the backend only through
  // systemApi.ts.
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Layers from "@lucide/svelte/icons/layers";
  import Boxes from "@lucide/svelte/icons/boxes";
  import Database from "@lucide/svelte/icons/database";
  import Server from "@lucide/svelte/icons/server";
  import Cpu from "@lucide/svelte/icons/cpu";
  import MemoryStick from "@lucide/svelte/icons/memory-stick";
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import {
    systemDf,
    systemInfo,
    systemPrune,
    humanBytes,
    type SystemDfDto,
    type SystemInfoDto,
  } from "../api/system";
  import type { EngineState } from "../types";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { confirmDialog } from "../state/confirm.svelte.js";

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
    if (
      !(await confirmDialog({
        title: "Prune unused data?",
        description: `This will permanently remove: ${parts.join(", ")}.`,
        destructive: true,
        confirmText: "Prune",
      }))
    )
      return;
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

  const engineRunning = $derived(engineState === "running");

  // Split a byte count into its numeric part and unit, so the unit can render
  // as a quiet <small> next to a loud value.
  function splitBytes(n: number): { v: string; u: string } {
    const s = humanBytes(n);
    const i = s.indexOf(" ");
    return i === -1 ? { v: s, u: "" } : { v: s.slice(0, i), u: s.slice(i + 1) };
  }

  // Clamp a reclaimable/size ratio into a 0-100 bar width.
  function reclaimPct(u: { size: number; reclaimable: number }): number {
    if (!u.size || u.size <= 0) return 0;
    return Math.max(0, Math.min(100, (u.reclaimable / u.size) * 100));
  }

  // Disk-usage stat cards. icon is a component reference.
  const diskCards = $derived(
    df
      ? [
          { label: "Images", icon: Layers, u: df.images },
          { label: "Containers", icon: Boxes, u: df.containers },
          { label: "Volumes", icon: Database, u: df.volumes },
          { label: "Build cache", icon: HardDrive, u: df.build_cache },
        ]
      : [],
  );

  // Total reclaimable across all categories (chip in the header).
  const totalReclaimable = $derived(
    df
      ? df.images.reclaimable +
          df.containers.reclaimable +
          df.volumes.reclaimable +
          df.build_cache.reclaimable
      : 0,
  );

  // The ONE focused (accent) card: whichever category has the most reclaimable
  // space — the actionable one. Falls back to none when nothing is reclaimable.
  const focusLabel = $derived.by(() => {
    if (!diskCards.length) return "";
    let best = diskCards[0];
    for (const c of diskCards) if (c.u.reclaimable > best.u.reclaimable) best = c;
    return best.u.reclaimable > 0 ? best.label : "";
  });

  // Key/value rows for the engine-info card. cls drives value typography:
  // "mono" for technical strings, "num" for counts/sizes, "" for prose.
  const infoRows = $derived(
    info
      ? [
          { k: "Server version", v: info.server_version ?? "—", cls: "mono" },
          { k: "OS", v: info.os ?? "—", cls: "" },
          { k: "OS type", v: info.os_type ?? "—", cls: "" },
          { k: "Kernel", v: info.kernel_version ?? "—", cls: "mono" },
          { k: "Architecture", v: info.architecture ?? "—", cls: "mono" },
          { k: "CPUs", v: info.ncpu != null ? String(info.ncpu) : "—", cls: "num" },
          { k: "Memory", v: info.mem_total != null ? humanBytes(info.mem_total) : "—", cls: "num" },
          { k: "Storage driver", v: info.storage_driver ?? "—", cls: "mono" },
          {
            k: "Containers",
            v:
              info.containers != null
                ? `${info.containers} (${info.containers_running ?? 0} running)`
                : "—",
            cls: "num",
          },
          { k: "Images", v: info.images != null ? String(info.images) : "—", cls: "num" },
        ]
      : [],
  );

  // Human label for the engine state shown in the header chip when not running.
  const stateLabel = $derived(
    engineState === "stopped"
      ? "Engine stopped"
      : engineState === "not-provisioned"
        ? "Engine not provisioned"
        : engineState === "broken"
          ? "Engine broken"
          : "Engine unknown",
  );
</script>

<div class="page">
  <!-- Page header -->
  <div class="head">
    <h1>System</h1>
    {#if engineRunning}
      <span class="chip">
        <span class="d"></span>Engine running
        {#if info?.server_version}<span class="x">·</span><b class="mono">{info.server_version}</b>{/if}
      </span>
    {:else}
      <span class="chip"><span class="dot-off"></span>{stateLabel}</span>
    {/if}
    {#if df && totalReclaimable > 0}
      <span class="chip"><b class="num">{humanBytes(totalReclaimable)}</b> reclaimable</span>
    {/if}
    {#if loading}<span class="chip">Loading…</span>{/if}
    <span class="sp"></span>
  </div>

  {#if errorMsg}
    <div class="banner err">
      <TriangleAlert aria-hidden="true" />
      <span>{errorMsg}</span>
    </div>
  {/if}

  <!-- Disk usage -->
  <section class="sec-block">
    <div class="section-title">Disk usage</div>
    {#if diskCards.length === 0}
      <div class="card card-pad muted-note">
        {#if loading}Loading…{:else if engineRunning}No data.{:else}Engine not running.{/if}
      </div>
    {:else}
      <div class="statgrid wide">
        {#each diskCards as c (c.label)}
          {@const s = splitBytes(c.u.size)}
          {@const Icon = c.icon}
          <div class="stat" class:focus={c.label === focusLabel}>
            <div class="k"><Icon aria-hidden="true" />{c.label}</div>
            <div class="big num">{s.v}<small>{s.u}</small></div>
            <div class="sub2">
              {c.u.count} item{c.u.count === 1 ? "" : "s"} · {humanBytes(c.u.reclaimable)} reclaimable
            </div>
            <div class="mbar"><i style="width:{reclaimPct(c.u)}%"></i></div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Reclaim space -->
  <section class="card card-pad reclaim">
    <div class="reclaim-head">
      <span class="ic"><Trash2 aria-hidden="true" /></span>
      <div>
        <div class="section-title">Reclaim space</div>
        <p class="prose">Removes stopped containers, unused images, and unused networks.</p>
      </div>
    </div>

    <div class="field">
      <Checkbox
        id="prune-all-images"
        bind:checked={allImages}
        disabled={pruning || !engineRunning}
      />
      <Label for="prune-all-images">Remove ALL unused images, not just dangling</Label>
    </div>
    <div class="field">
      <Checkbox
        id="prune-volumes"
        bind:checked={pruneVolumes}
        disabled={pruning || !engineRunning}
      />
      <Label for="prune-volumes">Also remove unused volumes</Label>
    </div>

    <div class="reclaim-acts">
      <Button
        disabled={pruning || !engineRunning}
        onclick={runPrune}
      >
        <Trash2 aria-hidden="true" />
        {pruning ? "Pruning…" : "Prune unused"}
      </Button>
      {#if pruneResult}
        <span class="prune-ok"><CircleCheck aria-hidden="true" />{pruneResult}</span>
      {/if}
    </div>
  </section>

  <!-- Engine info -->
  <section class="card card-pad">
    <div class="kv">
      <div class="sec engine-sec">
        <span class="ic-sm"><Server aria-hidden="true" /></span>
        Engine info
        {#if info?.name}
          <span class="engine-name"><Cpu aria-hidden="true" />{info.name}</span>
        {/if}
      </div>
      {#if infoRows.length === 0}
        <div class="muted-note plain">
          {#if loading}Loading…{:else if engineRunning}No data.{:else}Engine not running.{/if}
        </div>
      {:else}
        {#each infoRows as row (row.k)}
          <div class="r">
            <span class="k">{row.k}</span>
            <span class="v {row.cls}">{row.v}</span>
          </div>
        {/each}
      {/if}
    </div>
  </section>
</div>

<style>
  .sec-block {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .muted-note {
    color: var(--text-3);
    text-align: center;
    padding-top: 26px;
    padding-bottom: 26px;
  }
  .muted-note.plain {
    border: 0;
    box-shadow: none;
    background: transparent;
    padding: 18px 0 8px;
  }

  /* Reclaim card layout */
  .reclaim {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .reclaim-head {
    display: flex;
    align-items: flex-start;
    gap: 11px;
  }
  .reclaim-head .prose {
    margin-top: 4px;
  }
  .ic {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    flex: none;
    display: grid;
    place-items: center;
    background: linear-gradient(180deg, var(--s3), var(--s2));
    border: 1px solid var(--line);
    color: var(--text-3);
    box-shadow: inset 0 1px 0 var(--hi);
  }
  .ic :global(svg) {
    width: 15px;
    height: 15px;
  }
  .reclaim-acts {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .prune-ok {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--ok);
    -webkit-user-select: text;
    user-select: text;
  }
  .prune-ok :global(svg) {
    width: 14px;
    height: 14px;
    flex: none;
  }

  /* Engine-info section header */
  .engine-sec {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ic-sm {
    display: inline-flex;
    color: var(--text-4);
  }
  .ic-sm :global(svg) {
    width: 14px;
    height: 14px;
  }
  .engine-name {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-3);
    font-family: var(--mono);
  }
  .engine-name :global(svg) {
    width: 12px;
    height: 12px;
    color: var(--text-4);
  }

  /* value typography helpers (kv .v already supplies base styling) */
  .v.num {
    font-variant-numeric: tabular-nums;
  }

  /* quiet off-dot for non-running header chip */
  .dot-off {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--off);
    flex: none;
  }
</style>

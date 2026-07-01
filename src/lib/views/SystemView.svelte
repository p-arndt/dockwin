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
  import StatusDot from "../components/StatusDot.svelte";
  import {
    systemDf,
    systemInfo,
    systemPrune,
    systemWipe,
    humanBytes,
    type SystemDfDto,
    type SystemInfoDto,
  } from "../api/system";
  import type { EngineState } from "../types";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
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

  // Full-wipe state (force-remove everything, in use or not).
  let wiping = $state(false);

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

  // Full engine wipe: force-remove everything, whether or not it's in use.
  // A hard superset of prune — separate handler with its own stern confirm.
  async function runWipe() {
    if (wiping || pruning || engineState !== "running") return;
    const parts: string[] = [];
    if (df) {
      if (df.containers.count) parts.push(`${df.containers.count} container(s)`);
      if (df.images.count) parts.push(`${df.images.count} image(s)`);
      if (df.volumes.count) parts.push(`${df.volumes.count} volume(s)`);
    }
    const running = info?.containers_running ?? 0;
    const description =
      (parts.length
        ? `This will force-remove ${parts.join(", ")} plus all user-defined networks`
        : "This will force-remove all containers, images, volumes, and user-defined networks") +
      (running ? `, including ${running} running container(s)` : "") +
      ". Everything is deleted whether or not it is in use. This cannot be undone.";
    if (
      !(await confirmDialog({
        title: "Remove everything?",
        description,
        destructive: true,
        confirmText: "Remove everything",
      }))
    )
      return;
    wiping = true;
    pruneResult = "";
    errorMsg = "";
    try {
      const res = await systemWipe();
      pruneResult = `Removed everything · reclaimed ${humanBytes(res.space_reclaimed)} · ${res.containers_deleted} container(s), ${res.images_deleted} image(s), ${res.networks_deleted} network(s), ${res.volumes_deleted} volume(s).`;
      await load(); // refresh the disk-usage table
    } catch (e) {
      errorMsg = `Remove everything failed: ${errText(e)}`;
    } finally {
      wiping = false;
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

<div class="flex items-end gap-[14px] shrink-0 pt-[22px] px-[22px] pb-[16px]">
  <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">System</h1>
  {#if engineRunning}
    <Badge variant="secondary" class="gap-1.5 font-normal">
      <StatusDot tone="run" size={6} />Engine running
      {#if info?.server_version}<span class="text-muted-foreground/70">·</span><b class="font-mono tabular-nums">{info.server_version}</b>{/if}
    </Badge>
  {:else}
    <Badge variant="secondary" class="gap-1.5 font-normal"><StatusDot tone="off" size={6} />{stateLabel}</Badge>
  {/if}
  {#if df && totalReclaimable > 0}
    <Badge variant="secondary" class="gap-1.5 font-normal"><b class="tabular-nums">{humanBytes(totalReclaimable)}</b> reclaimable</Badge>
  {/if}
  {#if loading}<Badge variant="secondary" class="gap-1.5 font-normal">Loading…</Badge>{/if}
  <span class="flex-1"></span>
</div>
<div class="flex-1 overflow-auto min-h-0 flex flex-col gap-[24px] min-w-0 px-[22px] pb-[24px]">
  {#if errorMsg}
    <Alert.Root variant="destructive">
      <TriangleAlert aria-hidden="true" />
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {/if}

  <!-- Disk usage -->
  <section class="flex flex-col gap-[10px]">
    <div class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70">Disk usage</div>
    {#if diskCards.length === 0}
      <div class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] px-[18px] pt-[26px] pb-[26px] text-center text-muted-foreground">
        {#if loading}Loading…{:else if engineRunning}No data.{:else}Engine not running.{/if}
      </div>
    {:else}
      <div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-[10px]">
        {#each diskCards as c (c.label)}
          {@const s = splitBytes(c.u.size)}
          {@const Icon = c.icon}
          <div class="relative overflow-hidden bg-card border border-border rounded-[9px] py-[13px] px-[14px] shadow-sm">
            <div class="flex items-center gap-[6px] text-[11px] font-medium text-muted-foreground [&_svg]:size-[13px] [&_svg]:text-muted-foreground/70"><Icon aria-hidden="true" />{c.label}</div>
            <div class="text-[21px] font-[680] tracking-[-0.5px] mt-[8px] tabular-nums">{s.v}<small class="text-[12px] font-medium text-muted-foreground ml-[2px]">{s.u}</small></div>
            <div class="text-[11px] text-muted-foreground/70 tabular-nums mt-[2px]">
              {c.u.count} item{c.u.count === 1 ? "" : "s"} · {humanBytes(c.u.reclaimable)} reclaimable
            </div>
            <div class="relative overflow-hidden h-[4px] rounded-[3px] bg-muted mt-[9px]"><i class={(c.label === focusLabel ? "bg-primary" : "bg-muted-foreground") + " absolute inset-y-0 left-0 rounded-[3px]"} style="width:{reclaimPct(c.u)}%"></i></div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Reclaim space -->
  <section class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] py-[16px] px-[18px] flex flex-col gap-[12px]">
    <div class="flex items-start gap-[11px]">
      <span class="size-[30px] rounded-[8px] shrink-0 grid place-items-center bg-muted border border-border text-muted-foreground [&_svg]:size-[15px]"><Trash2 aria-hidden="true" /></span>
      <div>
        <div class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70">Reclaim space</div>
        <p class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground mt-[4px]">Removes stopped containers, unused images, and unused networks — only things not in use.</p>
      </div>
    </div>

    <div class="flex items-center gap-[9px] text-[13px] text-muted-foreground">
      <Checkbox
        id="prune-all-images"
        bind:checked={allImages}
        disabled={pruning || wiping || !engineRunning}
      />
      <Label for="prune-all-images">Remove all unused images, not just dangling</Label>
    </div>
    <div class="flex items-center gap-[9px] text-[13px] text-muted-foreground">
      <Checkbox
        id="prune-volumes"
        bind:checked={pruneVolumes}
        disabled={pruning || wiping || !engineRunning}
      />
      <Label for="prune-volumes">Also remove unused volumes</Label>
    </div>

    <div class="flex items-center gap-[12px] flex-wrap mt-[2px]">
      <Button
        disabled={pruning || wiping || !engineRunning}
        onclick={runPrune}
      >
        <Trash2 aria-hidden="true" />
        {pruning ? "Pruning…" : "Prune unused"}
      </Button>
      {#if pruneResult}
        <span class="inline-flex items-center gap-[7px] text-[12.5px] text-chart-2 select-text [&_svg]:size-[14px] [&_svg]:shrink-0"><CircleCheck aria-hidden="true" />{pruneResult}</span>
      {/if}
    </div>

    <!-- Danger zone: force-remove everything, in use or not. -->
    <div class="mt-[6px] pt-[14px] border-t border-destructive/20 flex flex-col gap-[10px]">
      <div class="flex items-start gap-[11px]">
        <span class="size-[30px] rounded-[8px] shrink-0 grid place-items-center bg-destructive/10 border border-destructive/25 text-destructive [&_svg]:size-[15px]"><TriangleAlert aria-hidden="true" /></span>
        <div>
          <div class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-destructive/80">Danger zone</div>
          <p class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground mt-[4px]">Force-removes <span class="text-foreground font-medium">everything</span> — all containers (even running), images, volumes, and networks — whether or not they're in use. This can't be undone.</p>
        </div>
      </div>
      <div>
        <Button
          disabled={pruning || wiping || !engineRunning}
          onclick={runWipe}
          variant="destructive"
        >
          <TriangleAlert aria-hidden="true" />
          {wiping ? "Removing…" : "Remove everything"}
        </Button>
      </div>
    </div>
  </section>

  <!-- Engine info -->
  <section class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] py-[16px] px-[18px]">
    <div class="flex flex-col">
      <div class="flex items-center gap-[8px] text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70 pt-[4px] pb-[9px]">
        <span class="inline-flex text-muted-foreground/70 [&_svg]:size-[14px]"><Server aria-hidden="true" /></span>
        Engine info
        {#if info?.name}
          <span class="ml-auto inline-flex items-center gap-[5px] text-[11px] font-medium tracking-normal normal-case text-muted-foreground font-mono [&_svg]:size-[12px] [&_svg]:text-muted-foreground/70"><Cpu aria-hidden="true" />{info.name}</span>
        {/if}
      </div>
      {#if infoRows.length === 0}
        <div class="text-center text-muted-foreground pt-[18px] pb-[8px]">
          {#if loading}Loading…{:else if engineRunning}No data.{:else}Engine not running.{/if}
        </div>
      {:else}
        {#each infoRows as row (row.k)}
          <div class="grid grid-cols-[120px_1fr] gap-[10px] py-[8px] border-t border-border items-start">
            <span class="text-[12.5px] text-muted-foreground">{row.k}</span>
            <span class={"text-left break-words text-foreground " + (row.cls === "mono" ? "font-mono text-[11.5px]" : row.cls === "num" ? "text-[12.5px] tabular-nums" : "text-[12.5px]")}>{row.v}</span>
          </div>
        {/each}
      {/if}
    </div>
  </section>
</div>

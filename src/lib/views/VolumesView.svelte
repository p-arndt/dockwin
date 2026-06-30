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
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { confirmDialog } from "../state/confirm.svelte.js";
  import { errText } from "../api";
  import {
    volumeList,
    volumeCreate,
    volumeRemove,
    volumePrune,
    volumeInspect,
    type Volume,
  } from "../api/volumes";
  import type { EngineState } from "../types";

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
    const desc = forceRemove
      ? `Force-remove volume "${v.name}"? This deletes its data even if in use.`
      : `Remove volume "${v.name}"? This permanently deletes its data.`;
    if (
      !(await confirmDialog({
        title: "Remove volume?",
        description: desc,
        destructive: true,
        confirmText: forceRemove ? "Force remove" : "Remove",
      }))
    )
      return;
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
    if (
      !(await confirmDialog({
        title: "Prune unused volumes?",
        description: "Remove all unused (dangling) volumes? This cannot be undone.",
        destructive: true,
        confirmText: "Prune",
      }))
    )
      return;
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
</script>

<div class="flex flex-col min-w-0 gap-[16px] pt-[18px] px-[22px] pb-[24px]">
  <div class="flex items-end shrink-0 gap-[14px] pt-[22px] px-[22px] pb-[16px]">
    <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-[1]">Volumes</h1>
    {#if volumes.length}
      <Badge variant="secondary" class="gap-1.5 font-normal"
        ><b class="tabular-nums">{volumes.length}</b> total</Badge
      >
    {/if}
    <span class="flex-1"></span>

    <div class="relative w-[220px]">
      <Search
        class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
        aria-hidden="true"
      />
      <Input
        class="pl-8"
        placeholder="Filter volumes"
        bind:value={filter}
        aria-label="Filter volumes"
      />
    </div>

    <div class="flex items-center gap-[9px] text-[13px] text-muted-foreground" title="Force removal even when a volume is in use">
      <Checkbox id="vol-force-remove" bind:checked={forceRemove} />
      <Label for="vol-force-remove">Force remove</Label>
    </div>

    <Button
      variant="destructive"
      disabled={pruning || engineState !== "running"}
      onclick={onPrune}
      title="Remove all unused volumes"
    >
      <Eraser aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune unused"}
    </Button>

    <Button
      variant={showCreate ? "secondary" : "outline"}
      disabled={engineState !== "running"}
      onclick={() => (showCreate = !showCreate)}
    >
      <Plus aria-hidden="true" />
      New volume
    </Button>
  </div>

  {#if errorMsg}
    <Alert.Root variant="destructive">
      <CircleAlert aria-hidden="true" />
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {/if}

  {#if pruneMsg}
    <Alert.Root>
      <Info aria-hidden="true" />
      <Alert.Description>{pruneMsg}</Alert.Description>
    </Alert.Root>
  {/if}

  {#if showCreate}
    <form class="bg-card border border-border rounded-[11px] shadow-sm py-[16px] px-[18px]" onsubmit={onCreate}>
      <div class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70 mb-[12px]">New volume</div>
      <div style="display:flex;flex-wrap:wrap;align-items:center;gap:10px">
        <div class="relative flex-1" style="min-width:200px">
          <HardDrive
            class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            class="pl-8"
            placeholder="volume name"
            bind:value={newName}
            disabled={creating || engineState !== "running"}
            aria-label="New volume name"
          />
        </div>
        <Input
          class="w-[170px]"
          placeholder="driver (local)"
          bind:value={newDriver}
          disabled={creating || engineState !== "running"}
          aria-label="Volume driver (optional)"
        />
        <Button
          type="submit"
          disabled={creating || engineState !== "running" || newName.trim() === ""}
        >
          <Plus aria-hidden="true" />
          {creating ? "Creating…" : "Create volume"}
        </Button>
        <Button
          variant="outline"
          type="button"
          onclick={() => (showCreate = false)}
        >
          Cancel
        </Button>
      </div>
    </form>
  {/if}

  <div class="bg-card border border-border rounded-[11px] shadow-sm overflow-hidden">
    <Table.Root class="table-fixed">
      <Table.Header>
        <Table.Row class="hover:bg-transparent">
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:30%">Name</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:12%">Driver</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:38%">Mountpoint</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:12%">Created</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:8%"></Table.Head
          >
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#if shown.length === 0}
          <Table.Row class="hover:bg-transparent">
            <Table.Cell colspan={5} class="py-7 text-center text-muted-foreground">
              {#if loading}
                Loading volumes…
              {:else if engineState !== "running"}
                Engine not running.
              {:else if filter.trim()}
                No volumes match “{filter.trim()}”.
              {:else}
                No volumes yet.
              {/if}
            </Table.Cell>
          </Table.Row>
        {:else}
          {#each shown as v (v.name)}
            {@const acting = busy.has(v.name)}
            {@const open = inspectName === v.name}
            <Table.Row
              class="group relative cursor-pointer data-[sel=true]:bg-muted data-[sel=true]:shadow-[inset_2px_0_0_var(--primary)]"
              data-sel={open}
              style={acting ? "opacity:.55" : undefined}
              role="button"
              tabindex={0}
              aria-busy={acting}
              onclick={() => onInspect(v)}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onInspect(v);
                }
              }}
            >
              <Table.Cell>
                <div class="flex items-center gap-[12px] min-w-0">
                  <span class="grid place-items-center size-[30px] rounded-[8px] shrink-0 bg-muted border border-border text-muted-foreground [&_svg]:size-[15px]"><HardDrive aria-hidden="true" /></span>
                  <div class="min-w-0">
                    <div class="font-semibold text-[13.5px] text-foreground tracking-[-0.1px] leading-[1.25] truncate" title={v.name}>{v.name}</div>
                    {#if v.scope}
                      <div class="font-mono text-[11px] text-muted-foreground/70 leading-[1.3]">{v.scope}</div>
                    {/if}
                  </div>
                </div>
              </Table.Cell>

              <Table.Cell><span class="text-muted-foreground">{v.driver || "—"}</span></Table.Cell>

              <Table.Cell>
                <span
                  class="font-mono tabular-nums text-muted-foreground block min-w-0 truncate"
                  title={v.mountpoint}
                  >{v.mountpoint || "—"}</span
                >
              </Table.Cell>

              <Table.Cell><span class="tabular-nums text-muted-foreground">{fmtCreated(v.created)}</span></Table.Cell>

              <Table.Cell class="text-right">
                <div
                  class="inline-flex justify-end gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 data-[sel=true]:opacity-100"
                  data-sel={open}
                >
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title={open ? "Hide inspect" : "Inspect"}
                    disabled={acting}
                    onclick={(e) => {
                      e.stopPropagation();
                      onInspect(v);
                    }}
                  >
                    <Search aria-hidden="true" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    class="text-muted-foreground hover:text-destructive"
                    title="Remove volume"
                    disabled={acting}
                    onclick={(e) => {
                      e.stopPropagation();
                      onRemove(v);
                    }}
                  >
                    <Trash2 aria-hidden="true" />
                  </Button>
                </div>
              </Table.Cell>
            </Table.Row>

            {#if open}
              <Table.Row class="hover:bg-transparent">
                <Table.Cell colspan={5} class="p-0">
                  <div class="py-[12px] px-[18px] border-b border-border">
                    <div class="border border-border rounded-[9px] bg-background overflow-hidden">
                      <div class="flex items-center gap-[8px] bg-muted border-b border-border py-[8px] px-[12px] text-[12px] text-muted-foreground">
                        <Search aria-hidden="true" />
                        <span>Inspect · <span class="font-mono tabular-nums">{v.name}</span></span>
                        <span class="flex-1"></span>
                        <Button
                          variant="outline"
                          size="icon-sm"
                          title="Close"
                          onclick={() => onInspect(v)}
                        >
                          <X aria-hidden="true" />
                        </Button>
                      </div>
                      <pre class="max-h-[14rem] overflow-auto py-[10px] px-[12px] font-mono text-[11.5px] leading-[1.55] text-muted-foreground select-text" style="white-space:pre">{inspecting
                          ? "Loading inspect…"
                          : inspectJson}</pre>
                    </div>
                  </div>
                </Table.Cell>
              </Table.Row>
            {/if}
          {/each}
        {/if}
      </Table.Body>
    </Table.Root>
  </div>
</div>

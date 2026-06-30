<script lang="ts">
  // Containers screen: header (counts + search affordance) and the list, with
  // the details drawer alongside or — when "full" — replacing the list entirely.
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Search from "@lucide/svelte/icons/search";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import ContainerList from "./ContainerList.svelte";
  import ContainerDetails from "./ContainerDetails.svelte";
  import type { NormalizedContainer } from "../types";

  type Action = "start" | "stop" | "restart" | "remove";

  let {
    containers,
    pending,
    runningCount,
    errorMsg,
    selected,
    detailFull,
    loaded = true,
    onAction,
    onBulkAction,
    onSelect,
    onCloseDetail,
    onToggleFull,
  }: {
    containers: NormalizedContainer[];
    pending: Set<string>;
    runningCount: number;
    errorMsg: string;
    selected: NormalizedContainer | null;
    detailFull: boolean;
    loaded?: boolean;
    onAction: (action: Action, c: NormalizedContainer) => void;
    onBulkAction: (action: Action, containers: NormalizedContainer[]) => void;
    onSelect: (c: NormalizedContainer) => void;
    onCloseDetail: () => void;
    onToggleFull: () => void;
  } = $props();

  // Bulk-selection is page-local presentation state (not part of AppController);
  // it resets naturally on navigation since this component remounts per route.
  let checkedIds = $state<Set<string>>(new Set());
  function toggleChecked(id: string) {
    const next = new Set(checkedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    checkedIds = next;
  }
  function toggleCheckedAll() {
    checkedIds = checkedIds.size === shown.length ? new Set() : new Set(shown.map((c) => c.id));
  }
  const checkedContainers = $derived(containers.filter((c) => checkedIds.has(c.id)));

  // Details live in a right-side Sheet overlay. The expand button toggles a wider
  // sheet so a running container has room for the 2-column stat layout.
  const sheetWidth = $derived(
    detailFull ? "w-[920px]! max-w-[94vw]!" : "w-[560px]! max-w-[94vw]!"
  );

  let filter = $state("");
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return containers;
    return containers.filter(
      (c) => c.name.toLowerCase().includes(q) || c.image.toLowerCase().includes(q)
    );
  });
</script>

<div class="flex items-end gap-[14px] pt-[22px] px-[22px] pb-[16px] shrink-0">
  <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">Containers</h1>
  <Badge variant="secondary" class="gap-1.5 font-normal">
    <span class="size-1.5 rounded-full bg-chart-2"></span>
    <b class="tabular-nums text-foreground">{runningCount}</b> running
    <span class="text-muted-foreground">·</span>
    <b class="tabular-nums text-foreground">{containers.length}</b> total
  </Badge>
  <span class="flex-1"></span>
  <div class="relative w-[220px]">
    <Search
      class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
      aria-hidden="true"
    />
    <Input class="pl-8" placeholder="Filter containers…" bind:value={filter} aria-label="Filter containers" />
  </div>
</div>
<div class="flex-1 overflow-auto min-h-0 px-[22px] pb-[22px]">
  {#if errorMsg}
    <Alert.Root variant="destructive" class="mb-3.5">
      <TriangleAlert aria-hidden="true" />
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {/if}
  {#if checkedContainers.length > 0}
    <div class="flex items-center gap-[10px] rounded-[9px] border border-border bg-muted/50 px-[14px] py-[9px] mb-3.5">
      <span class="text-[12.5px] font-medium text-foreground"
        ><b class="tabular-nums">{checkedContainers.length}</b> selected</span
      >
      <span class="flex-1"></span>
      <Button variant="success" size="sm" onclick={() => onBulkAction("start", checkedContainers)}>
        <Play aria-hidden="true" />Start
      </Button>
      <Button variant="destructive" size="sm" onclick={() => onBulkAction("stop", checkedContainers)}>
        <Square aria-hidden="true" />Stop
      </Button>
      <Button variant="outline" size="sm" onclick={() => onBulkAction("remove", checkedContainers)}>
        <Trash2 aria-hidden="true" />Remove
      </Button>
      <Button variant="ghost" size="sm" onclick={() => (checkedIds = new Set())}>Clear</Button>
    </div>
  {/if}
  <ContainerList
    containers={shown}
    {pending}
    {loaded}
    selectedId={selected?.id ?? null}
    {checkedIds}
    emptyMessage={filter.trim() ? "No containers match the filter." : "No containers."}
    onAction={onAction}
    onSelect={onSelect}
    onToggleChecked={toggleChecked}
    onToggleCheckedAll={toggleCheckedAll}
  />
</div>

<!-- Details as a right-side Sheet overlay; expand widens it (see sheetWidth). -->
<Sheet.Root open={!!selected} onOpenChange={(o) => { if (!o) onCloseDetail(); }}>
  <Sheet.Content side="right" showCloseButton={false} class={`flex flex-col gap-0 p-0 ${sheetWidth}`}>
    {#if selected}
      <ContainerDetails
        container={selected}
        full={detailFull}
        onClose={onCloseDetail}
        onToggleFull={onToggleFull}
      />
    {/if}
  </Sheet.Content>
</Sheet.Root>

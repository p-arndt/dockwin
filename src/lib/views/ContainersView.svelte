<script lang="ts">
  // Containers screen: header (counts + search affordance) and the list, with
  // the details drawer alongside or — when "full" — replacing the list entirely.
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Search from "@lucide/svelte/icons/search";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import ContainerList from "./ContainerList.svelte";
  import ContainerDetails from "./ContainerDetails.svelte";
  import type { NormalizedContainer } from "../types";

  let {
    containers,
    pending,
    runningCount,
    errorMsg,
    selected,
    detailFull,
    onAction,
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
    onAction: (action: "start" | "stop" | "restart" | "remove", c: NormalizedContainer) => void;
    onSelect: (c: NormalizedContainer) => void;
    onCloseDetail: () => void;
    onToggleFull: () => void;
  } = $props();

  // Details live in a right-side Sheet overlay. The expand button toggles a wider
  // sheet so a running container has room for the 2-column stat layout.
  const sheetWidth = $derived(
    detailFull ? "w-[920px]! max-w-[94vw]!" : "w-[560px]! max-w-[94vw]!"
  );
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
  <div class="relative w-[260px]" aria-hidden="true">
    <Search
      class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
      aria-hidden="true"
    />
    <div
      class="flex h-8 items-center gap-2 rounded-lg border border-input bg-card pl-9 pr-2 text-[12.5px] text-muted-foreground"
    >
      <span>Search</span>
      <kbd class="ml-auto rounded border border-border px-1 font-mono text-[10px]">Ctrl K</kbd>
    </div>
  </div>
</div>
<div class="flex-1 overflow-auto min-h-0 px-[22px] pb-[22px]">
  {#if errorMsg}
    <Alert.Root variant="destructive" class="mb-3.5">
      <TriangleAlert aria-hidden="true" />
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {/if}
  <ContainerList {containers} {pending} onAction={onAction} onSelect={onSelect} />
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

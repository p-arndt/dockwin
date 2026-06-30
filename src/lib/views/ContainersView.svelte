<script lang="ts">
  // Containers screen: header (counts + search affordance) and the list, with
  // the details drawer alongside or — when "full" — replacing the list entirely.
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Search from "@lucide/svelte/icons/search";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
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
</script>

{#if selected && detailFull}
  <!-- FULL-DETAIL: the list is hidden, ContainerDetails fills the pane. -->
  <div class="detail-full">
    <ContainerDetails container={selected} full={true} onClose={onCloseDetail} onToggleFull={onToggleFull} />
  </div>
{:else}
  <div class="head">
    <h1>Containers</h1>
    <Badge variant="secondary" class="gap-1.5 font-normal">
      <span class="size-1.5 rounded-full bg-[var(--ok)] shadow-[0_0_7px_var(--ok)]"></span>
      <b class="num text-foreground">{runningCount}</b> running
      <span class="text-muted-foreground">·</span>
      <b class="num text-foreground">{containers.length}</b> total
    </Badge>
    <span class="sp"></span>
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
  <div class="body" class:split={!!selected}>
    <div class="list">
      {#if errorMsg}
        <Alert.Root variant="destructive" class="mb-3.5">
          <TriangleAlert aria-hidden="true" />
          <Alert.Description>{errorMsg}</Alert.Description>
        </Alert.Root>
      {/if}
      <ContainerList {containers} {pending} onAction={onAction} onSelect={onSelect} />
    </div>
    {#if selected}
      <ContainerDetails container={selected} full={false} onClose={onCloseDetail} onToggleFull={onToggleFull} />
    {/if}
  </div>
{/if}

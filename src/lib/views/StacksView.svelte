<script lang="ts">
  // Stacks screen: compose toolbar (up/down/pull/build/logs), the streamed
  // compose-output panel, and the grouped stack list. All compose state + actions
  // live in the compose controller passed down from App.
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import FileUp from "@lucide/svelte/icons/file-up";
  import FileDown from "@lucide/svelte/icons/file-down";
  import Download from "@lucide/svelte/icons/download";
  import Hammer from "@lucide/svelte/icons/hammer";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import Terminal from "@lucide/svelte/icons/terminal";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import StackList from "./StackList.svelte";
  import type { EngineState, Stack } from "../types";
  import type { ComposeController } from "../state/compose.svelte";

  let {
    stacks,
    pending,
    engineState,
    errorMsg,
    compose,
    onStackAction,
    setFooter,
  }: {
    stacks: Stack[];
    pending: Set<string>;
    engineState: EngineState;
    errorMsg: string;
    compose: ComposeController;
    onStackAction: (action: "start" | "stop" | "restart", stack: Stack) => void;
    setFooter?: (msg: string, isError?: boolean) => void;
  } = $props();

  let disabled = $derived(compose.busy || engineState !== "running");
</script>

<div class="flex items-end gap-[14px] pt-[22px] px-[22px] pb-[16px] shrink-0">
  <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">Stacks</h1>
  <Badge variant="secondary" class="gap-1.5 font-normal">
    <b class="tabular-nums text-foreground">{stacks.length}</b>
    {stacks.length === 1 ? "project" : "projects"}
  </Badge>
  <span class="flex-1"></span>
  <Button title="Pick a docker-compose.yml and run it on the dockwin engine" {disabled} onclick={compose.up}>
    <FileUp aria-hidden="true" />Compose up
  </Button>
  <div class="flex items-center gap-[6px]">
    <Button variant="outline" title="docker compose down" {disabled} onclick={compose.down}>
      <FileDown aria-hidden="true" />Down
    </Button>
    <Button variant="outline" title="docker compose pull" {disabled} onclick={compose.pull}>
      <Download aria-hidden="true" />Pull
    </Button>
    <Button variant="outline" title="docker compose build" {disabled} onclick={compose.build}>
      <Hammer aria-hidden="true" />Build
    </Button>
    <Button variant="outline" title="docker compose logs (tail)" {disabled} onclick={compose.logs}>
      <ScrollText aria-hidden="true" />Logs
    </Button>
  </div>
</div>
<div class="flex-1 overflow-auto grid grid-cols-[1fr] min-h-0">
  <div class="flex flex-col gap-[16px] min-w-0 pt-0 px-[22px] pb-[24px]">
    {#if engineState === "running"}
      <p class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground">
        Tip: in a terminal you can also run
        <code class="font-mono text-[0.92em] text-muted-foreground">dockwin up</code> from a folder with a
        <code class="font-mono text-[0.92em] text-muted-foreground">docker-compose.yml</code> (use this instead of
        <code class="font-mono text-[0.92em] text-muted-foreground">docker compose</code>, which targets Docker Desktop).
      </p>
    {/if}
    {#if errorMsg}
      <Alert.Root variant="destructive">
        <TriangleAlert aria-hidden="true" />
        <Alert.Description>{errorMsg}</Alert.Description>
      </Alert.Root>
    {/if}
    {#if compose.panelOpen && compose.log.length}
      <div class="border border-border rounded-[9px] bg-background overflow-hidden">
        <div class="flex items-center gap-[8px] bg-muted border-b border-border px-[12px] py-[8px] text-[12px] text-muted-foreground">
          <Terminal aria-hidden="true" />
          <span class="font-semibold text-foreground">Compose output</span>
          {#if compose.lastFile}
            <span class="font-mono text-[11px] text-muted-foreground/70 truncate" title={compose.lastFile}>
              {compose.lastFile}
            </span>
          {/if}
          <Button variant="outline" size="sm" class="ml-auto" onclick={() => (compose.panelOpen = false)}>
            Hide
          </Button>
        </div>
        <div class="max-h-[14rem] overflow-auto px-[12px] py-[10px] font-mono text-[11.5px] leading-[1.55] text-muted-foreground select-text">
          {#each compose.log as line, i (i)}
            <div style="white-space:pre-wrap;word-break:break-all">{line}</div>
          {/each}
        </div>
      </div>
    {/if}
    <StackList {stacks} {pending} onStackAction={onStackAction} {setFooter} />
  </div>
</div>

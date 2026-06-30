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
  }: {
    stacks: Stack[];
    pending: Set<string>;
    engineState: EngineState;
    errorMsg: string;
    compose: ComposeController;
    onStackAction: (action: "start" | "stop" | "restart", stack: Stack) => void;
  } = $props();

  let disabled = $derived(compose.busy || engineState !== "running");
</script>

<div class="head">
  <h1>Stacks</h1>
  <Badge variant="secondary" class="gap-1.5 font-normal">
    <b class="num text-foreground">{stacks.length}</b>
    {stacks.length === 1 ? "project" : "projects"}
  </Badge>
  <span class="sp"></span>
  <Button title="Pick a docker-compose.yml and run it on the dockwin engine" {disabled} onclick={compose.up}>
    <FileUp aria-hidden="true" />
    {compose.busy ? "Working…" : "Compose up"}
  </Button>
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
<div class="body">
  <div class="page" style="padding-top:0">
    {#if engineState === "running"}
      <p class="prose">
        Tip: in a terminal you can also run
        <code class="code">dockwin up</code> from a folder with a
        <code class="code">docker-compose.yml</code> (use this instead of
        <code class="code">docker compose</code>, which targets Docker Desktop).
      </p>
    {/if}
    {#if errorMsg}
      <Alert.Root variant="destructive">
        <TriangleAlert aria-hidden="true" />
        <Alert.Description>{errorMsg}</Alert.Description>
      </Alert.Root>
    {/if}
    {#if compose.panelOpen && compose.log.length}
      <div class="outpane">
        <div class="bar">
          <Terminal aria-hidden="true" />
          <span style="font-weight:600;color:var(--text)">Compose output</span>
          {#if compose.lastFile}
            <span
              class="mono"
              style="font-size:11px;color:var(--text-4);overflow:hidden;text-overflow:ellipsis;white-space:nowrap"
              title={compose.lastFile}
            >
              {compose.lastFile}
            </span>
          {/if}
          <Button variant="outline" size="sm" style="margin-left:auto" onclick={() => (compose.panelOpen = false)}>
            Hide
          </Button>
        </div>
        <div class="body-out">
          {#each compose.log as line, i (i)}
            <div style="white-space:pre-wrap;word-break:break-all">{line}</div>
          {/each}
        </div>
      </div>
    {/if}
    <StackList {stacks} {pending} onStackAction={onStackAction} />
  </div>
</div>

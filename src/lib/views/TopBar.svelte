<script lang="ts">
  // Slim context bar above the active screen: sidebar toggle + active-view
  // breadcrumb on the left, theme/settings/refresh/engine controls on the right.
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Settings from "@lucide/svelte/icons/settings";
  import PlayCircle from "@lucide/svelte/icons/circle-play";
  import CircleStop from "@lucide/svelte/icons/circle-stop";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import type { EngineState } from "../types";

  let {
    label,
    engineState,
    settingsActive,
    engineBusy,
    working,
    engineToggleDisabled,
    onSettings,
    onRefresh,
    onToggleEngine,
  }: {
    label: string;
    engineState: EngineState;
    settingsActive: boolean;
    engineBusy: boolean;
    working: boolean;
    engineToggleDisabled: boolean;
    onSettings: () => void;
    onRefresh: () => void;
    onToggleEngine: () => void;
  } = $props();
</script>

<div class="ctx">
  <Sidebar.Trigger class="-ml-1 text-muted-foreground hover:text-foreground" />
  <span class="sep"></span>
  <span class="ctx-view">{label}</span>
  <span class="sp"></span>
  <Button
    variant={settingsActive ? "secondary" : "outline"}
    size="icon"
    title="Settings"
    onclick={onSettings}
  >
    <Settings aria-hidden="true" />
  </Button>
  <Button variant="outline" size="icon" title="Refresh" disabled={working} onclick={onRefresh}>
    <RefreshCw aria-hidden="true" />
  </Button>
  {#if engineState === "running" || engineState === "stopped"}
    <Button
      variant={engineState === "running" ? "destructive" : "success"}
      style="min-width:86px;justify-content:center"
      disabled={engineToggleDisabled}
      onclick={onToggleEngine}
    >
      {#if engineState === "running"}
        <CircleStop aria-hidden="true" />{engineBusy ? "Stopping…" : "Stop"}
      {:else}
        <PlayCircle aria-hidden="true" />{engineBusy ? "Starting…" : "Start"}
      {/if}
    </Button>
  {/if}
</div>

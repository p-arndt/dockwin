<script lang="ts">
  // Root shell. Owns the engine lifecycle + container data/actions/polling via the
  // shared AppController (published on context so every route reads live state),
  // and renders either the full-window EngineGate (engine not running) or the
  // sidebar management shell with the active route in {@render children()}.
  // Talks to dockwin-core only through $lib/api (no raw invoke).
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import Container from "@lucide/svelte/icons/container";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import AppSidebar, { type View } from "$lib/components/AppSidebar.svelte";
  import ThemeToggle from "$lib/components/ThemeToggle.svelte";
  import ConfirmHost from "$lib/components/ConfirmHost.svelte";
  import EngineGate from "$lib/views/EngineGate.svelte";
  import TopBar from "$lib/views/TopBar.svelte";
  import UpdateBanner from "$lib/views/UpdateBanner.svelte";
  import { setAppController } from "$lib/state/app.svelte";

  let { children } = $props();

  const app = setAppController();

  // The active screen is derived straight from the URL. "/" redirects to
  // /containers (see +page.ts), so the empty default is only transient.
  let activeView = $derived<View>((page.url.pathname.split("/")[1] || "containers") as View);

  function navigate(view: View) {
    goto(`/${view}`);
  }

  onMount(() => app.mount());
</script>

{#if !app.engineReady}
  <!-- Engine lifecycle: a slim branded bar (so theme is still toggleable) + the
       full-window EngineGate, which owns set-up / progress / start / unreachable. -->
  <div class="solo">
    <div class="ctx">
      <span class="brand" style="padding:0;gap:9px">
        <span class="logo" style="width:26px;height:26px;border-radius:8px">
          <Container size={15} aria-hidden="true" />
        </span>
        <span class="bt" style="font-size:14px">dockwin</span>
      </span>
      <span class="sep"></span>
      <span class="live {app.engineTone}"><span class="d"></span>{app.engineLine}</span>
      <span class="sp"></span>
      <ThemeToggle />
      <span class="sep"></span>
      <Button variant="outline" size="icon" title="Refresh" disabled={app.working} onclick={app.manualRefresh}>
        <RefreshCw aria-hidden="true" />
      </Button>
    </div>
    <EngineGate
      engineState={app.engineState}
      working={app.working}
      provision={app.provision}
      engineBusy={app.engineBusy}
      repairing={app.repairing}
      bind:enableTcp={app.enableTcp}
      onProvision={() => app.provisionEngine()}
      onStart={() => app.toggleEngine()}
      onRepair={() => app.repairEngine()}
      onRetry={app.manualRefresh}
    />
  </div>
{:else}
  <Sidebar.Provider class="h-screen overflow-hidden">
    <AppSidebar
      {activeView}
      counts={app.navCounts}
      engineTone={app.engineTone}
      engineLine={app.engineLine}
      onSelect={navigate}
    />

    <Sidebar.Inset class="main">
      <TopBar
        label={app.viewLabel(activeView)}
        engineState={app.engineState}
        settingsActive={activeView === "settings"}
        engineBusy={app.engineBusy}
        working={app.working}
        engineToggleDisabled={app.engineToggleDisabled}
        onSettings={() => navigate("settings")}
        onRefresh={app.manualRefresh}
        onToggleEngine={() => app.toggleEngine()}
      />

      {@render children()}

      <div class="statusbar" class:err={app.footerErr}>{app.footer}</div>
    </Sidebar.Inset>
  </Sidebar.Provider>
{/if}

<!-- In-app update toast (app + engine). Fixed-position; checks on mount. -->
<UpdateBanner />

<!-- Single mounted host for the shared confirm dialog (confirmDialog()). -->
<ConfirmHost />

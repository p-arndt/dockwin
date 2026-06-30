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
  import StatusDot from "$lib/components/StatusDot.svelte";
  import EngineGate from "$lib/views/EngineGate.svelte";
  import TopBar from "$lib/views/TopBar.svelte";
  import UpdateBanner from "$lib/views/UpdateBanner.svelte";
  import { setAppController } from "$lib/state/app.svelte";
  import { getAppVersion } from "$lib/api/updater";

  let { children } = $props();

  const app = setAppController();

  // App version shown in the status bar. Read once from the bundle on mount;
  // stays null (and hidden) in plain-browser dev where Tauri isn't present.
  let appVersion = $state<string | null>(null);

  // The active screen is derived straight from the URL. "/" redirects to
  // /containers (see +page.ts), so the empty default is only transient.
  let activeView = $derived<View>((page.url.pathname.split("/")[1] || "containers") as View);

  function navigate(view: View) {
    goto(`/${view}`);
  }

  onMount(() => {
    app.mount();
    getAppVersion().then((v) => (appVersion = v));
    // Hand off from the static app.html splash to the real shell now that
    // Svelte has committed its first paint.
    const splash = document.getElementById("initial-splash");
    if (splash) {
      splash.classList.add("is-hidden");
      splash.addEventListener("transitionend", () => splash.remove(), { once: true });
    }
  });
</script>

{#if !app.engineReady}
  <!-- Engine lifecycle: a slim branded bar (so theme is still toggleable) + the
       full-window EngineGate, which owns set-up / progress / start / unreachable. -->
  <div class="flex flex-col h-screen min-h-0">
    <div
      class="flex items-center gap-[14px] pl-[22px] pr-[16px] h-[50px] shrink-0 border-b border-border text-muted-foreground text-[12px]"
    >
      <span class="flex items-center gap-[9px] p-0">
        <span
          class="grid place-items-center shrink-0 size-[26px] rounded-[8px] bg-primary text-primary-foreground"
        >
          <Container size={19} aria-hidden="true" />
        </span>
        <span class="font-[680] text-[14px] tracking-[-0.2px] leading-[1.1]">dockwin</span>
      </span>
      <span class="w-px h-[13px] bg-border"></span>
      <span class="flex items-center gap-[7px] text-muted-foreground font-medium"
        ><StatusDot
          tone={app.engineTone === "warn" ? "warn" : app.engineTone === "off" ? "off" : "run"}
          halo={app.engineTone === "live"}
          size={6}
        />{app.engineLine}</span
      >
      <span class="flex-1"></span>
      <ThemeToggle />
      <span class="w-px h-[13px] bg-border"></span>
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
      settingsActive={activeView === "settings"}
      onSelect={navigate}
      onSettings={() => navigate("settings")}
    />

    <Sidebar.Inset class="flex flex-col min-w-0 overflow-hidden">
      <TopBar
        label={app.viewLabel(activeView)}
        working={app.working}
        onRefresh={app.manualRefresh}
      />

      {@render children()}

      <div
        class="shrink-0 flex items-center gap-[14px] border-t border-border bg-card py-[6px] px-[22px] text-[11.5px] select-text {app.footerErr
          ? 'text-destructive'
          : 'text-muted-foreground'}"
      >
        <span class="min-w-0 truncate">{app.footer}</span>
        <span class="flex-1"></span>
        {#if appVersion}
          <span class="shrink-0 tabular-nums text-muted-foreground" title="dockwin app version">
            v{appVersion}
          </span>
          <span class="w-px h-[11px] bg-border shrink-0"></span>
        {/if}
        <button
          type="button"
          onclick={() => navigate("settings")}
          title="Engine settings"
          class="flex items-center gap-[7px] shrink-0 rounded-[6px] -mr-1 px-[6px] py-[2px] font-medium text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
        >
          <StatusDot
            tone={app.engineTone === "warn" ? "warn" : app.engineTone === "off" ? "off" : "run"}
            halo={app.engineTone === "live"}
            size={6}
          />
          {app.engineLine}
        </button>
      </div>
    </Sidebar.Inset>
  </Sidebar.Provider>
{/if}

<!-- In-app update toast (app + engine). Fixed-position; checks on mount. -->
<UpdateBanner />

<!-- Single mounted host for the shared confirm dialog (confirmDialog()). -->
<ConfirmHost />

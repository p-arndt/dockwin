<script lang="ts">
  // Self-contained "update available" toast. Mount once near the app root:
  //   <UpdateBanner />
  // On launch it checks (a) for a newer signed dockwin app release and (b) for a
  // newer in-distro Docker Engine, and shows a dismissible card if either has an
  // update. Notify-only: nothing installs until the user clicks. Needs no props.
  import { onMount } from "svelte";
  import Download from "@lucide/svelte/icons/download";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import X from "@lucide/svelte/icons/x";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as api from "../api";
  import {
    checkAppUpdate,
    installAppUpdate,
    type AppUpdateInfo,
  } from "../api/updater";

  // --- app (GUI) update state ---
  let appUpdate = $state<AppUpdateInfo | null>(null);
  let appInstalling = $state(false);
  let appPct = $state(0); // 0..100 download progress (0 when total unknown)

  // --- engine (Docker) update state ---
  let engineInstalled = $state<string | null>(null);
  let engineCandidate = $state<string | null>(null);
  let engineAvailable = $state(false);
  let engineUpdating = $state(false);
  let enginePct = $state(0);
  let engineMsg = $state("");
  // True right after a successful engine update, until dismissed or the next
  // check — keeps the success line on screen instead of vanishing the instant
  // engineAvailable flips to false.
  let engineJustUpdated = $state(false);

  let errorMsg = $state("");
  let dismissed = $state(false);

  // Show the card only when there's something to offer and it isn't dismissed.
  let visible = $derived(
    !dismissed && (appUpdate !== null || engineAvailable || engineJustUpdated)
  );

  // Trim the apt epoch/suffix noise to a friendly "27.3.1" for display.
  function shortDocker(v: string | null): string {
    if (!v) return "";
    const m = /(\d+\.\d+\.\d+)/.exec(v);
    return m ? m[1] : v;
  }

  async function refreshAppUpdate() {
    try {
      appUpdate = await checkAppUpdate();
    } catch {
      // Endpoint unreachable / offline — stay quiet.
      appUpdate = null;
    }
  }

  async function refreshEngineUpdate() {
    try {
      const u = await api.engineUpdateCheck();
      engineInstalled = u.installed;
      engineCandidate = u.candidate;
      engineAvailable = u.update_available;
      if (engineAvailable) engineJustUpdated = false;
    } catch {
      engineAvailable = false;
    }
  }

  async function doInstallApp() {
    if (!appUpdate || appInstalling) return;
    appInstalling = true;
    appPct = 0;
    errorMsg = "";
    try {
      await installAppUpdate(appUpdate, (p) => {
        appPct = p.total ? Math.round((p.downloaded / p.total) * 100) : 0;
      });
      // installAppUpdate relaunches on success, so we won't reach here normally.
    } catch (e) {
      errorMsg = `App update failed: ${api.errText(e)}`;
      appInstalling = false;
    }
  }

  async function doUpdateEngine() {
    if (engineUpdating) return;
    engineUpdating = true;
    enginePct = 0;
    engineMsg = "Starting…";
    errorMsg = "";
    engineJustUpdated = false;
    try {
      await api.engineUpdate();
      // Reflect the new installed version in the card.
      engineInstalled = engineCandidate;
      engineAvailable = false;
      engineJustUpdated = true;
    } catch (e) {
      errorMsg = `Engine update failed: ${api.errText(e)}`;
    } finally {
      engineUpdating = false;
    }
  }

  onMount(() => {
    let unlisten: (() => void) | null = null;

    // Live engine-update progress bar + last message.
    api
      .onEngineUpdateProgress((p) => {
        enginePct = Math.min(100, Math.max(enginePct, p.pct));
        engineMsg = p.message;
        if (p.done && p.error) errorMsg = p.error;
      })
      .then((u) => (unlisten = u))
      .catch(() => {});

    // Kick off both checks (independent; either may surface an update).
    refreshAppUpdate();
    refreshEngineUpdate();

    return () => {
      try {
        unlisten?.();
      } catch {
        /* ignore */
      }
    };
  });
</script>

{#if visible}
  <Card.Root
    class="fixed bottom-9 right-4 z-50 w-[340px] select-text gap-0 py-0"
    role="status"
  >
    <div class="flex items-center gap-2 border-b border-border px-3.5 py-2.5">
      <Download size={15} class="text-muted-foreground" aria-hidden="true" />
      <span class="text-[13px] font-semibold">Updates available</span>
      <Button
        variant="ghost"
        size="icon-sm"
        class="ml-auto"
        title="Dismiss"
        aria-label="Dismiss"
        onclick={() => (dismissed = true)}
      >
        <X size={14} aria-hidden="true" />
      </Button>
    </div>

    <div class="flex flex-col gap-3 px-3.5 py-3">
      {#if errorMsg}
        <div
          class="rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-1.5 text-[12px] text-destructive"
        >
          {errorMsg}
        </div>
      {/if}

      <!-- App (GUI) update -->
      {#if appUpdate}
        <div class="flex flex-col gap-1.5">
          <div class="text-[12.5px] text-muted-foreground">
            <span class="font-semibold text-foreground">dockwin {appUpdate.version}</span>
            <span class="text-muted-foreground"> · you have {appUpdate.currentVersion}</span>
          </div>
          {#if appInstalling}
            <div class="h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                class="h-full bg-foreground/70 transition-[width] duration-150"
                style="width: {appPct}%"
              ></div>
            </div>
            <span class="text-[11px] text-muted-foreground">
              {appPct ? `Downloading… ${appPct}%` : "Downloading…"}
            </span>
          {:else}
            <Button size="sm" class="w-full" onclick={doInstallApp}>
              <Download size={14} aria-hidden="true" /> Install &amp; restart
            </Button>
          {/if}
        </div>
      {/if}

      {#if appUpdate && (engineAvailable || engineJustUpdated)}
        <div class="h-px bg-border"></div>
      {/if}

      <!-- Engine (Docker) update -->
      {#if engineAvailable || engineJustUpdated}
        <div class="flex flex-col gap-1.5">
          {#if engineJustUpdated && !engineUpdating}
            <div class="flex items-center gap-1.5 text-[12.5px] text-muted-foreground">
              <CircleCheck size={14} class="text-chart-2" aria-hidden="true" />
              <span class="font-semibold text-foreground">Docker Engine {shortDocker(engineInstalled)}</span>
              <span class="text-muted-foreground">up to date</span>
            </div>
          {:else}
            <div class="text-[12.5px] text-muted-foreground">
              <span class="font-semibold text-foreground">
                Docker Engine {shortDocker(engineCandidate)}
              </span>
              <span class="text-muted-foreground">
                · running {shortDocker(engineInstalled)}
              </span>
            </div>
          {/if}
          {#if engineUpdating}
            <div class="h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                class="h-full bg-foreground/70 transition-[width] duration-150"
                style="width: {enginePct}%"
              ></div>
            </div>
            <span class="truncate font-mono text-[11px] text-muted-foreground" title={engineMsg}>
              {engineMsg}
            </span>
          {:else if !engineJustUpdated}
            <Button variant="outline" size="sm" class="w-full" onclick={doUpdateEngine}>
              <RefreshCw size={14} aria-hidden="true" /> Update engine
            </Button>
          {/if}
        </div>
      {/if}
    </div>
  </Card.Root>
{/if}

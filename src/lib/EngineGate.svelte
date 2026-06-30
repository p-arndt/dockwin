<script lang="ts">
  // Full-window gate shown whenever the engine isn't running. It owns the entire
  // engine lifecycle (set up / provisioning progress / start / unreachable) so
  // none of that leaks into the management shell. The sidebar + resource views
  // only mount once the engine is running.
  import DownloadCloud from "@lucide/svelte/icons/download-cloud";
  import CircleStop from "@lucide/svelte/icons/circle-stop";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import HelpCircle from "@lucide/svelte/icons/circle-help";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import PlayCircle from "@lucide/svelte/icons/circle-play";
  import Hammer from "@lucide/svelte/icons/hammer";
  import type { EngineState, ProvisionUi } from "./types";

  let {
    engineState,
    working,
    provision,
    engineBusy,
    repairing,
    enableTcp = $bindable(),
    onProvision,
    onStart,
    onRepair,
    onRetry,
  }: {
    engineState: EngineState;
    working: boolean;
    provision: ProvisionUi | null;
    engineBusy: boolean;
    repairing: boolean;
    enableTcp: boolean;
    onProvision: () => void;
    onStart: () => void;
    onRepair: () => void;
    onRetry: () => void;
  } = $props();
</script>

<div class="flex min-h-0 flex-1 items-center justify-center overflow-auto p-6">
  <div class="w-full max-w-lg">
    {#if working && provision}
      <!-- Live provisioning progress. -->
      <section class="rounded-lg border border-[#262b34] bg-[#171a21] p-6">
        <div class="mb-3 flex items-center gap-2.5">
          <DownloadCloud size={22} class="text-[#2f81f7]" aria-hidden="true" />
          <h2 class="text-lg font-semibold">Setting up the dockwin engine…</h2>
          <span class="ml-auto font-mono-app text-sm text-[#9aa3af]">
            {Math.round(provision.pct)}%
          </span>
        </div>
        <div class="h-2.5 w-full overflow-hidden rounded-full bg-[#21262d]">
          <div
            class="provision-bar h-full rounded-full bg-[#1f6feb] transition-[width] duration-300 ease-out"
            style="width: {Math.max(2, provision.pct)}%"
          ></div>
        </div>
        <p class="mt-3 truncate text-[13px] text-[#c7ccd4]" title={provision.message}>
          {provision.message}
        </p>
        {#if provision.log.length}
          <div
            class="mt-3 max-h-44 select-text overflow-auto rounded-md border border-[#262b34] bg-[#0d1117] p-2.5 font-mono-app text-[11.5px] leading-relaxed text-[#9aa3af]"
          >
            {#each provision.log.slice(-12) as line, i (i)}
              <div class="whitespace-pre-wrap break-all">{line}</div>
            {/each}
          </div>
        {/if}
        <p class="mt-3 text-xs text-[#6e7681]">
          Downloading the Ubuntu image and installing Docker. You can keep this
          window open — it'll finish on its own.
        </p>
      </section>
    {:else if engineState === "not-provisioned"}
      <!-- First-run setup. -->
      <section class="rounded-lg border border-[#262b34] bg-[#171a21] p-6">
        <div class="mb-2 flex items-center gap-2.5">
          <DownloadCloud size={22} class="text-[#2f81f7]" aria-hidden="true" />
          <h2 class="text-lg font-semibold">Set up the dockwin engine</h2>
        </div>
        <p class="mb-4 text-[13px] leading-relaxed text-[#9aa3af]">
          dockwin runs Docker in a dedicated, isolated WSL2 distro — no Docker
          Desktop required. Setting up downloads a minimal Ubuntu image
          (~250&nbsp;MB) and installs the Docker Engine. This can take a few
          minutes; you can keep this window open.
        </p>
        <label class="mb-4 flex items-center gap-2 text-[13px] text-[#c7ccd4]">
          <input type="checkbox" bind:checked={enableTcp} disabled={working} />
          Also enable insecure loopback TCP (127.0.0.1:2375) — not recommended
        </label>
        <button
          class="flex items-center gap-2 rounded-md border border-[#2f81f7] bg-[#1f6feb] px-4 py-2 text-sm font-medium text-white transition-colors hover:not-disabled:bg-[#2f81f7] disabled:cursor-default disabled:opacity-60"
          disabled={working}
          onclick={onProvision}
        >
          <DownloadCloud size={16} aria-hidden="true" />
          {working ? "Setting up…" : "Set up engine"}
        </button>
      </section>
    {:else if engineState === "stopped"}
      <!-- Engine provisioned but not running. -->
      <section class="rounded-lg border border-[#262b34] bg-[#171a21] p-6">
        <div class="mb-2 flex items-center gap-2.5">
          <CircleStop size={22} class="text-[#9aa3af]" aria-hidden="true" />
          <h2 class="text-lg font-semibold">Engine is stopped</h2>
        </div>
        <p class="mb-4 text-[13px] leading-relaxed text-[#9aa3af]">
          Start the dockwin engine to manage your containers, images, volumes and
          networks.
        </p>
        <button
          class="flex items-center gap-2 rounded-md border border-[#2f81f7] bg-[#1f6feb] px-4 py-2 text-sm font-medium text-white transition-colors hover:not-disabled:bg-[#2f81f7] disabled:cursor-default disabled:opacity-60"
          disabled={engineBusy || working}
          onclick={onStart}
        >
          <PlayCircle size={16} aria-hidden="true" />
          {engineBusy ? "Starting…" : "Start engine"}
        </button>
      </section>
    {:else if engineState === "broken"}
      <!-- Broken engine: distro registered but its disk image is missing. -->
      <section class="rounded-lg border border-[#f8514966] bg-[#f851491a] p-6">
        <div class="mb-2 flex items-center gap-2.5">
          <TriangleAlert size={22} class="text-[#f85149]" aria-hidden="true" />
          <h2 class="text-lg font-semibold text-[#ff9b95]">Engine is broken</h2>
        </div>
        <p class="mb-4 text-[13px] leading-relaxed text-[#ffb3ae]">
          The dockwin WSL distro is registered but its disk image is missing.
          Reset it to unregister the dangling distro, then set the engine up again
          to reprovision.
        </p>
        <button
          class="flex items-center gap-2 rounded-md border border-[#f8514966] bg-[#f851491a] px-4 py-2 text-sm font-medium text-[#ff9b95] transition-colors hover:not-disabled:bg-[#f8514926] disabled:cursor-default disabled:opacity-60"
          disabled={repairing || working}
          onclick={onRepair}
        >
          <Hammer size={16} aria-hidden="true" />
          {repairing ? "Resetting…" : "Repair engine"}
        </button>
      </section>
    {:else}
      <!-- unknown: can't yet determine the engine state. -->
      <section class="rounded-lg border border-[#262b34] bg-[#171a21] p-6">
        <div class="mb-2 flex items-center gap-2.5">
          <HelpCircle size={22} class="text-[#9aa3af]" aria-hidden="true" />
          <h2 class="text-lg font-semibold">Checking engine…</h2>
        </div>
        <p class="mb-4 text-[13px] leading-relaxed text-[#9aa3af]">
          Couldn't determine the engine state. Retry to check again.
        </p>
        <button
          class="flex items-center gap-2 rounded-md border border-[#262b34] bg-[#21262d] px-4 py-2 text-sm font-medium text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-60"
          disabled={working}
          onclick={onRetry}
        >
          <RefreshCw size={16} aria-hidden="true" />
          Retry
        </button>
      </section>
    {/if}
  </div>
</div>

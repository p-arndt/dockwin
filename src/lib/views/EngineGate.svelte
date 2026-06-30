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
  import Terminal from "@lucide/svelte/icons/terminal";
  import StatusDot from "../components/StatusDot.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import type { EngineState, ProvisionUi } from "../types";

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

<div
  class="flex flex-1 min-h-0 items-center justify-center overflow-auto p-[24px]"
>
  {#if working && provision}
    <!-- Live provisioning progress. -->
    <section
      class="w-full max-w-[32rem] bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] p-[24px]"
    >
      <div class="flex items-center gap-[13px]">
        <span
          class="grid place-items-center shrink-0 size-[40px] rounded-[9px] bg-muted border border-border text-muted-foreground"
        >
          <DownloadCloud class="size-[20px]" aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <h2
            class="text-[17px] font-[680] tracking-[-0.3px] leading-[1.2] text-foreground"
          >
            Setting up the dockwin engine
          </h2>
          <div class="flex items-center gap-[7px] mt-[4px] text-[12px]">
            <StatusDot tone="warn" halo />
            <span class="font-semibold text-muted-foreground">Provisioning</span>
            <span class="text-muted-foreground/70">{provision.phase || "working"}</span>
          </div>
        </div>
        <span
          class="ml-auto text-[13px] font-semibold text-muted-foreground font-mono tabular-nums"
          >{Math.round(provision.pct)}%</span
        >
      </div>

      <div
        class="relative h-[10px] w-full rounded-full bg-muted overflow-hidden mt-[16px]"
      >
        <i class="fill" style="width:{Math.max(2, provision.pct)}%"></i>
      </div>

      <p
        class="mt-[12px] text-[12.5px] text-muted-foreground truncate"
        title={provision.message}
      >
        {provision.message}
      </p>

      {#if provision.log.length}
        <div
          class="border border-border rounded-[9px] bg-background overflow-hidden mt-[13px]"
        >
          <div
            class="flex items-center gap-[8px] bg-muted border-b border-border px-[12px] py-[8px] text-[12px] text-muted-foreground"
          >
            <Terminal aria-hidden="true" />
            <span>Setup log</span>
          </div>
          <div
            class="max-h-[14rem] overflow-auto px-[12px] py-[10px] font-mono text-[11.5px] leading-[1.55] text-muted-foreground select-text"
          >
            {#each provision.log.slice(-12) as line, i (i)}
              <div class="whitespace-pre-wrap break-all">{line}</div>
            {/each}
          </div>
        </div>
      {/if}

      <p
        class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground mt-[13px]"
      >
        Downloading the Ubuntu image and installing Docker. You can keep this
        window open — it'll finish on its own.
      </p>
    </section>
  {:else if engineState === "not-provisioned"}
    <!-- First-run setup. -->
    <section
      class="w-full max-w-[32rem] bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] p-[24px]"
    >
      <div class="flex items-center gap-[13px]">
        <span
          class="grid place-items-center shrink-0 size-[40px] rounded-[9px] bg-muted border border-border text-muted-foreground"
        >
          <DownloadCloud class="size-[20px]" aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <h2
            class="text-[17px] font-[680] tracking-[-0.3px] leading-[1.2] text-foreground"
          >
            Set up the dockwin engine
          </h2>
          <div class="flex items-center gap-[7px] mt-[4px] text-[12px]">
            <StatusDot tone="off" />
            <span class="font-semibold text-muted-foreground">Not set up</span>
            <span class="text-muted-foreground/70">no distro registered</span>
          </div>
        </div>
      </div>

      <p
        class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground mt-[13px]"
      >
        dockwin runs Docker in a dedicated, isolated WSL2 distro — no Docker
        Desktop required. Setting up downloads a minimal Ubuntu image
        (~250&nbsp;MB) and installs the Docker Engine. This can take a few
        minutes; you can keep this window open.
      </p>

      <div
        class="flex items-center gap-[9px] text-[13px] text-muted-foreground mt-[16px]"
      >
        <Checkbox id="enable-tcp" bind:checked={enableTcp} disabled={working} />
        <Label for="enable-tcp">
          Also enable insecure loopback TCP (127.0.0.1:2375) — not recommended
        </Label>
      </div>

      <div class="flex mt-[18px]">
        <Button disabled={working} onclick={onProvision}>
          <DownloadCloud aria-hidden="true" />
          {working ? "Setting up…" : "Set up engine"}
        </Button>
      </div>
    </section>
  {:else if engineState === "stopped"}
    <!-- Engine provisioned but not running. -->
    <section
      class="w-full max-w-[32rem] bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] p-[24px]"
    >
      <div class="flex items-center gap-[13px]">
        <span
          class="grid place-items-center shrink-0 size-[40px] rounded-[9px] bg-muted border border-border text-muted-foreground"
        >
          <CircleStop class="size-[20px]" aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <h2
            class="text-[17px] font-[680] tracking-[-0.3px] leading-[1.2] text-foreground"
          >
            Engine is stopped
          </h2>
          <div class="flex items-center gap-[7px] mt-[4px] text-[12px]">
            <StatusDot tone="off" />
            <span class="font-semibold text-muted-foreground">Stopped</span>
            <span class="text-muted-foreground/70">provisioned · not running</span>
          </div>
        </div>
      </div>

      <p
        class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground mt-[13px]"
      >
        Start the dockwin engine to manage your containers, images, volumes and
        networks.
      </p>

      <div class="flex mt-[18px]">
        <Button disabled={engineBusy || working} onclick={onStart}>
          <PlayCircle aria-hidden="true" />
          {engineBusy ? "Starting…" : "Start engine"}
        </Button>
      </div>
    </section>
  {:else if engineState === "broken"}
    <!-- Broken engine: distro registered but its disk image is missing. -->
    <section
      class="w-full max-w-[32rem] bg-destructive/15 border border-destructive/40 rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] p-[24px]"
    >
      <div class="flex items-center gap-[13px]">
        <span
          class="grid place-items-center shrink-0 size-[40px] rounded-[9px] bg-muted border border-border text-muted-foreground"
        >
          <TriangleAlert class="size-[20px]" aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <h2
            class="text-[17px] font-[680] tracking-[-0.3px] leading-[1.2] text-destructive"
          >
            Engine is broken
          </h2>
          <div class="flex items-center gap-[7px] mt-[4px] text-[12px]">
            <StatusDot tone="err" halo />
            <span class="font-semibold text-muted-foreground">Broken</span>
            <span class="text-muted-foreground/70">disk image missing</span>
          </div>
        </div>
      </div>

      <Alert.Root variant="destructive" class="mt-[13px]">
        <TriangleAlert aria-hidden="true" />
        <Alert.Description>
          The dockwin WSL distro is registered but its disk image is missing.
          Reset it to unregister the dangling distro, then set the engine up
          again to reprovision.
        </Alert.Description>
      </Alert.Root>

      <div class="flex mt-[18px]">
        <Button
          variant="destructive"
          disabled={repairing || working}
          onclick={onRepair}
        >
          <Hammer aria-hidden="true" />
          {repairing ? "Resetting…" : "Repair engine"}
        </Button>
      </div>
    </section>
  {:else}
    <!-- unknown: can't yet determine the engine state. -->
    <section
      class="w-full max-w-[32rem] bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] p-[24px]"
    >
      <div class="flex items-center gap-[13px]">
        <span
          class="grid place-items-center shrink-0 size-[40px] rounded-[9px] bg-muted border border-border text-muted-foreground"
        >
          <HelpCircle class="size-[20px]" aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <h2
            class="text-[17px] font-[680] tracking-[-0.3px] leading-[1.2] text-foreground"
          >
            Checking engine
          </h2>
          <div class="flex items-center gap-[7px] mt-[4px] text-[12px]">
            <StatusDot tone="off" />
            <span class="font-semibold text-muted-foreground">Unknown</span>
            <span class="text-muted-foreground/70">state undetermined</span>
          </div>
        </div>
      </div>

      <p
        class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground mt-[13px]"
      >
        Couldn't determine the engine state. Retry to check again.
      </p>

      <div class="flex mt-[18px]">
        <Button variant="outline" disabled={working} onclick={onRetry}>
          <RefreshCw aria-hidden="true" />
          Retry
        </Button>
      </div>
    </section>
  {/if}
</div>

<style>
  /* Progress-bar fill: solid primary with an animated sheen sweeping across it.
     The keyframe sheen can't be expressed as an inline Tailwind utility, so it
     lives here as a single local class. */
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 9999px;
    background-color: var(--primary);
    background-image: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, #fff 28%, transparent),
      transparent
    );
    background-size: 200% 100%;
    animation: progress-sheen 1.6s linear infinite;
    transition: width 0.3s ease-out;
  }
  @keyframes progress-sheen {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }
</style>

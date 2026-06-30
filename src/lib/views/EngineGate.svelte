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

<div class="gate">
  {#if working && provision}
    <!-- Live provisioning progress. -->
    <section class="gate-card">
      <div class="g-head">
        <span class="av"><DownloadCloud aria-hidden="true" /></span>
        <div class="g-head-text">
          <h2 class="g-title">Setting up the dockwin engine</h2>
          <div class="g-stat">
            <StatusDot tone="warn" halo />
            <span class="g-word">Provisioning</span>
            <span class="g-sub">{provision.phase || "working"}</span>
          </div>
        </div>
        <span class="g-pct mono num">{Math.round(provision.pct)}%</span>
      </div>

      <div class="progress" style="margin-top:16px">
        <i style="width:{Math.max(2, provision.pct)}%"></i>
      </div>

      <p class="g-msg" title={provision.message}>{provision.message}</p>

      {#if provision.log.length}
        <div class="outpane" style="margin-top:14px">
          <div class="bar">
            <Terminal aria-hidden="true" />
            <span>Setup log</span>
          </div>
          <div class="body-out">
            {#each provision.log.slice(-12) as line, i (i)}
              <div class="g-logline">{line}</div>
            {/each}
          </div>
        </div>
      {/if}

      <p class="prose" style="margin-top:14px">
        Downloading the Ubuntu image and installing Docker. You can keep this
        window open — it'll finish on its own.
      </p>
    </section>
  {:else if engineState === "not-provisioned"}
    <!-- First-run setup. -->
    <section class="gate-card">
      <div class="g-head">
        <span class="av"><DownloadCloud aria-hidden="true" /></span>
        <div class="g-head-text">
          <h2 class="g-title">Set up the dockwin engine</h2>
          <div class="g-stat">
            <StatusDot tone="off" />
            <span class="g-word">Not set up</span>
            <span class="g-sub">no distro registered</span>
          </div>
        </div>
      </div>

      <p class="prose" style="margin-top:14px">
        dockwin runs Docker in a dedicated, isolated WSL2 distro — no Docker
        Desktop required. Setting up downloads a minimal Ubuntu image
        (~250&nbsp;MB) and installs the Docker Engine. This can take a few
        minutes; you can keep this window open.
      </p>

      <div class="field" style="margin-top:16px">
        <Checkbox id="enable-tcp" bind:checked={enableTcp} disabled={working} />
        <Label for="enable-tcp">
          Also enable insecure loopback TCP (127.0.0.1:2375) — not recommended
        </Label>
      </div>

      <div class="g-acts">
        <Button disabled={working} onclick={onProvision}>
          <DownloadCloud aria-hidden="true" />
          {working ? "Setting up…" : "Set up engine"}
        </Button>
      </div>
    </section>
  {:else if engineState === "stopped"}
    <!-- Engine provisioned but not running. -->
    <section class="gate-card">
      <div class="g-head">
        <span class="av"><CircleStop aria-hidden="true" /></span>
        <div class="g-head-text">
          <h2 class="g-title">Engine is stopped</h2>
          <div class="g-stat">
            <StatusDot tone="off" />
            <span class="g-word">Stopped</span>
            <span class="g-sub">provisioned · not running</span>
          </div>
        </div>
      </div>

      <p class="prose" style="margin-top:14px">
        Start the dockwin engine to manage your containers, images, volumes and
        networks.
      </p>

      <div class="g-acts">
        <Button disabled={engineBusy || working} onclick={onStart}>
          <PlayCircle aria-hidden="true" />
          {engineBusy ? "Starting…" : "Start engine"}
        </Button>
      </div>
    </section>
  {:else if engineState === "broken"}
    <!-- Broken engine: distro registered but its disk image is missing. -->
    <section class="gate-card err">
      <div class="g-head">
        <span class="av"><TriangleAlert aria-hidden="true" /></span>
        <div class="g-head-text">
          <h2 class="g-title">Engine is broken</h2>
          <div class="g-stat">
            <StatusDot tone="err" halo />
            <span class="g-word">Broken</span>
            <span class="g-sub">disk image missing</span>
          </div>
        </div>
      </div>

      <Alert.Root variant="destructive" class="mt-[14px]">
        <TriangleAlert aria-hidden="true" />
        <Alert.Description>
          The dockwin WSL distro is registered but its disk image is missing.
          Reset it to unregister the dangling distro, then set the engine up
          again to reprovision.
        </Alert.Description>
      </Alert.Root>

      <div class="g-acts">
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
    <section class="gate-card">
      <div class="g-head">
        <span class="av"><HelpCircle aria-hidden="true" /></span>
        <div class="g-head-text">
          <h2 class="g-title">Checking engine</h2>
          <div class="g-stat">
            <StatusDot tone="off" />
            <span class="g-word">Unknown</span>
            <span class="g-sub">state undetermined</span>
          </div>
        </div>
      </div>

      <p class="prose" style="margin-top:14px">
        Couldn't determine the engine state. Retry to check again.
      </p>

      <div class="g-acts">
        <Button variant="outline" disabled={working} onclick={onRetry}>
          <RefreshCw aria-hidden="true" />
          Retry
        </Button>
      </div>
    </section>
  {/if}
</div>

<style>
  .g-head {
    display: flex;
    align-items: center;
    gap: 13px;
  }
  .av {
    width: 40px;
    height: 40px;
    border-radius: var(--r);
  }
  .av :global(svg) {
    width: 20px;
    height: 20px;
  }
  .g-head-text {
    min-width: 0;
  }
  .g-title {
    font-size: 17px;
    font-weight: 680;
    letter-spacing: -0.3px;
    line-height: 1.2;
    color: var(--text);
  }
  .g-stat {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 4px;
    font-size: 12px;
  }
  .g-word {
    font-weight: 600;
    color: var(--text-2);
  }
  .g-sub {
    color: var(--text-4);
  }
  .gate-card.err .g-title {
    color: var(--err);
  }
  .g-pct {
    margin-left: auto;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }
  .g-msg {
    margin-top: 12px;
    font-size: 12.5px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .g-logline {
    white-space: pre-wrap;
    word-break: break-all;
  }
  .g-acts {
    display: flex;
    margin-top: 18px;
  }
</style>

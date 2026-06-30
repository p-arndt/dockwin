<script lang="ts">
  // Settings screen: appearance (theme), engine control (start/stop/restart) and
  // engine teardown (danger zone).
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import CircleStop from "@lucide/svelte/icons/circle-stop";
  import PlayCircle from "@lucide/svelte/icons/circle-play";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import ThemeToggle from "../components/ThemeToggle.svelte";
  import StatusDot from "../components/StatusDot.svelte";
  import type { EngineState } from "../types";

  let {
    working,
    withBackup = $bindable(),
    engineState,
    engineLine,
    engineTone,
    engineBusy,
    engineToggleDisabled,
    onToggleEngine,
    onRestartEngine,
    onTeardown,
  }: {
    working: boolean;
    withBackup: boolean;
    engineState: EngineState;
    engineLine: string;
    engineTone: string;
    engineBusy: boolean;
    engineToggleDisabled: boolean;
    onToggleEngine: () => void;
    onRestartEngine: () => void;
    onTeardown: () => void;
  } = $props();
</script>

<div class="flex items-end gap-[14px] px-[22px] pt-[22px] pb-[16px] shrink-0">
  <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">Settings</h1>
</div>
<div class="flex-1 overflow-auto grid grid-cols-[1fr] min-h-0">
  <div class="px-[22px] pt-[18px] pb-[24px] min-w-0 flex flex-col gap-[16px]">
    <div
      class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] py-[16px] px-[18px] max-w-[60ch]"
    >
      <div
        class="text-[12px] font-semibold text-muted-foreground mb-[12px]"
      >
        Appearance
      </div>
      <div class="flex flex-col gap-[16px]">
        <div class="flex items-center justify-between gap-[18px]">
          <div>
            <div class="font-semibold text-foreground text-[13px]">Theme</div>
            <div class="text-muted-foreground text-[12px] mt-[2px]">
              Dark is the hero; light is first-class.
            </div>
          </div>
          <ThemeToggle />
        </div>
      </div>
    </div>
    <div
      class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] py-[16px] px-[18px] max-w-[60ch]"
    >
      <div
        class="text-[12px] font-semibold text-muted-foreground mb-[12px]"
      >
        Engine
      </div>
      <div class="flex items-center justify-between gap-[18px]">
        <div class="flex items-center gap-[10px]">
          <StatusDot
            tone={engineTone === "warn" ? "warn" : engineTone === "off" ? "off" : "ok"}
            halo={engineTone === "live"}
            size={8}
          />
          <div>
            <div class="font-semibold text-foreground text-[13px]">{engineLine}</div>
            <div class="text-muted-foreground text-[12px] mt-[2px]">WSL2 backend</div>
          </div>
        </div>
        <div class="flex items-center gap-[8px] shrink-0">
          {#if engineState === "running" || engineState === "stopped"}
            <Button
              variant={engineState === "running" ? "destructive" : "success"}
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
          <Button
            variant="outline"
            disabled={engineToggleDisabled || engineState !== "running"}
            onclick={onRestartEngine}
          >
            <RotateCcw aria-hidden="true" />Restart
          </Button>
        </div>
      </div>
    </div>
    <div
      class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] py-[16px] px-[18px] max-w-[60ch]"
    >
      <div
        class="text-[12px] font-semibold text-muted-foreground mb-[12px]"
      >
        Danger zone
      </div>
      <div class="flex flex-col gap-[14px]">
        <p class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground m-0">
          The engine listens on the Windows named pipe by default. The insecure
          loopback-TCP endpoint (127.0.0.1:2375) is only enabled if you opted in
          during setup — it is not recommended for normal use.
        </p>
        <div class="flex items-center gap-[9px] text-[13px] text-muted-foreground">
          <Checkbox id="opt-backup" bind:checked={withBackup} />
          <Label for="opt-backup">
            Export a <code class="font-mono text-[0.92em] text-muted-foreground">.tar</code> backup before removing
          </Label>
        </div>
        <div>
          <Button variant="destructive" disabled={working} onclick={onTeardown}>
            <Trash2 aria-hidden="true" />Remove engine
          </Button>
        </div>
      </div>
    </div>
  </div>
</div>

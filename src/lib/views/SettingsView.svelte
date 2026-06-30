<script lang="ts">
  // Settings screen: appearance (theme) and engine teardown.
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import ThemeToggle from "../components/ThemeToggle.svelte";

  let {
    working,
    withBackup = $bindable(),
    onTeardown,
  }: {
    working: boolean;
    withBackup: boolean;
    onTeardown: () => void;
  } = $props();
</script>

<div class="flex items-end gap-[14px] px-[22px] pt-[22px] pb-[16px] shrink-0">
  <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">Settings</h1>
</div>
<div class="flex-1 overflow-auto grid grid-cols-[1fr] min-h-0">
  <div class="px-[22px] pt-[18px] pb-[24px] min-w-0 flex flex-col gap-[16px]">
    <div
      class="bg-card border border-border rounded-[11px] shadow-sm py-[16px] px-[18px] max-w-[60ch]"
    >
      <div
        class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70 mb-[12px]"
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
      class="bg-card border border-border rounded-[11px] shadow-sm py-[16px] px-[18px] max-w-[60ch]"
    >
      <div
        class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70 mb-[12px]"
      >
        Engine
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

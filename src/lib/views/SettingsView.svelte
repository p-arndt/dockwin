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

<div class="head"><h1>Settings</h1></div>
<div class="body">
  <div class="page">
    <div class="card card-pad" style="max-width:60ch">
      <div class="section-title" style="margin-bottom:12px">Appearance</div>
      <div style="display:flex;flex-direction:column;gap:16px">
        <div class="setrow">
          <div>
            <div class="setrow-t">Theme</div>
            <div class="setrow-s">Dark is the hero; light is first-class.</div>
          </div>
          <ThemeToggle />
        </div>
      </div>
    </div>
    <div class="card card-pad" style="max-width:60ch">
      <div class="section-title" style="margin-bottom:12px">Engine</div>
      <div style="display:flex;flex-direction:column;gap:14px">
        <p class="prose" style="margin:0">
          The engine listens on the Windows named pipe by default. The insecure
          loopback-TCP endpoint (127.0.0.1:2375) is only enabled if you opted in
          during setup — it is not recommended for normal use.
        </p>
        <div class="field">
          <Checkbox id="opt-backup" bind:checked={withBackup} />
          <Label for="opt-backup">
            Export a <code class="code">.tar</code> backup before removing
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

<script lang="ts">
  // Dark/Light segmented toggle. Reads/writes the global theme store, so it can
  // be dropped anywhere (engine-gate bar, top bar, Settings) with no props.
  import Moon from "@lucide/svelte/icons/moon";
  import Sun from "@lucide/svelte/icons/sun";
  import { theme } from "../state/theme.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
</script>

<div
  class="inline-flex items-center gap-0.5 rounded-lg border border-border bg-card p-0.5"
  aria-label="Theme"
>
  <Button
    variant={theme.theme === "dark" ? "secondary" : "ghost"}
    size="sm"
    aria-pressed={theme.theme === "dark"}
    onclick={() => theme.setTheme("dark")}
  >
    <Moon aria-hidden="true" />Dark
  </Button>
  <Button
    variant={theme.theme === "light" ? "secondary" : "ghost"}
    size="sm"
    aria-pressed={theme.theme === "light"}
    onclick={() => theme.setTheme("light")}
  >
    <Sun aria-hidden="true" />Light
  </Button>
</div>

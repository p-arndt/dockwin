<script lang="ts">
  // Slim context bar above the active screen: sidebar toggle + active-view
  // breadcrumb on the left, refresh on the right. (Settings + engine controls
  // are reached from the sidebar engine button.)
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { Button } from "$lib/components/ui/button/index.js";

  let {
    label,
    working,
    onRefresh,
  }: {
    label: string;
    working: boolean;
    onRefresh: () => void;
  } = $props();
</script>

<div class="flex items-center gap-[14px] pl-[22px] pr-[16px] h-[50px] shrink-0 border-b border-border text-muted-foreground">
  <Sidebar.Trigger class="-ml-1 text-muted-foreground hover:text-foreground" />
  <span class="w-px h-[13px] bg-border"></span>
  <span class="font-[650] text-[12.5px] tracking-[-0.1px] text-muted-foreground">{label}</span>
  <span class="flex-1"></span>
  <Button variant="outline" size="icon" title="Refresh" disabled={working} onclick={onRefresh}>
    <RefreshCw aria-hidden="true" />
  </Button>
</div>

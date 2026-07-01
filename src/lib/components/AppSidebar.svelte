<script lang="ts" module>
  import type { Component } from "svelte";
  import Boxes from "@lucide/svelte/icons/boxes";
  import Network from "@lucide/svelte/icons/network";
  import Layers from "@lucide/svelte/icons/layers";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Waypoints from "@lucide/svelte/icons/waypoints";
  import Gauge from "@lucide/svelte/icons/gauge";

  // The set of routable screens. "settings" is reached from the top bar, not the
  // nav, so it has no NavItem. Exported so App.svelte shares one source of truth.
  export type View =
    | "containers"
    | "stacks"
    | "images"
    | "volumes"
    | "networks"
    | "system"
    | "settings";

  type NavSection = "Workloads" | "Resources";
  interface NavItem {
    id: View;
    label: string;
    icon: Component;
    section: NavSection;
  }

  // Nav model, grouped into sections. Counts (see Props.counts) are supplied by
  // the parent so this stays a pure presentation component.
  export const NAV: NavItem[] = [
    { id: "containers", label: "Containers", icon: Boxes, section: "Workloads" },
    { id: "stacks", label: "Stacks", icon: Network, section: "Workloads" },
    { id: "images", label: "Images", icon: Layers, section: "Workloads" },
    { id: "volumes", label: "Volumes", icon: HardDrive, section: "Resources" },
    { id: "networks", label: "Networks", icon: Waypoints, section: "Resources" },
    { id: "system", label: "System", icon: Gauge, section: "Resources" },
  ];
  const NAV_SECTIONS: NavSection[] = ["Workloads", "Resources"];
</script>

<script lang="ts">
  import Settings from "@lucide/svelte/icons/settings";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import StatusDot from "./StatusDot.svelte";

  let {
    activeView,
    counts,
    engineTone,
    engineLine,
    settingsActive,
    onSelect,
    onSettings,
  }: {
    activeView: View;
    counts: Partial<Record<View, number>>;
    engineTone: string;
    engineLine: string;
    settingsActive: boolean;
    onSelect: (view: View) => void;
    onSettings: () => void;
  } = $props();
</script>

<!-- The fixed sidebar panel is pinned to the viewport top (shadcn default); push
     it below the custom titlebar so it doesn't overlap (--titlebar-h set in the
     root layout). -->
<Sidebar.Root
  collapsible="icon"
  class="top-[var(--titlebar-h)]! h-[calc(100svh-var(--titlebar-h))]!"
>
  <Sidebar.Content>
    {#each NAV_SECTIONS as section (section)}
      <Sidebar.Group>
        <Sidebar.GroupLabel>{section}</Sidebar.GroupLabel>
        <Sidebar.GroupContent>
          <Sidebar.Menu>
            {#each NAV.filter((n) => n.section === section) as item (item.id)}
              {@const ItemIcon = item.icon}
              {@const count = counts[item.id]}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton
                  isActive={activeView === item.id}
                  tooltipContent={item.label}
                  aria-current={activeView === item.id ? "page" : undefined}
                  onclick={() => onSelect(item.id)}
                  class="relative font-medium gap-3 rounded-md text-muted-foreground hover:bg-foreground/[0.05]! hover:text-foreground! data-[active=true]:bg-foreground/[0.06]! data-[active=true]:text-foreground! data-[active=true]:shadow-[inset_2px_0_0_var(--primary)] [&_svg]:text-muted-foreground data-[active=true]:[&_svg]:text-foreground"
                >
                  <ItemIcon aria-hidden="true" />
                  <span>{item.label}</span>
                </Sidebar.MenuButton>
                {#if count != null}
                  <Sidebar.MenuBadge>{count}</Sidebar.MenuBadge>
                {/if}
              </Sidebar.MenuItem>
            {/each}
          </Sidebar.Menu>
        </Sidebar.GroupContent>
      </Sidebar.Group>
    {/each}
  </Sidebar.Content>

  <Sidebar.Footer class="group-data-[collapsible=icon]:hidden">
    <button
      type="button"
      onclick={onSettings}
      aria-current={settingsActive ? "page" : undefined}
      title="Engine settings"
      class="group/eng flex w-full items-center gap-3 rounded-[8px] px-[8px] py-[7px] text-left transition-colors hover:bg-foreground/[0.06] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 {settingsActive
        ? 'bg-foreground/[0.06]'
        : ''}"
    >
      <StatusDot
        tone={engineTone === "warn" ? "warn" : engineTone === "off" ? "off" : "run"}
        halo={engineTone === "live"}
        size={6}
      />
      <div class="min-w-0 flex-1 leading-tight">
        <div class="text-[12px] font-medium text-foreground/90">{engineLine}</div>
        <div class="text-[10.5px] text-muted-foreground/80">WSL2 backend</div>
      </div>
      <Settings
        class="size-[14px] shrink-0 text-muted-foreground/60 transition-colors group-hover/eng:text-foreground {settingsActive
          ? 'text-foreground'
          : ''}"
        aria-hidden="true"
      />
    </button>
  </Sidebar.Footer>

  <Sidebar.Rail />
</Sidebar.Root>

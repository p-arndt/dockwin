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
  import Container from "@lucide/svelte/icons/container";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";

  let {
    activeView,
    counts,
    engineTone,
    engineLine,
    onSelect,
  }: {
    activeView: View;
    counts: Partial<Record<View, number>>;
    engineTone: string;
    engineLine: string;
    onSelect: (view: View) => void;
  } = $props();
</script>

<Sidebar.Root collapsible="icon">
  <Sidebar.Header>
    <div
      class="flex items-center gap-2.5 p-1 group-data-[collapsible=icon]:justify-center group-data-[collapsible=icon]:p-0"
    >
      <span
        class="flex aspect-square size-8 shrink-0 items-center justify-center rounded-lg bg-[linear-gradient(150deg,var(--lime-bright),var(--lime-deep))] text-[var(--lime-ink)] shadow-[0_4px_14px_-4px_var(--lime-line),inset_0_1px_0_rgba(255,255,255,0.35)]"
      >
        <Container size={17} aria-hidden="true" />
      </span>
      <div class="grid flex-1 leading-tight group-data-[collapsible=icon]:hidden">
        <span class="text-sm font-semibold tracking-tight">dockwin</span>
        <span class="text-[11px] text-muted-foreground">Docker workspace</span>
      </div>
    </div>
  </Sidebar.Header>

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
                  class="font-medium text-muted-foreground hover:bg-foreground/10! hover:text-foreground! data-[active=true]:bg-foreground/[0.14]! data-[active=true]:text-foreground! data-[active=true]:shadow-[inset_2px_0_0_var(--lime)] [&_svg]:text-muted-foreground data-[active=true]:[&_svg]:text-[var(--lime)]"
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
    <div class="eng mt-0! border-t-0! px-0.5! py-1!">
      <div class="row">
        <span class="dot {engineTone}"></span>
        <div>
          <div class="et">{engineLine}</div>
          <div class="es">WSL2 backend</div>
        </div>
      </div>
    </div>
  </Sidebar.Footer>

  <Sidebar.Rail />
</Sidebar.Root>

<script lang="ts">
  // Stateless view: renders the containers table in the crafted v2 language. All
  // engine logic lives in the parent (App.svelte) / api.ts — this component only
  // renders rows + surfaces actions and the per-row "select" affordance. Svelte 5
  // runes API.
  import Box from "@lucide/svelte/icons/box";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { openExternal } from "../api/external";
  import type { NormalizedContainer, NormalizedPort } from "../types";

  type Action = "start" | "stop" | "restart" | "remove";

  interface Props {
    containers?: NormalizedContainer[];
    pending?: Set<string>;
    onAction?: (action: Action, c: NormalizedContainer) => void;
    onSelect?: (c: NormalizedContainer) => void;
  }

  let {
    containers = [],
    pending = new Set<string>(),
    onAction,
    onSelect,
  }: Props = $props();

  function act(e: MouseEvent, action: Action, c: NormalizedContainer) {
    e.stopPropagation();
    onAction?.(action, c);
  }

  // Quiet status descriptor: a tone (drives the dot + word colour) + the lamp
  // class for the name-cell indicator + a single word.
  interface StatusView {
    tone: "run" | "warn" | "err" | "exit";
    lamp: "run" | "warn" | "err" | "";
    word: string;
  }
  function statusOf(c: NormalizedContainer): StatusView {
    if (c.running) return { tone: "run", lamp: "run", word: "Running" };
    switch (c.state) {
      case "paused":
        return { tone: "warn", lamp: "warn", word: "Paused" };
      case "restarting":
        return { tone: "warn", lamp: "warn", word: "Restarting" };
      case "created":
        return { tone: "exit", lamp: "", word: "Created" };
      case "dead":
        return { tone: "err", lamp: "err", word: "Dead" };
      case "exited":
        return { tone: "exit", lamp: "", word: "Exited" };
      default:
        return {
          tone: "exit",
          lamp: "",
          word: c.state ? c.state[0].toUpperCase() + c.state.slice(1) : "Unknown",
        };
    }
  }

  function portTitle(p: NormalizedPort): string {
    if (p.wildcard) return `${p.host}:${p.container}/${p.proto}`;
    return `Bound to ${p.ip} — NOT forwarded to Windows localhost`;
  }

  function openPort(e: MouseEvent, url: string) {
    e.stopPropagation();
    openExternal(url);
  }
</script>

<div
  class="data-table-card overflow-hidden rounded-[11px] border border-border bg-card shadow-sm"
>
  <Table.Root class="table-fixed">
    <Table.Header>
      <Table.Row class="hover:bg-transparent">
        <Table.Head
          class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
          style="width:30%">Name</Table.Head
        >
        <Table.Head
          class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
          style="width:22%">Image</Table.Head
        >
        <Table.Head
          class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
          style="width:16%">Status</Table.Head
        >
        <Table.Head
          class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
          style="width:22%">Ports</Table.Head
        >
        <Table.Head
          class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
          style="width:10%"
        ></Table.Head>
      </Table.Row>
    </Table.Header>
    <Table.Body>
      {#if containers.length === 0}
        <Table.Row class="hover:bg-transparent">
          <Table.Cell colspan={5} class="py-7 text-center text-muted-foreground"
            >No containers.</Table.Cell
          >
        </Table.Row>
      {:else}
        {#each containers as c (c.id)}
          {@const acting = pending.has(c.id)}
          {@const st = statusOf(c)}
          <Table.Row
            class="group relative cursor-pointer data-[sel=true]:bg-muted data-[sel=true]:shadow-[inset_2px_0_0_var(--lime)]"
            style={acting ? "opacity:.55" : undefined}
            role="button"
            tabindex={0}
            aria-busy={acting}
            onclick={() => onSelect?.(c)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect?.(c);
              }
            }}
          >
            <Table.Cell>
              <div class="flex min-w-0 items-center gap-[12px]">
                <span
                  class="h-[7px] w-[7px] shrink-0 rounded-full {st.lamp ===
                  'run'
                    ? 'bg-chart-2'
                    : st.lamp === 'warn'
                      ? 'bg-chart-3'
                      : st.lamp === 'err'
                        ? 'bg-destructive'
                        : 'bg-chart-5'}"
                ></span>
                <span
                  class="grid size-[30px] shrink-0 place-items-center rounded-[8px] border border-border bg-muted text-muted-foreground"
                  ><Box aria-hidden="true" class="size-[15px]" /></span
                >
                <div class="min-w-0">
                  <div
                    class="truncate text-[13.5px] font-semibold leading-[1.25] tracking-[-0.1px] text-foreground"
                    title={c.name}
                  >
                    {c.name}
                  </div>
                  <div
                    class="font-mono text-[11px] leading-[1.3] text-muted-foreground/70"
                    title={c.id}
                  >
                    {c.shortId}
                  </div>
                </div>
              </div>
            </Table.Cell>

            <Table.Cell>
              <span
                class="block min-w-0 truncate font-mono text-[12px] text-muted-foreground"
                title={c.image}>{c.image}</span
              >
            </Table.Cell>

            <Table.Cell>
              <div class="flex min-w-0 flex-col gap-[2px]">
                <span
                  class="flex items-center gap-[7px] text-[12.5px] font-medium {st.tone ===
                  'warn'
                    ? 'text-chart-3'
                    : st.tone === 'err'
                      ? 'text-destructive'
                      : st.tone === 'exit'
                        ? 'text-muted-foreground'
                        : 'text-foreground'}"
                  ><span
                    class="h-[6px] w-[6px] shrink-0 rounded-full {st.tone ===
                    'warn'
                      ? 'bg-chart-3'
                      : st.tone === 'err'
                        ? 'bg-destructive'
                        : st.tone === 'exit'
                          ? 'bg-chart-5'
                          : 'bg-chart-2'}"
                  ></span>{st.word}</span
                >
                {#if c.status}<span
                    class="truncate text-[11px] tabular-nums text-muted-foreground/70"
                    >{c.status}</span
                  >{/if}
              </div>
            </Table.Cell>

            <Table.Cell>
              <div class="flex flex-wrap gap-[5px]">
                {#if c.ports.length === 0}
                  <span class="text-muted-foreground/70">—</span>
                {:else}
                  {#each c.ports as p, i (i)}
                    {#if p.url}
                      <Button
                        variant="outline"
                        size="xs"
                        class="h-6 gap-1 px-2 font-mono text-[11px]"
                        title={`Open ${p.url} (forwarded to Windows localhost)`}
                        onclick={(e) => openPort(e, p.url!)}
                      >
                        :{p.host}<ExternalLink aria-hidden="true" />
                      </Button>
                    {:else}
                      <Badge
                        variant="outline"
                        class="font-mono text-[11px] font-normal"
                        title={portTitle(p)}
                        >{p.host}:{p.container}/{p.proto}</Badge
                      >
                    {/if}
                  {/each}
                {/if}
              </div>
            </Table.Cell>

            <Table.Cell class="text-right">
              <div class="inline-flex items-center justify-end gap-1">
                <!-- primary lifecycle action: always visible, colour-coded -->
                {#if c.running}
                  <Button
                    variant="destructive"
                    size="icon-sm"
                    title="Stop"
                    disabled={acting}
                    onclick={(e) => act(e, "stop", c)}
                  >
                    <Square aria-hidden="true" />
                  </Button>
                {:else}
                  <Button
                    variant="success"
                    size="icon-sm"
                    title="Start"
                    disabled={acting}
                    onclick={(e) => act(e, "start", c)}
                  >
                    <Play aria-hidden="true" />
                  </Button>
                {/if}
                <!-- secondary actions: revealed on row hover/focus -->
                <div
                  class="inline-flex gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
                >
                  {#if c.running}
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      title="Restart"
                      disabled={acting}
                      onclick={(e) => act(e, "restart", c)}
                    >
                      <RotateCw aria-hidden="true" />
                    </Button>
                  {/if}
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    class="text-muted-foreground hover:bg-destructive/15 hover:text-destructive"
                    title="Remove"
                    disabled={acting}
                    onclick={(e) => act(e, "remove", c)}
                  >
                    <Trash2 aria-hidden="true" />
                  </Button>
                </div>
              </div>
            </Table.Cell>
          </Table.Row>
        {/each}
      {/if}
    </Table.Body>
  </Table.Root>
</div>

<style>
  /* The shadcn table container renders its own `overflow-x-auto`, but inside a
     clipping card with fixed-width truncating columns that only yields a stray
     1px scrollbar. This targets a child component's slot, so it can't be an
     inline utility — keep it as a scoped :global rule. */
  .data-table-card :global([data-slot="table-container"]) {
    overflow: visible;
  }
</style>

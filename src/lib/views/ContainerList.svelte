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

<div class="card list-card overflow-hidden">
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
              <div class="cell-name">
                <span class="lamp {st.lamp}"></span>
                <span class="av"><Box aria-hidden="true" /></span>
                <div style="min-width:0">
                  <div class="nm" title={c.name}>{c.name}</div>
                  <div class="id" title={c.id}>{c.shortId}</div>
                </div>
              </div>
            </Table.Cell>

            <Table.Cell>
              <span class="img" title={c.image}>{c.image}</span>
            </Table.Cell>

            <Table.Cell>
              <div class="st {st.tone}">
                <span class="l"><span class="d"></span>{st.word}</span>
                {#if c.status}<span class="sub">{c.status}</span>{/if}
              </div>
            </Table.Cell>

            <Table.Cell>
              <div class="ports">
                {#if c.ports.length === 0}
                  <span class="muted">—</span>
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
              <div
                class="inline-flex justify-end gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
              >
                {#if c.running}
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title="Stop"
                    disabled={acting}
                    onclick={(e) => act(e, "stop", c)}
                  >
                    <Square aria-hidden="true" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title="Restart"
                    disabled={acting}
                    onclick={(e) => act(e, "restart", c)}
                  >
                    <RotateCw aria-hidden="true" />
                  </Button>
                {:else}
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title="Start"
                    disabled={acting}
                    onclick={(e) => act(e, "start", c)}
                  >
                    <Play aria-hidden="true" />
                  </Button>
                {/if}
                <Button
                  variant="ghost"
                  size="icon-sm"
                  class="text-muted-foreground hover:text-destructive"
                  title="Remove"
                  disabled={acting}
                  onclick={(e) => act(e, "remove", c)}
                >
                  <Trash2 aria-hidden="true" />
                </Button>
              </div>
            </Table.Cell>
          </Table.Row>
        {/each}
      {/if}
    </Table.Body>
  </Table.Root>
</div>

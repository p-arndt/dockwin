<script lang="ts">
  // Docker Compose stacks: containers grouped by their compose project, with
  // start/stop/restart applied to the whole stack. Stateless view — all actions
  // bubble to the parent (App.svelte), which fans them out over the existing
  // container commands. Presentation uses the v2 foundation classes/tokens.
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Package from "@lucide/svelte/icons/package";
  import Box from "@lucide/svelte/icons/box";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Pill from "../components/Pill.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { openExternal, openFolder, wslToWindowsPath } from "../api/external";
  import type { Stack, NormalizedContainer, NormalizedPort } from "../types";

  type StackAction = "start" | "stop" | "restart";

  let {
    stacks = [],
    pending = new Set<string>(),
    onStackAction,
  }: {
    stacks: Stack[];
    pending: Set<string>;
    onStackAction?: (action: StackAction, stack: Stack) => void;
  } = $props();

  function stackBusy(s: Stack): boolean {
    return s.containers.some((c) => pending.has(c.id));
  }
  function act(action: StackAction, s: Stack) {
    onStackAction?.(action, s);
  }

  function stackTone(s: Stack): "ok" | "warn" | "neutral" {
    if (s.running === 0) return "neutral";
    return s.running === s.total ? "ok" : "warn";
  }

  // Loud status word for a service (running => "Running"; otherwise capitalise
  // the raw docker state, falling back to "Stopped").
  function stateWord(c: NormalizedContainer): string {
    if (c.running) return "Running";
    const st = (c.state || "").trim();
    if (!st) return "Stopped";
    return st.charAt(0).toUpperCase() + st.slice(1);
  }

  function portLabel(p: NormalizedPort): string {
    return `${p.host}:${p.container}/${p.proto}`;
  }
  function portTitle(p: NormalizedPort): string {
    if (p.url) return `Open ${p.url} (forwarded to Windows localhost)`;
    if (p.wildcard) return portLabel(p);
    return `Bound to ${p.ip} — NOT forwarded to Windows localhost`;
  }
  function openPort(p: NormalizedPort) {
    const url = p.url ?? `http://localhost:${p.host}`;
    void openExternal(url);
  }
</script>

{#if stacks.length === 0}
  <div
    class="bg-card border border-border rounded-[11px] shadow-sm py-[16px] px-[18px]"
  >
    <p
      class="max-w-[64ch] text-[13px] leading-[1.6] text-muted-foreground"
      style="margin:0"
    >
      No Compose stacks. Containers started with
      <code class="font-mono text-[0.92em] text-muted-foreground"
        >docker compose</code
      > appear here, grouped by project.
    </p>
  </div>
{:else}
  <div class="flex flex-col gap-[16px]">
    {#each stacks as s (s.project)}
      {@const busy = stackBusy(s)}
      {@const allRunning = s.running === s.total}
      <section
        class="bg-card border border-border rounded-[11px] shadow-sm overflow-hidden"
      >
        <header
          class="flex items-center gap-[12px] py-[12px] px-[16px] border-b border-border bg-muted/50"
        >
          <span
            class="grid place-items-center size-[30px] rounded-[8px] shrink-0 bg-muted border border-border [&_svg]:size-[15px]"
            ><Package aria-hidden="true" /></span
          >
          <span
            class="font-[650] text-[14px] tracking-[-0.2px] text-foreground truncate"
            title={s.project}>{s.project}</span
          >
          <Pill tone={stackTone(s)} dot={s.running > 0}>
            <span class="tabular-nums">{s.running}</span><span
              class="text-muted-foreground/70 mx-px">/</span
            ><span class="tabular-nums">{s.total}</span> running
          </Pill>
          {#if s.workingDir}
            <Button
              variant="outline"
              size="sm"
              class="max-w-[360px] font-mono"
              title={`Open ${wslToWindowsPath(s.workingDir)} in Explorer`}
              onclick={() => openFolder(s.workingDir!)}
            >
              <FolderOpen aria-hidden="true" />
              <span class="truncate">{wslToWindowsPath(s.workingDir)}</span>
            </Button>
          {/if}
          <div class="ml-auto flex items-center gap-[6px]">
            <Button
              variant="outline"
              size="sm"
              disabled={busy || allRunning}
              onclick={() => act("start", s)}
              title="Start the whole stack"
            >
              <Play aria-hidden="true" /> Start
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={busy || s.running === 0}
              onclick={() => act("stop", s)}
              title="Stop the whole stack"
            >
              <Square aria-hidden="true" /> Stop
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={busy || s.running === 0}
              onclick={() => act("restart", s)}
              title="Restart the whole stack"
            >
              <RotateCw aria-hidden="true" /> Restart
            </Button>
          </div>
        </header>

        <Table.Root class="table-fixed">
          <Table.Header>
            <Table.Row class="hover:bg-transparent">
              <Table.Head
                class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
                style="width:30%">Service</Table.Head
              >
              <Table.Head
                class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
                style="width:26%">Image</Table.Head
              >
              <Table.Head
                class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
                style="width:20%">Status</Table.Head
              >
              <Table.Head
                class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
                style="width:24%">Ports</Table.Head
              >
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each s.containers as c (c.id)}
              <Table.Row
                class="hover:bg-transparent"
                style={pending.has(c.id) ? "opacity:.55" : undefined}
              >
                <Table.Cell>
                  <div class="flex items-center gap-[12px] min-w-0">
                    <span
                      class="w-[7px] h-[7px] rounded-full shrink-0 {c.running
                        ? 'bg-chart-2'
                        : 'bg-chart-5'}"
                    ></span>
                    <span
                      class="grid place-items-center size-[30px] rounded-[8px] shrink-0 bg-muted border border-border [&_svg]:size-[15px]"
                      ><Box aria-hidden="true" /></span
                    >
                    <div style="min-width:0">
                      <div
                        class="font-semibold text-[13.5px] text-foreground tracking-[-0.1px] leading-[1.25] truncate"
                      >
                        {c.name}
                      </div>
                      <div
                        class="font-mono text-[11px] text-muted-foreground/70 leading-[1.3]"
                      >
                        {c.shortId}
                      </div>
                    </div>
                  </div>
                </Table.Cell>

                <Table.Cell>
                  <span
                    class="font-mono text-[12px] text-muted-foreground block min-w-0 truncate"
                    title={c.image}>{c.image}</span
                  >
                </Table.Cell>

                <Table.Cell>
                  <div class="flex flex-col gap-[2px] min-w-0">
                    <span
                      class="flex items-center gap-[7px] text-[12.5px] font-medium {c.running
                        ? 'text-foreground'
                        : 'text-muted-foreground'}"
                      ><span
                        class="w-[6px] h-[6px] rounded-full shrink-0 {c.running
                          ? 'bg-chart-2'
                          : 'bg-chart-5'}"
                      ></span>{stateWord(c)}</span
                    >
                    {#if c.status}<span
                        class="text-[11px] text-muted-foreground/70 tabular-nums truncate"
                        >{c.status}</span
                      >{/if}
                  </div>
                </Table.Cell>

                <Table.Cell>
                  <div class="flex gap-[5px] flex-wrap">
                    {#if c.ports.length === 0}
                      <span class="text-muted-foreground/70">—</span>
                    {:else}
                      {#each c.ports as p, i (i)}
                        {#if p.url}
                          <Button
                            variant="outline"
                            size="xs"
                            class="h-6 gap-1 px-2 font-mono text-[11px]"
                            onclick={() => openPort(p)}
                            title={portTitle(p)}
                          >
                            {portLabel(p)}<ExternalLink aria-hidden="true" />
                          </Button>
                        {:else}
                          <Badge
                            variant="outline"
                            class="font-mono text-[11px] font-normal"
                            title={portTitle(p)}>{portLabel(p)}</Badge
                          >
                        {/if}
                      {/each}
                    {/if}
                  </div>
                </Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </section>
    {/each}
  </div>
{/if}

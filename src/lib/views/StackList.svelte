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
  <div class="card card-pad">
    <p class="prose" style="margin:0">
      No Compose stacks. Containers started with
      <code class="code">docker compose</code> appear here, grouped by project.
    </p>
  </div>
{:else}
  <div class="stacks">
    {#each stacks as s (s.project)}
      {@const busy = stackBusy(s)}
      {@const allRunning = s.running === s.total}
      <section class="card overflow-hidden">
        <header class="shead">
          <span class="av"><Package aria-hidden="true" /></span>
          <span class="nm" title={s.project}>{s.project}</span>
          <Pill tone={stackTone(s)} dot={s.running > 0}>
            <span class="num">{s.running}</span><span class="x">/</span><span
              class="num">{s.total}</span
            > running
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
              <span class="shead-dir-path">{wslToWindowsPath(s.workingDir)}</span>
            </Button>
          {/if}
          <div class="shead-acts">
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
                  <div class="cell-name">
                    <span class="lamp" class:run={c.running}></span>
                    <span class="av"><Box aria-hidden="true" /></span>
                    <div style="min-width:0">
                      <div class="nm">{c.name}</div>
                      <div class="id">{c.shortId}</div>
                    </div>
                  </div>
                </Table.Cell>

                <Table.Cell>
                  <span class="img" title={c.image}>{c.image}</span>
                </Table.Cell>

                <Table.Cell>
                  <div class="st" class:run={c.running} class:exit={!c.running}>
                    <span class="l"><span class="d"></span>{stateWord(c)}</span>
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

<style>
  .stacks {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* Per-stack header strip: project identity + whole-stack actions. */
  .shead {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    background: linear-gradient(180deg, var(--s2), transparent);
  }
  .shead .av {
    width: 30px;
    height: 30px;
  }
  .shead .nm {
    font-weight: 650;
    font-size: 14px;
    letter-spacing: -0.2px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .shead-acts {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* Open-the-compose-folder chip: keep the monospace path truncating. */
  .shead-dir-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Quieten the count separator inside the pill. */
  .x {
    color: var(--text-4);
    margin: 0 1px;
  }
</style>

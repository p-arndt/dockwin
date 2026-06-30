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
  import { errText } from "../api";
  import type { Stack, NormalizedContainer, NormalizedPort } from "../types";

  type StackAction = "start" | "stop" | "restart";

  let {
    stacks = [],
    pending = new Set<string>(),
    onStackAction,
    setFooter,
  }: {
    stacks: Stack[];
    pending: Set<string>;
    onStackAction?: (action: StackAction, stack: Stack) => void;
    setFooter?: (msg: string, isError?: boolean) => void;
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

  // Quiet status descriptor: a tone (drives the dot + word colour) + the lamp
  // class for the name-cell indicator + a single word. Mirrors
  // ContainerList.svelte's statusOf() so service status reads identically
  // whether viewed from Containers or from a Stack's service table.
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
    openExternal(url).catch((e) => setFooter?.(`Couldn't open ${url}: ${errText(e)}`, true));
  }

  function openStackFolder(s: Stack) {
    openFolder(s.workingDir!).catch((e) =>
      setFooter?.(`Couldn't open ${wslToWindowsPath(s.workingDir!)}: ${errText(e)}`, true)
    );
  }
</script>

{#if stacks.length === 0}
  <div
    class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] py-[16px] px-[18px]"
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
        class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] overflow-hidden"
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
              onclick={() => openStackFolder(s)}
            >
              <FolderOpen aria-hidden="true" />
              <span class="truncate">{wslToWindowsPath(s.workingDir)}</span>
            </Button>
          {/if}
          <div class="ml-auto flex items-center gap-[6px]">
            <Button
              variant="success"
              size="sm"
              disabled={busy || allRunning}
              onclick={() => act("start", s)}
              title="Start the whole stack"
            >
              <Play aria-hidden="true" /> Start
            </Button>
            <Button
              variant="destructive"
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
              {@const st = statusOf(c)}
              <Table.Row
                class="hover:bg-transparent"
                style={pending.has(c.id) ? "opacity:.55" : undefined}
              >
                <Table.Cell>
                  <div class="flex items-center gap-[12px] min-w-0">
                    <span
                      class="w-[7px] h-[7px] rounded-full shrink-0 {st.lamp === 'run'
                        ? 'bg-chart-2'
                        : st.lamp === 'warn'
                          ? 'bg-chart-3'
                          : st.lamp === 'err'
                            ? 'bg-destructive'
                            : 'bg-chart-5'}"
                    ></span>
                    <span
                      class="grid place-items-center size-[30px] rounded-[8px] shrink-0 bg-muted border border-border [&_svg]:size-[15px]"
                      ><Box aria-hidden="true" /></span
                    >
                    <div class="min-w-0">
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
                      class="flex items-center gap-[7px] text-[12.5px] font-medium {st.tone ===
                      'warn'
                        ? 'text-chart-3'
                        : st.tone === 'err'
                          ? 'text-destructive'
                          : st.tone === 'exit'
                            ? 'text-muted-foreground'
                            : 'text-foreground'}"
                      ><span
                        class="w-[6px] h-[6px] rounded-full shrink-0 {st.tone === 'warn'
                          ? 'bg-chart-3'
                          : st.tone === 'err'
                            ? 'bg-destructive'
                            : st.tone === 'exit'
                              ? 'bg-chart-5'
                              : 'bg-chart-2'}"
                      ></span>{st.word}</span
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

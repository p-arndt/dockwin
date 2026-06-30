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
  import Pill from "./components/Pill.svelte";
  import { openExternal } from "./openExternal";
  import type { Stack, NormalizedContainer, NormalizedPort } from "./types";

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

  // Shared grid template for each stack's service table (header + rows).
  const COLS =
    "minmax(160px,1.5fr) minmax(120px,1.3fr) minmax(120px,1fr) minmax(150px,1.2fr)";

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
      <section class="table">
        <header class="shead">
          <span class="av"><Package aria-hidden="true" /></span>
          <span class="nm" title={s.project}>{s.project}</span>
          <Pill tone={stackTone(s)} dot={s.running > 0}>
            <span class="num">{s.running}</span><span class="x">/</span><span
              class="num">{s.total}</span
            > running
          </Pill>
          <div class="shead-acts">
            <button
              class="btn btn-soft sm"
              disabled={busy || allRunning}
              onclick={() => act("start", s)}
              title="Start the whole stack"
            >
              <Play aria-hidden="true" /> Start
            </button>
            <button
              class="btn btn-soft sm"
              disabled={busy || s.running === 0}
              onclick={() => act("stop", s)}
              title="Stop the whole stack"
            >
              <Square aria-hidden="true" /> Stop
            </button>
            <button
              class="btn btn-soft sm"
              disabled={busy || s.running === 0}
              onclick={() => act("restart", s)}
              title="Restart the whole stack"
            >
              <RotateCw aria-hidden="true" /> Restart
            </button>
          </div>
        </header>

        <div class="thead" style="--cols:{COLS}">
          <span>Service</span><span>Image</span><span>Status</span><span
            >Ports</span
          >
        </div>

        {#each s.containers as c (c.id)}
          <div
            class="trow"
            style="--cols:{COLS};cursor:default"
            class:busy={pending.has(c.id)}
          >
            <div class="cell-name">
              <span class="lamp" class:run={c.running}></span>
              <span class="av"><Box aria-hidden="true" /></span>
              <div style="min-width:0">
                <div class="nm">{c.name}</div>
                <div class="id">{c.shortId}</div>
              </div>
            </div>

            <span class="img" title={c.image}>{c.image}</span>

            <div class="st" class:run={c.running} class:exit={!c.running}>
              <span class="l"><span class="d"></span>{stateWord(c)}</span>
              {#if c.status}<span class="sub">{c.status}</span>{/if}
            </div>

            <div class="ports">
              {#if c.ports.length === 0}
                <span class="muted">—</span>
              {:else}
                {#each c.ports as p, i (i)}
                  {#if p.url}
                    <button
                      class="port port-link"
                      onclick={() => openPort(p)}
                      title={portTitle(p)}
                    >
                      {portLabel(p)}<ExternalLink aria-hidden="true" />
                    </button>
                  {:else}
                    <span class="port" title={portTitle(p)}>{portLabel(p)}</span
                    >
                  {/if}
                {/each}
              {/if}
            </div>
          </div>
        {/each}
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

  /* Quieten the count separator inside the pill. */
  .x {
    color: var(--text-4);
    margin: 0 1px;
  }

  /* Service rows are read-only here (no detail routing). */
  .trow.busy {
    opacity: 0.55;
  }

  /* Clickable published-port chip — neutral, brightens on hover. */
  .port-link {
    font: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    transition: color 0.13s var(--ease), border-color 0.13s var(--ease);
  }
  .port-link:hover {
    color: var(--text);
    border-color: var(--text-4);
  }
  .port-link :global(svg) {
    width: 11px;
    height: 11px;
    color: var(--text-4);
  }
</style>

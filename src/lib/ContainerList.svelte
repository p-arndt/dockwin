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
  import { openExternal } from "./openExternal";
  import type { NormalizedContainer, NormalizedPort } from "./types";

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

  // Shared grid template (Name · Image · Status · Ports · actions gutter).
  const COLS =
    "minmax(190px,1.7fr) minmax(120px,1.2fr) minmax(112px,1fr) minmax(120px,1.3fr) 96px";

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

<div class="table">
  <div class="thead" style="--cols:{COLS}">
    <span>Name</span>
    <span>Image</span>
    <span>Status</span>
    <span>Ports</span>
    <span></span>
  </div>

  {#if containers.length === 0}
    <div class="empty">No containers.</div>
  {:else}
    {#each containers as c (c.id)}
      {@const acting = pending.has(c.id)}
      {@const st = statusOf(c)}
      <div
        class="trow"
        style="--cols:{COLS}"
        style:opacity={acting ? 0.55 : undefined}
        role="button"
        tabindex="0"
        aria-busy={acting}
        onclick={() => onSelect?.(c)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect?.(c);
          }
        }}
      >
        <div class="cell-name">
          <span class="lamp {st.lamp}"></span>
          <span class="av"><Box aria-hidden="true" /></span>
          <div style="min-width:0">
            <div class="nm" title={c.name}>{c.name}</div>
            <div class="id" title={c.id}>{c.shortId}</div>
          </div>
        </div>

        <span class="img" title={c.image}>{c.image}</span>

        <div class="st {st.tone}">
          <span class="l"><span class="d"></span>{st.word}</span>
          {#if c.status}<span class="sub">{c.status}</span>{/if}
        </div>

        <div class="ports">
          {#if c.ports.length === 0}
            <span class="muted">—</span>
          {:else}
            {#each c.ports as p, i (i)}
              {#if p.url}
                <button
                  class="port"
                  type="button"
                  style="cursor:pointer"
                  title={`Open ${p.url} (forwarded to Windows localhost)`}
                  onclick={(e) => openPort(e, p.url!)}
                >
                  :{p.host}<ExternalLink aria-hidden="true" />
                </button>
              {:else}
                <span class="port" title={portTitle(p)}
                  >{p.host}:{p.container}/{p.proto}</span
                >
              {/if}
            {/each}
          {/if}
        </div>

        <div class="rowact">
          {#if c.running}
            <button
              type="button"
              title="Stop"
              disabled={acting}
              onclick={(e) => act(e, "stop", c)}
            >
              <Square aria-hidden="true" />
            </button>
            <button
              type="button"
              title="Restart"
              disabled={acting}
              onclick={(e) => act(e, "restart", c)}
            >
              <RotateCw aria-hidden="true" />
            </button>
          {:else}
            <button
              type="button"
              title="Start"
              disabled={acting}
              onclick={(e) => act(e, "start", c)}
            >
              <Play aria-hidden="true" />
            </button>
          {/if}
          <button
            class="dng"
            type="button"
            title="Remove"
            disabled={acting}
            onclick={(e) => act(e, "remove", c)}
          >
            <Trash2 aria-hidden="true" />
          </button>
        </div>
      </div>
    {/each}
  {/if}
</div>

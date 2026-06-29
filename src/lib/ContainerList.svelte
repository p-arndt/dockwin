<script lang="ts">
  // Stateless view: renders containers + action buttons. All logic lives in the
  // parent (App.svelte) / api.ts. Svelte 5 runes API.
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ExternalLink from "@lucide/svelte/icons/external-link";
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

  function act(action: Action, c: NormalizedContainer) {
    onAction?.(action, c);
  }

  function portTitle(p: NormalizedPort): string {
    if (p.wildcard) return `${p.host}:${p.container}/${p.proto}`;
    return `Bound to ${p.ip} — NOT forwarded to Windows localhost`;
  }

  const BTN_BASE =
    "inline-flex cursor-pointer items-center gap-1 rounded-md border px-2 py-[3px] text-xs transition-colors disabled:cursor-default disabled:opacity-45";
</script>

{#if containers.length === 0}
  <div class="px-3.5 py-7 text-center text-[#9aa3af]">No containers.</div>
{:else}
  <div class="overflow-x-auto">
    <table class="w-full border-collapse text-[13px]">
      <thead>
        <tr>
          <th
            class="sticky top-0 w-[22px] whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
          ></th>
          <th
            class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
            >Name</th
          >
          <th
            class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
            >Image</th
          >
          <th
            class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
            >Status</th
          >
          <th
            class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
            >Ports</th
          >
          <th
            class="sticky top-0 w-[1%] whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
            >Actions</th
          >
        </tr>
      </thead>
      <tbody>
        {#each containers as c (c.id)}
          {@const acting = pending.has(c.id)}
          <tr class="hover:bg-[#1b1f27] {acting ? 'opacity-60' : ''}">
            <td class="w-[22px] border-b border-[#262b34] px-3 py-2.5 align-middle">
              <span
                class="cdot {c.running ? 'cdot-on' : 'cdot-off'}"
                title={c.state || "unknown"}
              ></span>
            </td>
            <td
              class="border-b border-[#262b34] px-3 py-2.5 align-middle font-medium"
            >
              <button
                class="cursor-pointer text-left font-medium text-[#e6e8eb] hover:text-[#2f81f7] hover:underline"
                title={`${c.shortId} — open details (stats, inspect, top)`}
                onclick={() => onSelect?.(c)}>{c.name}</button
              >
            </td>
            <td
              class="font-mono-app max-w-[240px] overflow-hidden text-ellipsis whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-xs text-[#9aa3af]"
              title={c.image}>{c.image}</td
            >
            <td
              class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
              >{c.status || c.state}</td
            >
            <td
              class="font-mono-app border-b border-[#262b34] px-3 py-2.5 align-middle text-xs"
            >
              {#if c.ports.length === 0}
                &mdash;
              {:else}
                {#each c.ports as p, i (i)}
                  {#if i > 0}{" "}{/if}
                  {#if p.url}
                    <a
                      class="inline-flex items-center gap-0.5 text-[#2f81f7] no-underline hover:underline"
                      href={p.url}
                      target="_blank"
                      rel="noreferrer"
                      title={`Open ${p.url} (forwarded to Windows localhost)`}
                      >{p.host}:{p.container}/{p.proto}<ExternalLink
                        size={11}
                        aria-hidden="true"
                      /></a
                    >
                  {:else}
                    <span class="text-[#9aa3af]" title={portTitle(p)}
                      >{p.host}:{p.container}/{p.proto}</span
                    >
                  {/if}
                {/each}
              {/if}
            </td>
            <td class="border-b border-[#262b34] px-3 py-2.5 align-middle">
              <div class="flex justify-end gap-1.5">
                {#if c.running}
                  <button
                    class="{BTN_BASE} border-[#d2992280] text-[#d29922] hover:not-disabled:bg-[#d299221f]"
                    disabled={acting}
                    onclick={() => act("stop", c)}
                    ><Square size={13} aria-hidden="true" />Stop</button
                  >
                  <button
                    class="{BTN_BASE} border-[#262b34] text-[#e6e8eb] hover:not-disabled:bg-[#21262d]"
                    disabled={acting}
                    onclick={() => act("restart", c)}
                    ><RotateCw size={13} aria-hidden="true" />Restart</button
                  >
                {:else}
                  <button
                    class="{BTN_BASE} border-[#3fb95080] text-[#3fb950] hover:not-disabled:bg-[#3fb9501f]"
                    disabled={acting}
                    onclick={() => act("start", c)}
                    ><Play size={13} aria-hidden="true" />Start</button
                  >
                {/if}
                <button
                  class="{BTN_BASE} border-[#f8514980] text-[#f85149] hover:not-disabled:bg-[#f851491f]"
                  disabled={acting}
                  onclick={() => act("remove", c)}
                  ><Trash2 size={13} aria-hidden="true" />Remove</button
                >
              </div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

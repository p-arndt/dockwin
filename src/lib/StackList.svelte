<script lang="ts">
  // Docker Compose stacks: containers grouped by their compose project, with
  // start/stop/restart applied to the whole stack. Stateless view — all actions
  // bubble to the parent (App.svelte), which fans them out over the existing
  // container commands.
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Package from "@lucide/svelte/icons/package";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import type { Stack } from "./types";

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
</script>

{#if stacks.length === 0}
  <div class="px-3.5 py-6 text-center text-[13px] text-[#9aa3af]">
    No Compose stacks. Containers started with
    <code class="text-[#c7ccd4]">docker compose</code> appear here, grouped by project.
  </div>
{:else}
  <div class="flex flex-col gap-3">
    {#each stacks as s (s.project)}
      {@const busy = stackBusy(s)}
      {@const allRunning = s.running === s.total}
      <section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
        <header
          class="flex items-center gap-2.5 border-b border-[#262b34] px-3.5 py-2.5"
        >
          <Package size={16} class="text-[#2f81f7]" aria-hidden="true" />
          <span class="text-sm font-semibold">{s.project}</span>
          <span
            class="rounded-full border border-[#262b34] bg-[#21262d] px-2 py-0.5 text-[11px] text-[#9aa3af]"
          >
            {s.running}/{s.total} running
          </span>
          <div class="ml-auto flex items-center gap-1.5">
            <button
              class="flex items-center gap-1 rounded-md border border-[#238636]/50 bg-[#2386361a] px-2.5 py-[5px] text-[12px] text-[#5ad17a] transition-colors hover:not-disabled:bg-[#23863626] disabled:cursor-default disabled:opacity-40"
              disabled={busy || allRunning}
              onclick={() => act("start", s)}
            >
              <Play size={13} aria-hidden="true" /> Start
            </button>
            <button
              class="flex items-center gap-1 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
              disabled={busy || s.running === 0}
              onclick={() => act("stop", s)}
            >
              <Square size={13} aria-hidden="true" /> Stop
            </button>
            <button
              class="flex items-center gap-1 rounded-md border border-[#262b34] bg-[#21262d] px-2.5 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:not-disabled:bg-[#2b3138] disabled:cursor-default disabled:opacity-40"
              disabled={busy || s.running === 0}
              onclick={() => act("restart", s)}
            >
              <RotateCw size={13} aria-hidden="true" /> Restart
            </button>
          </div>
        </header>
        <ul class="divide-y divide-[#1f242c]">
          {#each s.containers as c (c.id)}
            <li class="flex items-center gap-2.5 px-3.5 py-2 text-[13px]">
              <span
                class="h-2 w-2 flex-none rounded-full {c.running
                  ? 'bg-[#3fb950]'
                  : 'bg-[#6e7681]'}"
                title={c.state || 'unknown'}
                aria-hidden="true"
              ></span>
              <span class="font-medium text-[#e6e8eb]" title={c.shortId}>{c.name}</span>
              <span class="font-mono-app truncate text-[12px] text-[#9aa3af]" title={c.image}>
                {c.image}
              </span>
              {#if c.ports.length}
                <span class="ml-auto flex items-center gap-2 text-[12px]">
                  {#each c.ports as p, i (i)}
                    {#if p.url}
                      <a
                        class="flex items-center gap-1 text-[#2f81f7] hover:underline"
                        href={p.url}
                        target="_blank"
                        rel="noreferrer"
                        title={`Open ${p.url}`}
                      >
                        {p.host}:{p.container}/{p.proto}
                        <ExternalLink size={12} aria-hidden="true" />
                      </a>
                    {:else}
                      <span class="text-[#9aa3af]">{p.host}:{p.container}/{p.proto}</span>
                    {/if}
                  {/each}
                </span>
              {:else}
                <span class="ml-auto text-[12px] text-[#6e7681]">{c.status || c.state}</span>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
{/if}

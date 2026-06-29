<script lang="ts">
  // Networks view. Owns its own fetch lifecycle: loads on mount, when the engine
  // becomes running, and on an explicit refresh request from the parent. Talks to
  // the backend only through networksApi.ts. Svelte 5 runes API.
  import Network from "@lucide/svelte/icons/network";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Plus from "@lucide/svelte/icons/plus";
  import Link from "@lucide/svelte/icons/link";
  import { errText } from "./api";
  import * as net from "./networksApi";
  import type { EngineState } from "./types";

  interface Props {
    engineState?: EngineState;
    // Monotonically increasing token; bump it from the parent to force a reload.
    refreshKey?: number;
  }

  let { engineState = "unknown", refreshKey = 0 }: Props = $props();

  let networks = $state<net.NetworkDto[]>([]);
  let errorMsg = $state("");
  let loading = $state(false);

  // Per-action busy tracking, keyed by network id (or a sentinel for global ops).
  let pending = $state<Set<string>>(new Set());
  // Currently expanded inspect row -> pretty JSON (null while loading that row).
  let inspectId = $state<string | null>(null);
  let inspectJson = $state<string>("");

  // Create form state.
  let newName = $state("");
  let newDriver = $state("bridge");
  let newInternal = $state(false);

  // Prune result line.
  let pruneMsg = $state("");

  let busy = false; // non-reactive guard against overlapping loads

  function setPending(key: string, on: boolean) {
    const next = new Set(pending);
    if (on) next.add(key);
    else next.delete(key);
    pending = next;
  }

  async function load() {
    if (engineState !== "running") {
      networks = [];
      if (engineState === "stopped") {
        errorMsg = "Engine is stopped. Start the engine to see networks.";
      } else if (engineState === "not-provisioned") {
        errorMsg = "Engine is not provisioned. Set up the engine first.";
      } else {
        errorMsg = "";
      }
      return;
    }
    if (busy) return;
    busy = true;
    loading = true;
    try {
      const raw = await net.networkList();
      const list = Array.isArray(raw) ? raw : [];
      // Built-in networks first, then alphabetical.
      list.sort((a, b) => {
        if (a.builtin !== b.builtin) return a.builtin ? -1 : 1;
        return (a.name ?? "").localeCompare(b.name ?? "");
      });
      networks = list;
      errorMsg = "";
    } catch (e) {
      errorMsg = `Failed to load networks: ${errText(e)}`;
    } finally {
      loading = false;
      busy = false;
    }
  }

  // Reload on mount and whenever the engine state or the refresh token changes.
  $effect(() => {
    void engineState;
    void refreshKey;
    load();
  });

  async function createNetwork(e: SubmitEvent) {
    e.preventDefault();
    const name = newName.trim();
    if (!name) {
      errorMsg = "Network name is required.";
      return;
    }
    setPending("__create__", true);
    errorMsg = "";
    try {
      await net.networkCreate(name, newDriver, newInternal);
      newName = "";
      newInternal = false;
      newDriver = "bridge";
      await load();
    } catch (err) {
      errorMsg = `Failed to create network: ${errText(err)}`;
    } finally {
      setPending("__create__", false);
    }
  }

  async function removeNetwork(n: net.NetworkDto) {
    if (n.builtin) return;
    if (!confirm(`Remove network "${n.name}"? This cannot be undone.`)) return;
    setPending(n.id, true);
    errorMsg = "";
    try {
      await net.networkRemove(n.id);
      if (inspectId === n.id) {
        inspectId = null;
        inspectJson = "";
      }
      await load();
    } catch (err) {
      errorMsg = `Failed to remove network: ${errText(err)}`;
    } finally {
      setPending(n.id, false);
    }
  }

  async function toggleInspect(n: net.NetworkDto) {
    if (inspectId === n.id) {
      inspectId = null;
      inspectJson = "";
      return;
    }
    inspectId = n.id;
    inspectJson = "";
    setPending(`inspect:${n.id}`, true);
    errorMsg = "";
    try {
      inspectJson = await net.networkInspect(n.id);
    } catch (err) {
      errorMsg = `Failed to inspect network: ${errText(err)}`;
      inspectId = null;
    } finally {
      setPending(`inspect:${n.id}`, false);
    }
  }

  async function pruneNetworks() {
    if (!confirm("Remove all unused networks?")) return;
    setPending("__prune__", true);
    errorMsg = "";
    pruneMsg = "";
    try {
      const res = await net.networkPrune();
      const removed = res?.removed ?? [];
      pruneMsg = removed.length
        ? `Removed ${removed.length} network${removed.length === 1 ? "" : "s"}: ${removed.join(", ")}`
        : "No unused networks to remove.";
      await load();
    } catch (err) {
      errorMsg = `Failed to prune networks: ${errText(err)}`;
    } finally {
      setPending("__prune__", false);
    }
  }

  const BTN_BASE =
    "inline-flex cursor-pointer items-center gap-1 rounded-md border px-2 py-[3px] text-xs transition-colors disabled:cursor-default disabled:opacity-45";
  const creating = $derived(pending.has("__create__"));
  const pruning = $derived(pending.has("__prune__"));
</script>

<section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
  <div class="flex items-baseline gap-2.5 border-b border-[#262b34] px-3.5 py-3">
    <h2 class="text-sm font-semibold">Networks</h2>
    <span class="text-xs text-[#9aa3af]">
      {networks.length ? `${networks.length} total` : ""}
    </span>
    <div class="ml-auto">
      <button
        class="{BTN_BASE} border-[#f8514980] text-[#f85149] hover:not-disabled:bg-[#f851491f]"
        disabled={pruning || engineState !== "running"}
        onclick={pruneNetworks}
        ><Trash2 size={13} aria-hidden="true" />Prune unused</button
      >
    </div>
  </div>

  {#if errorMsg}
    <div
      class="mx-3.5 mt-3 select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
    >
      {errorMsg}
    </div>
  {/if}

  {#if pruneMsg}
    <div
      class="mx-3.5 mt-3 select-text rounded-md border border-[#262b34] bg-[#1b1f27] px-3 py-2 text-[13px] text-[#9aa3af]"
    >
      {pruneMsg}
    </div>
  {/if}

  <!-- Create network form -->
  {#if engineState === "running"}
    <form
      class="flex flex-wrap items-end gap-2.5 border-b border-[#262b34] px-3.5 py-3"
      onsubmit={createNetwork}
    >
      <label class="flex flex-col gap-1 text-xs text-[#9aa3af]">
        <span>Name</span>
        <input
          class="w-44 rounded-md border border-[#262b34] bg-[#0f1217] px-2 py-[5px] text-[13px] text-[#e6e8eb] outline-none focus:border-[#2f81f7]"
          type="text"
          placeholder="my_network"
          bind:value={newName}
        />
      </label>
      <label class="flex flex-col gap-1 text-xs text-[#9aa3af]">
        <span>Driver</span>
        <select
          class="rounded-md border border-[#262b34] bg-[#0f1217] px-2 py-[5px] text-[13px] text-[#e6e8eb] outline-none focus:border-[#2f81f7]"
          bind:value={newDriver}
        >
          <option value="bridge">bridge</option>
          <option value="macvlan">macvlan</option>
        </select>
      </label>
      <label class="flex items-center gap-1.5 pb-[6px] text-[13px] text-[#e6e8eb]">
        <input type="checkbox" bind:checked={newInternal} />
        <span>Internal</span>
      </label>
      <button
        class="{BTN_BASE} mb-[1px] border-[#2f81f7] text-[#2f81f7] hover:not-disabled:bg-[#2f81f71f]"
        type="submit"
        disabled={creating}
        ><Plus size={13} aria-hidden="true" />Create network</button
      >
    </form>
  {/if}

  {#if networks.length === 0}
    <div class="flex flex-col items-center gap-2 px-3.5 py-9 text-center text-[#9aa3af]">
      <Network size={22} class="opacity-60" aria-hidden="true" />
      <span>
        {#if loading}
          Loading networks…
        {:else if engineState === "running"}
          No networks.
        {:else}
          Engine not running.
        {/if}
      </span>
    </div>
  {:else}
    <div class="overflow-x-auto">
      <table class="w-full border-collapse text-[13px]">
        <thead>
          <tr>
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Name</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Driver</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Scope</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Internal</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >#Containers</th
            >
            <th
              class="sticky top-0 w-[1%] whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Actions</th
            >
          </tr>
        </thead>
        <tbody>
          {#each networks as n (n.id)}
            {@const acting = pending.has(n.id)}
            {@const inspecting = pending.has(`inspect:${n.id}`)}
            {@const open = inspectId === n.id}
            <tr class="hover:bg-[#1b1f27] {acting ? 'opacity-60' : ''}">
              <td
                class="border-b border-[#262b34] px-3 py-2.5 align-middle font-medium"
                title={n.id}
              >
                {n.name}
                {#if n.builtin}
                  <span
                    class="ml-1.5 rounded border border-[#262b34] px-1 py-px text-[10px] uppercase tracking-wide text-[#9aa3af]"
                    >built-in</span
                  >
                {/if}
              </td>
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{n.driver || "—"}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{n.scope || "—"}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{n.internal ? "yes" : "no"}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{n.containers}</td
              >
              <td class="border-b border-[#262b34] px-3 py-2.5 align-middle">
                <div class="flex justify-end gap-1.5">
                  <button
                    class="{BTN_BASE} border-[#262b34] text-[#e6e8eb] hover:not-disabled:bg-[#21262d]"
                    disabled={inspecting}
                    onclick={() => toggleInspect(n)}
                    ><Link size={13} aria-hidden="true" />{open
                      ? "Hide"
                      : "Inspect"}</button
                  >
                  <button
                    class="{BTN_BASE} border-[#f8514980] text-[#f85149] hover:not-disabled:bg-[#f851491f]"
                    disabled={acting || n.builtin}
                    title={n.builtin
                      ? "Built-in networks cannot be removed"
                      : "Remove network"}
                    onclick={() => removeNetwork(n)}
                    ><Trash2 size={13} aria-hidden="true" />Remove</button
                  >
                </div>
              </td>
            </tr>
            {#if open}
              <tr>
                <td colspan="6" class="border-b border-[#262b34] bg-[#0f1217] px-3 py-2.5">
                  {#if inspecting}
                    <div class="text-[13px] text-[#9aa3af]">Loading…</div>
                  {:else}
                    <pre
                      class="font-mono-app max-h-80 select-text overflow-auto whitespace-pre text-xs text-[#9aa3af]">{inspectJson}</pre>
                  {/if}
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

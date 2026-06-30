<script lang="ts">
  // Networks view. Owns its own fetch lifecycle: loads on mount, when the engine
  // becomes running, and on an explicit refresh request from the parent. Talks to
  // the backend only through networksApi.ts. Svelte 5 runes API.
  import Network from "@lucide/svelte/icons/network";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";
  import Braces from "@lucide/svelte/icons/braces";
  import Eraser from "@lucide/svelte/icons/eraser";
  import X from "@lucide/svelte/icons/x";
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

  // Client-side name filter.
  let query = $state("");

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

  // Short id for the secondary line under the network name.
  function shortId(id: string): string {
    return (id ?? "").replace(/^sha256:/, "").slice(0, 12);
  }

  const creating = $derived(pending.has("__create__"));
  const pruning = $derived(pending.has("__prune__"));
  const filtered = $derived(
    query.trim()
      ? networks.filter((n) =>
          (n.name ?? "").toLowerCase().includes(query.trim().toLowerCase())
        )
      : networks
  );

  const COLS = "minmax(200px,1.7fr) 0.9fr 0.8fr 0.8fr 0.7fr";
</script>

<section class="page netview">
  <div class="head">
    <h1>Networks</h1>
    <span class="chip"><b class="num">{networks.length}</b> total</span>
    <span class="sp"></span>
    <div class="search">
      <Search aria-hidden="true" />
      <input
        type="text"
        placeholder="Filter networks"
        bind:value={query}
        disabled={engineState !== "running"}
        aria-label="Filter networks by name"
      />
    </div>
    <button
      class="btn btn-danger"
      disabled={pruning || engineState !== "running"}
      onclick={pruneNetworks}
      title="Remove all unused networks"
    >
      <Eraser aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune unused"}
    </button>
  </div>

  {#if errorMsg}
    <div class="banner err">{errorMsg}</div>
  {/if}

  {#if pruneMsg}
    <div class="banner">{pruneMsg}</div>
  {/if}

  <!-- Create network form -->
  {#if engineState === "running"}
    <form class="card card-pad createbar" onsubmit={createNetwork}>
      <label class="fieldcol">
        <span class="flabel">Name</span>
        <span class="search inputwrap">
          <input type="text" placeholder="my_network" bind:value={newName} />
        </span>
      </label>
      <label class="fieldcol">
        <span class="flabel">Driver</span>
        <span class="search inputwrap selwrap">
          <select bind:value={newDriver}>
            <option value="bridge">bridge</option>
            <option value="macvlan">macvlan</option>
          </select>
        </span>
      </label>
      <label class="field internalbox">
        <input type="checkbox" bind:checked={newInternal} />
        <span>Internal</span>
      </label>
      <button class="btn btn-pri" type="submit" disabled={creating}>
        <Plus aria-hidden="true" />
        {creating ? "Creating…" : "Create network"}
      </button>
    </form>
  {/if}

  {#if filtered.length === 0}
    <div class="table">
      <div class="empty">
        {#if loading}
          Loading networks…
        {:else if engineState !== "running"}
          Engine not running.
        {:else if query.trim()}
          No networks match “{query.trim()}”.
        {:else}
          No networks.
        {/if}
      </div>
    </div>
  {:else}
    <div class="table">
      <div class="thead" style="--cols:{COLS}">
        <span>Name</span>
        <span>Driver</span>
        <span>Scope</span>
        <span>Internal</span>
        <span>Containers</span>
      </div>

      {#each filtered as n (n.id)}
        {@const acting = pending.has(n.id)}
        {@const inspecting = pending.has(`inspect:${n.id}`)}
        {@const open = inspectId === n.id}
        <div
          class="trow"
          class:sel={open}
          style="--cols:{COLS}; {acting ? 'opacity:.55' : ''}"
        >
          <div class="cell-name">
            <span class="lamp" class:run={n.containers > 0}></span>
            <span class="av"><Network aria-hidden="true" /></span>
            <div style="min-width:0">
              <div class="nm-line">
                <span class="nm" title={n.name}>{n.name}</span>
                {#if n.builtin}
                  <span class="tag">built-in</span>
                {/if}
              </div>
              <div class="id" title={n.id}>{shortId(n.id) || "—"}</div>
            </div>
          </div>

          <span class="cell-text">{n.driver || "—"}</span>
          <span class="cell-dim">{n.scope || "—"}</span>
          <span>
            {#if n.internal}
              <span class="tag">yes</span>
            {:else}
              <span class="muted">no</span>
            {/if}
          </span>
          <span class="num cell-text">{n.containers}</span>

          <div class="rowact">
            <button
              title={open ? "Hide inspect" : "Inspect"}
              disabled={inspecting}
              onclick={() => toggleInspect(n)}
            >
              {#if open}<X aria-hidden="true" />{:else}<Braces aria-hidden="true" />{/if}
            </button>
            {#if !n.builtin}
              <button
                class="dng"
                title="Remove network"
                disabled={acting}
                onclick={() => removeNetwork(n)}
              >
                <Trash2 aria-hidden="true" />
              </button>
            {/if}
          </div>
        </div>

        {#if open}
          <div class="inspect-pane">
            <div class="outpane">
              <div class="bar">
                <Search aria-hidden="true" />
                <span>Inspect · <span class="mono">{n.name}</span></span>
              </div>
              {#if inspecting}
                <div class="body-out">Loading…</div>
              {:else}
                <pre class="body-out">{inspectJson}</pre>
              {/if}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  /* Create bar: reuse foundation surfaces; only layout lives here. */
  .createbar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 12px;
  }
  .fieldcol {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .flabel {
    font-size: 10.5px;
    font-weight: 650;
    letter-spacing: 0.7px;
    text-transform: uppercase;
    color: var(--text-4);
  }
  /* Reuse .search chrome as a text/select input shell. */
  .inputwrap {
    width: 200px;
    padding: 6px 11px;
  }
  .selwrap {
    width: 150px;
  }
  .inputwrap select {
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    outline: none;
    width: 100%;
    cursor: pointer;
  }
  .inputwrap select option {
    background: var(--s2);
    color: var(--text);
  }
  .internalbox {
    padding-bottom: 7px;
  }
  .createbar .btn-pri {
    margin-left: auto;
  }

  /* Plain table cell text using tokens (no raw colors). */
  .cell-text {
    color: var(--text-2);
    font-size: 13px;
  }
  .cell-dim {
    color: var(--text-3);
    font-size: 13px;
  }

  .nm-line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .nm-line .nm {
    min-width: 0;
  }
  .nm-line .tag {
    flex: none;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    color: var(--text-3);
  }

  /* Full-width inspect drawer sits between grid rows inside .table. */
  .inspect-pane {
    padding: 0 18px 14px;
    border-bottom: 1px solid var(--line-soft);
  }
  .inspect-pane .body-out {
    white-space: pre;
    max-height: 20rem;
    -webkit-user-select: text;
    user-select: text;
  }
</style>

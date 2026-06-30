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
  import { errText } from "../api";
  import * as net from "../api/networks";
  import type { EngineState } from "../types";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { confirmDialog } from "../state/confirm.svelte.js";

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
    if (
      !(await confirmDialog({
        title: "Remove network?",
        description: `Remove network "${n.name}"? This cannot be undone.`,
        destructive: true,
        confirmText: "Remove",
      }))
    )
      return;
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
    if (
      !(await confirmDialog({
        title: "Prune unused networks?",
        description: "Remove all unused networks?",
        destructive: true,
        confirmText: "Prune",
      }))
    )
      return;
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
</script>

<section class="page netview">
  <div class="head">
    <h1>Networks</h1>
    <Badge variant="secondary" class="gap-1.5 font-normal"
      ><b class="num">{networks.length}</b> total</Badge
    >
    <span class="sp"></span>
    <div class="relative w-[220px]">
      <Search
        class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
        aria-hidden="true"
      />
      <Input
        class="pl-8"
        placeholder="Filter networks"
        bind:value={query}
        disabled={engineState !== "running"}
        aria-label="Filter networks by name"
      />
    </div>
    <Button
      variant="destructive"
      disabled={pruning || engineState !== "running"}
      onclick={pruneNetworks}
      title="Remove all unused networks"
    >
      <Eraser aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune unused"}
    </Button>
  </div>

  {#if errorMsg}
    <Alert.Root variant="destructive">
      <X aria-hidden="true" />
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {/if}

  {#if pruneMsg}
    <Alert.Root>
      <Eraser aria-hidden="true" />
      <Alert.Description>{pruneMsg}</Alert.Description>
    </Alert.Root>
  {/if}

  <!-- Create network form -->
  {#if engineState === "running"}
    <form class="card card-pad createbar" onsubmit={createNetwork}>
      <label class="fieldcol">
        <span class="flabel">Name</span>
        <Input class="w-[200px]" placeholder="my_network" bind:value={newName} />
      </label>
      <div class="fieldcol">
        <span class="flabel">Driver</span>
        <Select.Root type="single" bind:value={newDriver}>
          <Select.Trigger class="w-[150px]">{newDriver}</Select.Trigger>
          <Select.Content>
            <Select.Item value="bridge" label="bridge">bridge</Select.Item>
            <Select.Item value="macvlan" label="macvlan">macvlan</Select.Item>
          </Select.Content>
        </Select.Root>
      </div>
      <div class="field internalbox">
        <Checkbox id="net-internal" bind:checked={newInternal} />
        <Label for="net-internal">Internal</Label>
      </div>
      <Button type="submit" disabled={creating} class="ml-auto">
        <Plus aria-hidden="true" />
        {creating ? "Creating…" : "Create network"}
      </Button>
    </form>
  {/if}

  <div class="card overflow-hidden">
    <Table.Root class="table-fixed">
      <Table.Header>
        <Table.Row class="hover:bg-transparent">
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:32%">Name</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:16%">Driver</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:15%">Scope</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:15%">Internal</Table.Head
          >
          <Table.Head
            class="h-9 text-[10.5px] font-semibold uppercase tracking-wider"
            style="width:12%">Containers</Table.Head
          >
          <Table.Head class="h-9" style="width:10%"></Table.Head>
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#if filtered.length === 0}
          <Table.Row class="hover:bg-transparent">
            <Table.Cell colspan={6} class="py-7 text-center text-muted-foreground">
              {#if loading}
                Loading networks…
              {:else if engineState !== "running"}
                Engine not running.
              {:else if query.trim()}
                No networks match “{query.trim()}”.
              {:else}
                No networks.
              {/if}
            </Table.Cell>
          </Table.Row>
        {:else}
          {#each filtered as n (n.id)}
            {@const acting = pending.has(n.id)}
            {@const inspecting = pending.has(`inspect:${n.id}`)}
            {@const open = inspectId === n.id}
            <Table.Row
              class="group relative data-[sel=true]:bg-muted data-[sel=true]:shadow-[inset_2px_0_0_var(--lime)]"
              data-sel={open}
              style={acting ? "opacity:.55" : undefined}
              aria-busy={acting}
            >
              <Table.Cell>
                <div class="cell-name">
                  <span class="lamp" class:run={n.containers > 0}></span>
                  <span class="av"><Network aria-hidden="true" /></span>
                  <div style="min-width:0">
                    <div class="nm-line">
                      <span class="nm" title={n.name}>{n.name}</span>
                      {#if n.builtin}
                        <Badge variant="outline" class="font-normal">built-in</Badge>
                      {/if}
                    </div>
                    <div class="id" title={n.id}>{shortId(n.id) || "—"}</div>
                  </div>
                </div>
              </Table.Cell>

              <Table.Cell class="text-2 text-[13px]">{n.driver || "—"}</Table.Cell>
              <Table.Cell class="text-3 text-[13px]">{n.scope || "—"}</Table.Cell>
              <Table.Cell>
                {#if n.internal}
                  <Badge variant="outline" class="font-normal">yes</Badge>
                {:else}
                  <span class="muted">no</span>
                {/if}
              </Table.Cell>
              <Table.Cell class="num text-2 text-[13px]">{n.containers}</Table.Cell>

              <Table.Cell class="text-right">
                <div
                  class="inline-flex justify-end gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 data-[sel=true]:opacity-100"
                  data-sel={open}
                >
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    title={open ? "Hide inspect" : "Inspect"}
                    disabled={inspecting}
                    onclick={() => toggleInspect(n)}
                  >
                    {#if open}<X aria-hidden="true" />{:else}<Braces
                        aria-hidden="true"
                      />{/if}
                  </Button>
                  {#if !n.builtin}
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      class="text-muted-foreground hover:text-destructive"
                      title="Remove network"
                      disabled={acting}
                      onclick={() => removeNetwork(n)}
                    >
                      <Trash2 aria-hidden="true" />
                    </Button>
                  {/if}
                </div>
              </Table.Cell>
            </Table.Row>

            {#if open}
              <Table.Row class="hover:bg-transparent">
                <Table.Cell colspan={6} class="p-0">
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
                </Table.Cell>
              </Table.Row>
            {/if}
          {/each}
        {/if}
      </Table.Body>
    </Table.Root>
  </div>
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
  .internalbox {
    padding-bottom: 7px;
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

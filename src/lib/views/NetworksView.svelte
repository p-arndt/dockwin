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

<section class="flex flex-col gap-[16px] pt-[18px] px-[22px] pb-[24px] min-w-0">
  <div class="flex items-end gap-[14px] pt-[22px] px-[22px] pb-[16px] shrink-0">
    <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">Networks</h1>
    <Badge variant="secondary" class="gap-1.5 font-normal"
      ><b class="tabular-nums">{networks.length}</b> total</Badge
    >
    <span class="flex-1"></span>
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
    <form
      class="bg-card border border-border rounded-[11px] shadow-sm px-[18px] py-[16px] flex flex-wrap items-end gap-[12px]"
      onsubmit={createNetwork}
    >
      <label class="flex flex-col gap-[6px]">
        <span
          class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70"
          >Name</span
        >
        <Input class="w-[200px]" placeholder="my_network" bind:value={newName} />
      </label>
      <div class="flex flex-col gap-[6px]">
        <span
          class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70"
          >Driver</span
        >
        <Select.Root type="single" bind:value={newDriver}>
          <Select.Trigger class="w-[150px]">{newDriver}</Select.Trigger>
          <Select.Content>
            <Select.Item value="bridge" label="bridge">bridge</Select.Item>
            <Select.Item value="macvlan" label="macvlan">macvlan</Select.Item>
          </Select.Content>
        </Select.Root>
      </div>
      <div class="flex items-center gap-[9px] text-[13px] text-muted-foreground pb-[7px]">
        <Checkbox id="net-internal" bind:checked={newInternal} />
        <Label for="net-internal">Internal</Label>
      </div>
      <Button type="submit" disabled={creating} class="ml-auto">
        <Plus aria-hidden="true" />
        {creating ? "Creating…" : "Create network"}
      </Button>
    </form>
  {/if}

  <div class="bg-card border border-border rounded-[11px] shadow-sm overflow-hidden">
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
                <div class="flex items-center gap-[12px] min-w-0">
                  <span
                    class="w-[7px] h-[7px] rounded-full shrink-0 {n.containers > 0
                      ? 'bg-chart-2'
                      : 'bg-chart-5'}"
                  ></span>
                  <span
                    class="size-[30px] rounded-[8px] shrink-0 grid place-items-center bg-muted border border-border text-muted-foreground"
                    ><Network class="size-[15px]" aria-hidden="true" /></span
                  >
                  <div class="min-w-0">
                    <div class="flex items-center gap-[8px] min-w-0">
                      <span
                        class="font-semibold text-[13.5px] text-foreground tracking-[-0.1px] leading-[1.25] truncate min-w-0"
                        title={n.name}>{n.name}</span
                      >
                      {#if n.builtin}
                        <Badge variant="outline" class="font-normal">built-in</Badge>
                      {/if}
                    </div>
                    <div
                      class="font-mono text-[11px] text-muted-foreground/70 leading-[1.3]"
                      title={n.id}
                    >
                      {shortId(n.id) || "—"}
                    </div>
                  </div>
                </div>
              </Table.Cell>

              <Table.Cell class="text-muted-foreground text-[13px]">{n.driver || "—"}</Table.Cell>
              <Table.Cell class="text-muted-foreground text-[13px]">{n.scope || "—"}</Table.Cell>
              <Table.Cell>
                {#if n.internal}
                  <Badge variant="outline" class="font-normal">yes</Badge>
                {:else}
                  <span class="text-muted-foreground/70">no</span>
                {/if}
              </Table.Cell>
              <Table.Cell class="tabular-nums text-muted-foreground text-[13px]">{n.containers}</Table.Cell>

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
                  <div class="px-[18px] pb-[14px] border-b border-border">
                    <div class="border border-border rounded-[9px] bg-background overflow-hidden">
                      <div
                        class="flex items-center gap-[8px] bg-muted border-b border-border px-[12px] py-[8px] text-[12px] text-muted-foreground"
                      >
                        <Search aria-hidden="true" />
                        <span
                          >Inspect · <span class="font-mono tabular-nums">{n.name}</span></span
                        >
                      </div>
                      {#if inspecting}
                        <div
                          class="max-h-[20rem] overflow-auto px-[12px] py-[10px] font-mono text-[11.5px] leading-[1.55] text-muted-foreground select-text whitespace-pre"
                        >
                          Loading…
                        </div>
                      {:else}
                        <pre
                          class="max-h-[20rem] overflow-auto px-[12px] py-[10px] font-mono text-[11.5px] leading-[1.55] text-muted-foreground select-text whitespace-pre">{inspectJson}</pre>
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

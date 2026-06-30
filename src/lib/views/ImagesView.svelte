<script lang="ts">
  // Rich images view: pull-with-progress, list, and per-image actions (remove,
  // tag, history, inspect) plus a prune control. Owns its own fetch lifecycle
  // (loads on mount, when the engine becomes running, and on a parent refresh
  // token). Talks to the backend only through imagesApi.ts.
  //
  // Presentation: crafted v2 language (foundation contract) — layered surfaces,
  // a single lime primary (Pull), quiet status, a right-hand detail drawer with
  // Overview / History / Inspect tabs. No raw hex; tokens + component classes.
  import { onMount } from "svelte";
  import Layers from "@lucide/svelte/icons/layers";
  import Download from "@lucide/svelte/icons/download";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Tag from "@lucide/svelte/icons/tag";
  import History from "@lucide/svelte/icons/history";
  import Search from "@lucide/svelte/icons/search";
  import Info from "@lucide/svelte/icons/info";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import Copy from "@lucide/svelte/icons/copy";
  import Boxes from "@lucide/svelte/icons/boxes";
  import * as imagesApi from "../api/images";
  import type { ImageLayer } from "../api/images";
  import type { EngineState, ImageDto } from "../types";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Alert from "$lib/components/ui/alert/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { confirmDialog } from "../state/confirm.svelte.js";

  interface Props {
    engineState?: EngineState;
    // Monotonically increasing token; bump it from the parent to force a reload.
    refreshKey?: number;
  }

  let { engineState = "unknown", refreshKey = 0 }: Props = $props();

  // --- list state ---
  let images = $state<ImageDto[]>([]);
  let errorMsg = $state("");
  let loading = $state(false);
  let filter = $state("");
  let busy = false; // non-reactive guard against overlapping loads

  // --- pull state ---
  let pullRef = $state("");
  let pulling = $state(false);
  let pullStatus = $state("");
  let pullProgress = $state("");

  // --- per-row action state ---
  let pending = $state<Set<string>>(new Set()); // image ids with an in-flight action

  // --- detail drawer (one image at a time): overview / history / inspect ---
  type DetailTab = "overview" | "history" | "inspect";
  let selectedId = $state<string | null>(null);
  let detailTab = $state<DetailTab>("overview");
  let detailLoading = $state(false);
  let detailError = $state("");
  let historyData = $state<ImageLayer[]>([]);
  let inspectData = $state("");
  // tag editor inputs
  let tagRepo = $state("");
  let tagTag = $state("latest");

  // --- prune state ---
  let pruneAll = $state(false);
  let pruning = $state(false);
  let pruneResult = $state("");

  // Filtered + sorted view of the images.
  let shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    let list = images;
    if (q) {
      list = list.filter(
        (img) =>
          repoTag(img).toLowerCase().includes(q) ||
          imagesApi.shortId(img.id).includes(q)
      );
    }
    return list;
  });

  // The currently selected image object (or null when nothing/gone).
  let selected = $derived(
    selectedId ? (images.find((i) => i.id === selectedId) ?? null) : null
  );

  // Total on-disk size across all images (informational; may double-count
  // shared layers, same as `docker images` reports).
  let totalSize = $derived(
    images.reduce((sum, img) => sum + (img.size || 0), 0)
  );

  async function load() {
    if (engineState !== "running") {
      images = [];
      if (engineState === "stopped") {
        errorMsg = "Engine is stopped. Start the engine to see images.";
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
      const raw = await imagesApi.imageList(true);
      const list = Array.isArray(raw) ? raw : [];
      list.sort((a, b) => (b.created ?? 0) - (a.created ?? 0));
      images = list;
      errorMsg = "";
    } catch (e) {
      errorMsg = `Failed to load images: ${imagesApi.errText(e)}`;
    } finally {
      loading = false;
      busy = false;
    }
  }

  // Reload on mount and whenever engine state / refresh token change.
  $effect(() => {
    void engineState;
    void refreshKey;
    load();
  });

  // Subscribe to live pull progress.
  onMount(() => {
    let unlisten: (() => void) | undefined;
    imagesApi
      .onImagePull((p) => {
        if (p.error) {
          pullStatus = p.error;
          return;
        }
        if (p.status) pullStatus = p.id ? `${p.status} ${p.id}` : p.status;
        pullProgress = p.progress ?? "";
      })
      .then((u) => (unlisten = u))
      .catch(() => {});
    return () => {
      try {
        unlisten?.();
      } catch {
        /* ignore */
      }
    };
  });

  // --- pull ---
  async function doPull() {
    const ref = pullRef.trim();
    if (!ref || pulling || engineState !== "running") return;
    pulling = true;
    pullStatus = `Pulling ${ref}…`;
    pullProgress = "";
    errorMsg = "";
    try {
      await imagesApi.imagePull(ref);
      pullStatus = `Pulled ${ref}.`;
      pullProgress = "";
      pullRef = "";
      await load();
    } catch (e) {
      errorMsg = `Pull failed: ${imagesApi.errText(e)}`;
      pullStatus = "";
      pullProgress = "";
    } finally {
      pulling = false;
    }
  }

  function onPullKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") doPull();
  }

  // --- per-row action helpers ---
  function markPending(id: string, on: boolean) {
    const next = new Set(pending);
    if (on) next.add(id);
    else next.delete(id);
    pending = next;
  }

  async function doRemove(img: ImageDto) {
    if (pending.has(img.id)) return;
    const label = repoTag(img);
    if (
      !(await confirmDialog({
        title: "Remove image?",
        description: `Remove image "${label}"?`,
        destructive: true,
        confirmText: "Remove",
      }))
    )
      return;
    markPending(img.id, true);
    errorMsg = "";
    try {
      await imagesApi.imageRemove(img.id, false);
      if (selectedId === img.id) closeDetail();
      await load();
    } catch (e) {
      // Likely in use or has multiple tags — offer a force removal.
      if (
        await confirmDialog({
          title: "Force remove image?",
          description: `Remove failed: ${imagesApi.errText(e)}\n\nForce remove "${label}"?`,
          destructive: true,
          confirmText: "Force remove",
        })
      ) {
        try {
          await imagesApi.imageRemove(img.id, true);
          if (selectedId === img.id) closeDetail();
          await load();
        } catch (e2) {
          errorMsg = `Force remove failed: ${imagesApi.errText(e2)}`;
        }
      }
    } finally {
      markPending(img.id, false);
    }
  }

  // --- detail drawer ---
  function closeDetail() {
    selectedId = null;
    detailTab = "overview";
    detailError = "";
    historyData = [];
    inspectData = "";
  }

  // Select an image and open the drawer on a given tab (loading lazily).
  function selectImage(img: ImageDto, tab: DetailTab = "overview") {
    selectedId = img.id;
    detailError = "";
    // Seed the tag editor from the first tag if present.
    const first = (img.tags ?? []).find((t) => t && t !== "<none>:<none>");
    if (first && first.includes(":")) {
      const idx = first.lastIndexOf(":");
      tagRepo = first.slice(0, idx);
      tagTag = first.slice(idx + 1) || "latest";
    } else {
      tagRepo = first ?? "";
      tagTag = "latest";
    }
    setTab(tab);
  }

  function setTab(tab: DetailTab) {
    detailTab = tab;
    detailError = "";
    if (!selectedId) return;
    if (tab === "history") void loadHistory();
    else if (tab === "inspect") void loadInspect();
  }

  async function loadHistory() {
    const id = selectedId;
    if (!id) return;
    historyData = [];
    detailLoading = true;
    detailError = "";
    try {
      historyData = await imagesApi.imageHistory(id);
    } catch (e) {
      detailError = imagesApi.errText(e);
    } finally {
      detailLoading = false;
    }
  }

  async function loadInspect() {
    const id = selectedId;
    if (!id) return;
    inspectData = "";
    detailLoading = true;
    detailError = "";
    try {
      inspectData = await imagesApi.imageInspect(id);
    } catch (e) {
      detailError = imagesApi.errText(e);
    } finally {
      detailLoading = false;
    }
  }

  async function applyTag() {
    if (!selectedId) return;
    const repo = tagRepo.trim();
    const tag = tagTag.trim() || "latest";
    if (!repo) {
      detailError = "Repository is required.";
      return;
    }
    detailLoading = true;
    detailError = "";
    try {
      await imagesApi.imageTag(selectedId, repo, tag);
      await load();
    } catch (e) {
      detailError = imagesApi.errText(e);
    } finally {
      detailLoading = false;
    }
  }

  // --- prune ---
  async function doPrune() {
    if (pruning || engineState !== "running") return;
    const note = pruneAll
      ? "Remove ALL unused images (not just dangling)? This cannot be undone."
      : "Remove dangling (untagged) images? This cannot be undone.";
    if (
      !(await confirmDialog({
        title: "Prune images?",
        description: note,
        destructive: true,
        confirmText: "Prune",
      }))
    )
      return;
    pruning = true;
    pruneResult = "";
    errorMsg = "";
    try {
      const res = await imagesApi.imagePrune(pruneAll);
      pruneResult = `Removed ${res.images_deleted} image${
        res.images_deleted === 1 ? "" : "s"
      }, reclaimed ${imagesApi.humanBytes(res.space_reclaimed)}.`;
      await load();
    } catch (e) {
      errorMsg = `Prune failed: ${imagesApi.errText(e)}`;
    } finally {
      pruning = false;
    }
  }

  // --- pure formatting helpers ---
  function cleanTags(img: ImageDto): string[] {
    return (img.tags ?? []).filter((t) => t && t !== "<none>:<none>");
  }

  function repoTag(img: ImageDto): string {
    const tags = cleanTags(img);
    return tags.length ? tags.join(", ") : "<none>";
  }

  // The first usable tag, used as the loud row/detail name.
  function primaryTag(img: ImageDto): string {
    const tags = cleanTags(img);
    return tags.length ? tags[0] : "<none>";
  }

  function isDangling(img: ImageDto): boolean {
    return cleanTags(img).length === 0;
  }

  // Heuristic: Docker Library "official" images have a single-segment repo with
  // no registry host or user namespace (e.g. nginx, redis:7) — purely a label.
  function isOfficial(img: ImageDto): boolean {
    const first = cleanTags(img)[0];
    if (!first) return false;
    const repo = first.slice(0, first.lastIndexOf(":") >= 0 ? first.lastIndexOf(":") : first.length);
    return repo.length > 0 && !repo.includes("/") && !repo.includes(".");
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      /* ignore — clipboard may be unavailable */
    }
  }

</script>

<div class="flex flex-col gap-[16px] min-w-0 pt-[18px] px-[22px] pb-[24px]">
  <!-- ===== Page header ===== -->
  <div class="flex items-end gap-[14px] pt-[22px] px-[22px] pb-[16px] shrink-0">
    <h1 class="text-[23px] font-[680] tracking-[-0.5px] leading-none">Images</h1>
    <Badge variant="secondary" class="gap-1.5 font-normal">
      <b class="tabular-nums">{images.length}</b> images
      {#if totalSize > 0}
        <span class="text-muted-foreground/70">·</span>
        <b class="tabular-nums">{imagesApi.humanBytes(totalSize)}</b>
      {/if}
    </Badge>
    <span class="flex-1"></span>
    <div class="relative w-[220px]">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
      <Input class="pl-8" placeholder="Filter images…" bind:value={filter} aria-label="Filter images" />
    </div>
  </div>

  <!-- ===== Toolbar: pull (primary) + prune ===== -->
  <div class="flex items-center gap-[10px] flex-wrap">
    <div class="flex-1 min-w-[240px] max-w-[520px] relative">
      <Download class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
      <Input
        class="pl-8"
        placeholder="Pull an image, e.g. nginx:latest"
        bind:value={pullRef}
        disabled={pulling || engineState !== "running"}
        onkeydown={onPullKeydown}
        aria-label="Pull an image"
      />
    </div>
    <Button
      disabled={pulling || engineState !== "running" || !pullRef.trim()}
      onclick={doPull}
    >
      <Download aria-hidden="true" />
      {pulling ? "Pulling…" : "Pull"}
    </Button>

    <span class="flex-1"></span>

    <div class="flex items-center gap-[9px] text-[13px] text-muted-foreground">
      <Checkbox id="prune-all" bind:checked={pruneAll} disabled={pruning} />
      <Label for="prune-all">All unused</Label>
    </div>
    <Button
      variant="destructive"
      disabled={pruning || engineState !== "running"}
      onclick={doPrune}
    >
      <Trash2 aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune"}
    </Button>
  </div>

  <!-- Pull progress / status -->
  {#if pulling || pullStatus}
    <div class="flex flex-col gap-[7px]">
      {#if pulling}
        <div class="h-[10px] w-full rounded-full bg-muted overflow-hidden relative"><i class="progress-fill" style="width:100%"></i></div>
      {/if}
      <p class="text-[12px] text-muted-foreground truncate font-mono tabular-nums" title={pullStatus}>
        {pullStatus}{pullProgress ? `  ${pullProgress}` : ""}
      </p>
    </div>
  {/if}

  {#if pruneResult}
    <p class="text-[12px] text-muted-foreground truncate">{pruneResult}</p>
  {/if}

  {#if errorMsg}
    <Alert.Root variant="destructive">
      <Info aria-hidden="true" />
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {/if}

  <!-- ===== List + detail drawer ===== -->
  <div class="grid gap-[16px] items-start min-w-0 {selected ? 'grid-cols-[minmax(0,1fr)_clamp(340px,34%,392px)]' : 'grid-cols-[minmax(0,1fr)]'}">
    <div class="bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] overflow-hidden">
      <Table.Root class="table-fixed">
        <Table.Header>
          <Table.Row class="hover:bg-transparent">
            <Table.Head class="h-9 text-[10.5px] font-semibold uppercase tracking-wider" style="width:42%">Repository : Tag</Table.Head>
            <Table.Head class="h-9 text-[10.5px] font-semibold uppercase tracking-wider" style="width:19%">Image ID</Table.Head>
            <Table.Head class="h-9 text-[10.5px] font-semibold uppercase tracking-wider" style="width:14%">Size</Table.Head>
            <Table.Head class="h-9 text-[10.5px] font-semibold uppercase tracking-wider" style="width:17%">Created</Table.Head>
            <Table.Head class="h-9 text-[10.5px] font-semibold uppercase tracking-wider" style="width:8%"></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#if shown.length === 0}
            <Table.Row class="hover:bg-transparent">
              <Table.Cell colspan={5} class="py-7 text-center text-muted-foreground">
                {#if loading}
                  Loading images…
                {:else if engineState === "running"}
                  {filter.trim() ? "No images match the filter." : "No images yet — pull one to get started."}
                {:else}
                  Engine not running.
                {/if}
              </Table.Cell>
            </Table.Row>
          {:else}
            {#each shown as img (img.id)}
              {@const acting = pending.has(img.id)}
              {@const dangling = isDangling(img)}
              <Table.Row
                class="group relative cursor-pointer data-[sel=true]:bg-muted data-[sel=true]:shadow-[inset_2px_0_0_var(--primary)]"
                data-sel={selectedId === img.id}
                style={acting ? "opacity:.55" : undefined}
                role="button"
                tabindex={0}
                aria-busy={acting}
                onclick={() => selectImage(img)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    selectImage(img);
                  }
                }}
              >
                <Table.Cell>
                  <div class="flex items-center gap-[12px] min-w-0">
                    <span class="size-[30px] rounded-[8px] shrink-0 grid place-items-center bg-muted border border-border text-muted-foreground [&_svg]:size-[15px]"><Layers aria-hidden="true" /></span>
                    <div style="min-width:0">
                      <div class="flex items-center gap-[8px] min-w-0">
                        <span class="font-semibold text-[13.5px] text-foreground tracking-[-0.1px] leading-[1.25] truncate" title={repoTag(img)}>{primaryTag(img)}</span>
                        {#if isOfficial(img)}
                          <span class="inline-flex items-center gap-[4px] text-[10.5px] font-[650] text-primary bg-primary/10 border border-primary/30 rounded-[5px] py-px px-[6px] [&_svg]:size-[11px]"><Check aria-hidden="true" />Official</span>
                        {/if}
                        {#if dangling}
                          <span class="inline-flex items-center gap-[6px] text-[11px] font-semibold py-[2px] px-[8px] rounded-[6px] tabular-nums text-chart-3 bg-chart-3/15 border border-chart-3/30"><span class="size-[6px] rounded-full shrink-0 bg-chart-3"></span>Untagged</span>
                        {/if}
                      </div>
                      {#if cleanTags(img).length > 1}
                        <div class="font-mono text-[11px] text-muted-foreground/70 leading-[1.3]">+{cleanTags(img).length - 1} more tag{cleanTags(img).length - 1 === 1 ? "" : "s"}</div>
                      {/if}
                    </div>
                  </div>
                </Table.Cell>
                <Table.Cell><span class="font-mono text-[11px] text-muted-foreground/70 leading-[1.3]" title={img.id}>{imagesApi.shortId(img.id)}</span></Table.Cell>
                <Table.Cell><span class="tabular-nums text-muted-foreground/70">{imagesApi.humanBytes(img.size)}</span></Table.Cell>
                <Table.Cell>
                  <span class="text-muted-foreground/70 tabular-nums" title={imagesApi.fullDate(img.created)}>{imagesApi.relativeTime(img.created)}</span>
                </Table.Cell>
                <Table.Cell class="text-right">
                  <div
                    class="inline-flex justify-end gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 data-[sel=true]:opacity-100"
                    data-sel={selectedId === img.id}
                  >
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      title="Tag"
                      disabled={acting}
                      onclick={(e) => { e.stopPropagation(); selectImage(img, "overview"); }}
                    ><Tag aria-hidden="true" /></Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      title="History"
                      disabled={acting}
                      onclick={(e) => { e.stopPropagation(); selectImage(img, "history"); }}
                    ><History aria-hidden="true" /></Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      title="Inspect"
                      disabled={acting}
                      onclick={(e) => { e.stopPropagation(); selectImage(img, "inspect"); }}
                    ><Info aria-hidden="true" /></Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      class="text-muted-foreground hover:text-destructive"
                      title="Remove"
                      disabled={acting}
                      onclick={(e) => { e.stopPropagation(); doRemove(img); }}
                    ><Trash2 aria-hidden="true" /></Button>
                  </div>
                </Table.Cell>
              </Table.Row>
            {/each}
          {/if}
        </Table.Body>
      </Table.Root>
    </div>

    <!-- ===== Detail drawer ===== -->
    {#if selected}
      {@const sel = selected}
      <aside class="flex flex-col min-w-0 overflow-auto bg-card border border-border rounded-xl shadow-[0_1px_2px_rgba(0,0,0,0.45),0_10px_28px_-12px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.04)] max-h-[calc(100vh-210px)] sticky top-0">
        <div class="pt-[18px] px-[20px] pb-[16px] border-b border-border">
          <div class="flex items-center gap-[12px]">
            <span class="size-[38px] rounded-[10px] shrink-0 grid place-items-center bg-muted border border-border text-muted-foreground [&_svg]:size-[19px]"><Layers aria-hidden="true" /></span>
            <div style="min-width:0">
              <div class="text-[16px] font-[680] tracking-[-0.3px] leading-[1.15]" title={repoTag(sel)}>{primaryTag(sel)}</div>
              <div class="flex items-center gap-[8px] text-[11.5px] text-muted-foreground mt-[3px] flex-wrap">
                {#if isOfficial(sel)}
                  <span class="inline-flex items-center gap-[4px] text-[10.5px] font-[650] text-primary bg-primary/10 border border-primary/30 rounded-[5px] py-px px-[6px] [&_svg]:size-[11px]"><Check aria-hidden="true" />Official</span>
                {:else if isDangling(sel)}
                  <span class="inline-flex items-center gap-[6px] text-[11px] font-semibold py-[2px] px-[8px] rounded-[6px] tabular-nums text-chart-3 bg-chart-3/15 border border-chart-3/30"><span class="size-[6px] rounded-full shrink-0 bg-chart-3"></span>Untagged</span>
                {/if}
                <span class="font-mono tabular-nums text-muted-foreground/70">{imagesApi.shortId(sel.id)}</span>
                <span>·</span>
                <span class="tabular-nums">{imagesApi.humanBytes(sel.size)}</span>
              </div>
            </div>
            <div class="ml-auto flex gap-[7px]">
              <Button variant="outline" size="icon-sm" title="Close" aria-label="Close" onclick={closeDetail}>
                <X aria-hidden="true" />
              </Button>
            </div>
          </div>
          <div class="flex gap-[7px] mt-[15px] flex-wrap">
            <Button
              variant="destructive"
              size="sm"
              disabled={pending.has(sel.id)}
              onclick={() => doRemove(sel)}
            >
              <Trash2 aria-hidden="true" />Remove
            </Button>
          </div>
        </div>

        <Tabs.Root value={detailTab} onValueChange={(v) => setTab(v as DetailTab)}>
          <Tabs.List variant="line" class="mx-5">
            <Tabs.Trigger value="overview" class="after:bg-primary">Overview</Tabs.Trigger>
            <Tabs.Trigger value="history" class="after:bg-primary">History</Tabs.Trigger>
            <Tabs.Trigger value="inspect" class="after:bg-primary">Inspect</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>

        <div class="pt-[16px] px-[20px] pb-[24px] flex flex-col gap-[16px]">
          {#if detailError}
            <Alert.Root variant="destructive">
              <Info aria-hidden="true" />
              <Alert.Description>{detailError}</Alert.Description>
            </Alert.Root>
          {/if}

          {#if detailTab === "overview"}
            <div class="flex flex-col">
              <div class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70 pt-[4px] pb-[9px]">Details</div>
              <div class="grid grid-cols-[120px_1fr] gap-[10px] py-[8px] border-t border-border items-start">
                <span class="text-[12.5px] text-muted-foreground">Image ID</span>
                <span class="text-[11.5px] text-foreground text-left break-words font-mono inline-flex items-center gap-[6px] justify-start [&_svg]:size-[13px]"
                  >{imagesApi.shortId(sel.id)}<Button
                    variant="ghost"
                    size="icon-xs"
                    class="text-muted-foreground"
                    title="Copy full ID"
                    onclick={() => copyText(sel.id)}><Copy aria-hidden="true" /></Button
                  ></span
                >
              </div>
              <div class="grid grid-cols-[120px_1fr] gap-[10px] py-[8px] border-t border-border items-start">
                <span class="text-[12.5px] text-muted-foreground">Size</span>
                <span class="text-[12.5px] text-foreground text-left break-words tabular-nums">{imagesApi.humanBytes(sel.size)}</span>
              </div>
              <div class="grid grid-cols-[120px_1fr] gap-[10px] py-[8px] border-t border-border items-start">
                <span class="text-[12.5px] text-muted-foreground">Created</span>
                <span class="text-[12.5px] text-foreground text-left break-words tabular-nums" title={imagesApi.fullDate(sel.created)}
                  >{imagesApi.relativeTime(sel.created)}</span
                >
              </div>
              <div class="grid grid-cols-[120px_1fr] gap-[10px] py-[8px] border-t border-border items-start">
                <span class="text-[12.5px] text-muted-foreground">Tags</span>
                <span class="text-[12.5px] text-foreground text-left break-words">
                  {#if cleanTags(sel).length}
                    <span class="flex gap-[5px] flex-wrap justify-start">
                      {#each cleanTags(sel) as t (t)}
                        <Badge variant="outline" class="font-mono font-normal">{t}</Badge>
                      {/each}
                    </span>
                  {:else}
                    <span class="text-muted-foreground/70">&lt;none&gt;</span>
                  {/if}
                </span>
              </div>
            </div>

            <div class="flex flex-col">
              <div class="text-[10.5px] font-[650] tracking-[0.7px] uppercase text-muted-foreground/70 pt-[4px] pb-[9px]">Tag image</div>
              <div class="flex items-center flex-wrap gap-[8px]">
                <Input
                  class="flex-1"
                  placeholder="repository (e.g. myrepo/app)"
                  bind:value={tagRepo}
                  aria-label="Repository"
                />
                <span class="text-muted-foreground/70">:</span>
                <Input class="w-[96px]" placeholder="tag" bind:value={tagTag} aria-label="Tag" />
                <Button
                  variant="outline"
                  disabled={detailLoading || !tagRepo.trim()}
                  onclick={applyTag}
                >
                  <Tag aria-hidden="true" />
                  {detailLoading ? "Tagging…" : "Apply"}
                </Button>
              </div>
            </div>
          {:else if detailTab === "history"}
            {#if detailLoading}
              <p class="text-[12px] text-muted-foreground truncate">Loading history…</p>
            {:else if historyData.length === 0}
              <p class="text-[12px] text-muted-foreground truncate">No history.</p>
            {:else}
              <div class="flex flex-col border border-border rounded-[9px] overflow-hidden bg-card">
                {#each historyData as layer, i (i)}
                  <div class="grid grid-cols-[1fr_auto] gap-[12px] items-center py-[9px] px-[14px] border-t border-border first:border-t-0">
                    <span class="font-mono text-[11.5px] text-muted-foreground truncate" title={layer.created_by}
                      >{layer.created_by}{layer.comment ? `  (${layer.comment})` : ""}</span
                    >
                    <span class="text-[11.5px] text-muted-foreground/70 tabular-nums shrink-0">{imagesApi.humanBytes(layer.size)}</span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else if detailTab === "inspect"}
            {#if detailLoading}
              <p class="text-[12px] text-muted-foreground truncate">Loading…</p>
            {:else}
              <div class="border border-border rounded-[9px] bg-background overflow-hidden">
                <div class="flex items-center gap-[8px] bg-muted border-b border-border py-[8px] px-[12px] text-[12px] text-muted-foreground">
                  <Boxes aria-hidden="true" />
                  <span>Inspect</span>
                  <span style="flex:1"></span>
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground" title="Copy JSON" onclick={() => copyText(inspectData)}>
                    <Copy aria-hidden="true" />
                  </Button>
                </div>
                <pre class="max-h-[14rem] overflow-auto py-[10px] px-[12px] font-mono text-[11.5px] leading-[1.55] text-muted-foreground select-text">{inspectData}</pre>
              </div>
            {/if}
          {/if}
        </div>
      </aside>
    {/if}
  </div>
</div>

<style>
  /* Animation-only: progress sheen (cannot be expressed with inline utilities). */
  .progress-fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 999px;
    background-color: var(--primary);
    background-image: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, #fff 28%, transparent),
      transparent
    );
    background-size: 200% 100%;
    animation: progress-sheen 1.6s linear infinite;
    transition: width 0.3s ease-out;
  }
  @keyframes progress-sheen {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }
</style>

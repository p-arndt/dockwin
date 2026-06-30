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
    if (!confirm(`Remove image "${label}"?`)) return;
    markPending(img.id, true);
    errorMsg = "";
    try {
      await imagesApi.imageRemove(img.id, false);
      if (selectedId === img.id) closeDetail();
      await load();
    } catch (e) {
      // Likely in use or has multiple tags — offer a force removal.
      if (confirm(`Remove failed: ${imagesApi.errText(e)}\n\nForce remove "${label}"?`)) {
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
    if (!confirm(note)) return;
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

  // Image table column template (shared by header + rows).
  const COLS = "minmax(220px,2.4fr) 1.1fr 0.8fr 1fr";
</script>

<div class="page">
  <!-- ===== Page header ===== -->
  <div class="head">
    <h1>Images</h1>
    <span class="chip">
      <b class="num">{images.length}</b> images
      {#if totalSize > 0}
        <span class="x">·</span>
        <b class="num">{imagesApi.humanBytes(totalSize)}</b>
      {/if}
    </span>
    <span class="sp"></span>
    <label class="search">
      <Search aria-hidden="true" />
      <input placeholder="Filter images…" bind:value={filter} />
    </label>
  </div>

  <!-- ===== Toolbar: pull (primary) + prune ===== -->
  <div class="img-toolbar">
    <label class="search img-pull">
      <Download aria-hidden="true" />
      <input
        placeholder="Pull an image, e.g. nginx:latest"
        bind:value={pullRef}
        disabled={pulling || engineState !== "running"}
        onkeydown={onPullKeydown}
      />
    </label>
    <button
      class="btn btn-pri"
      disabled={pulling || engineState !== "running" || !pullRef.trim()}
      onclick={doPull}
    >
      <Download aria-hidden="true" />
      {pulling ? "Pulling…" : "Pull"}
    </button>

    <span class="sp"></span>

    <label class="field">
      <input type="checkbox" bind:checked={pruneAll} disabled={pruning} />
      All unused
    </label>
    <button
      class="btn btn-danger"
      disabled={pruning || engineState !== "running"}
      onclick={doPrune}
    >
      <Trash2 aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune"}
    </button>
  </div>

  <!-- Pull progress / status -->
  {#if pulling || pullStatus}
    <div class="pull-feed">
      {#if pulling}
        <div class="progress"><i style="width:100%"></i></div>
      {/if}
      <p class="pull-status mono" title={pullStatus}>
        {pullStatus}{pullProgress ? `  ${pullProgress}` : ""}
      </p>
    </div>
  {/if}

  {#if pruneResult}
    <p class="pull-status">{pruneResult}</p>
  {/if}

  {#if errorMsg}
    <div class="banner err">
      <Info aria-hidden="true" />
      <span>{errorMsg}</span>
    </div>
  {/if}

  <!-- ===== List + detail drawer ===== -->
  <div class="img-split" class:has-detail={selected}>
    <div class="table">
      <div class="thead" style="--cols:{COLS}">
        <span>Repository : Tag</span>
        <span>Image ID</span>
        <span>Size</span>
        <span>Created</span>
      </div>

      {#if shown.length === 0}
        <div class="empty">
          {#if loading}
            Loading images…
          {:else if engineState === "running"}
            {filter.trim() ? "No images match the filter." : "No images yet — pull one to get started."}
          {:else}
            Engine not running.
          {/if}
        </div>
      {:else}
        {#each shown as img (img.id)}
          {@const acting = pending.has(img.id)}
          {@const dangling = isDangling(img)}
          <div
            class="trow"
            class:sel={selectedId === img.id}
            style="--cols:{COLS}; {acting ? 'opacity:.55' : ''}"
            role="button"
            tabindex="0"
            onclick={() => selectImage(img)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                selectImage(img);
              }
            }}
          >
            <div class="cell-name">
              <span class="av"><Layers aria-hidden="true" /></span>
              <div style="min-width:0">
                <div class="nm-row">
                  <span class="nm" title={repoTag(img)}>{primaryTag(img)}</span>
                  {#if isOfficial(img)}
                    <span class="official"><Check aria-hidden="true" />Official</span>
                  {/if}
                  {#if dangling}
                    <span class="pill warn"><span class="d"></span>Untagged</span>
                  {/if}
                </div>
                {#if cleanTags(img).length > 1}
                  <div class="id">+{cleanTags(img).length - 1} more tag{cleanTags(img).length - 1 === 1 ? "" : "s"}</div>
                {/if}
              </div>
            </div>

            <span class="id" title={img.id}>{imagesApi.shortId(img.id)}</span>
            <span class="num muted">{imagesApi.humanBytes(img.size)}</span>
            <span class="muted num" title={imagesApi.fullDate(img.created)}
              >{imagesApi.relativeTime(img.created)}</span
            >

            <div class="rowact">
              <button
                title="Tag"
                disabled={acting}
                onclick={(e) => { e.stopPropagation(); selectImage(img, "overview"); }}
              ><Tag aria-hidden="true" /></button>
              <button
                title="History"
                disabled={acting}
                onclick={(e) => { e.stopPropagation(); selectImage(img, "history"); }}
              ><History aria-hidden="true" /></button>
              <button
                title="Inspect"
                disabled={acting}
                onclick={(e) => { e.stopPropagation(); selectImage(img, "inspect"); }}
              ><Info aria-hidden="true" /></button>
              <button
                class="dng"
                title="Remove"
                disabled={acting}
                onclick={(e) => { e.stopPropagation(); doRemove(img); }}
              ><Trash2 aria-hidden="true" /></button>
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <!-- ===== Detail drawer ===== -->
    {#if selected}
      {@const sel = selected}
      <aside class="detail img-detail">
        <div class="dt-head">
          <div class="dt-top">
            <span class="dt-av"><Layers aria-hidden="true" /></span>
            <div style="min-width:0">
              <div class="dt-name" title={repoTag(sel)}>{primaryTag(sel)}</div>
              <div class="dt-sub">
                {#if isOfficial(sel)}
                  <span class="official"><Check aria-hidden="true" />Official</span>
                {:else if isDangling(sel)}
                  <span class="pill warn"><span class="d"></span>Untagged</span>
                {/if}
                <span class="mono">{imagesApi.shortId(sel.id)}</span>
                <span>·</span>
                <span class="num">{imagesApi.humanBytes(sel.size)}</span>
              </div>
            </div>
            <div class="dt-head-acts">
              <button class="dt-x" title="Close" onclick={closeDetail}>
                <X aria-hidden="true" />
              </button>
            </div>
          </div>
          <div class="dt-acts">
            <button
              class="btn dng"
              disabled={pending.has(sel.id)}
              onclick={() => doRemove(sel)}
            >
              <Trash2 aria-hidden="true" />Remove
            </button>
          </div>
        </div>

        <div class="tabs">
          <button class:on={detailTab === "overview"} onclick={() => setTab("overview")}>Overview</button>
          <button class:on={detailTab === "history"} onclick={() => setTab("history")}>History</button>
          <button class:on={detailTab === "inspect"} onclick={() => setTab("inspect")}>Inspect</button>
        </div>

        <div class="dt-body">
          {#if detailError}
            <div class="banner err">
              <Info aria-hidden="true" />
              <span>{detailError}</span>
            </div>
          {/if}

          {#if detailTab === "overview"}
            <div class="kv">
              <div class="sec">Details</div>
              <div class="r">
                <span class="k">Image ID</span>
                <span class="v copy mono"
                  >{imagesApi.shortId(sel.id)}<button
                    class="copy-btn"
                    title="Copy full ID"
                    onclick={() => copyText(sel.id)}><Copy aria-hidden="true" /></button
                  ></span
                >
              </div>
              <div class="r">
                <span class="k">Size</span>
                <span class="v num">{imagesApi.humanBytes(sel.size)}</span>
              </div>
              <div class="r">
                <span class="k">Created</span>
                <span class="v num" title={imagesApi.fullDate(sel.created)}
                  >{imagesApi.relativeTime(sel.created)}</span
                >
              </div>
              <div class="r">
                <span class="k">Tags</span>
                <span class="v">
                  {#if cleanTags(sel).length}
                    <span class="chips">
                      {#each cleanTags(sel) as t (t)}
                        <span class="tag mono">{t}</span>
                      {/each}
                    </span>
                  {:else}
                    <span class="muted">&lt;none&gt;</span>
                  {/if}
                </span>
              </div>
            </div>

            <div class="kv">
              <div class="sec">Tag image</div>
              <div class="tag-form">
                <input
                  class="inp"
                  placeholder="repository (e.g. myrepo/app)"
                  bind:value={tagRepo}
                />
                <span class="tag-sep">:</span>
                <input class="inp inp-tag" placeholder="tag" bind:value={tagTag} />
                <button
                  class="btn btn-soft"
                  disabled={detailLoading || !tagRepo.trim()}
                  onclick={applyTag}
                >
                  <Tag aria-hidden="true" />
                  {detailLoading ? "Tagging…" : "Apply"}
                </button>
              </div>
            </div>
          {:else if detailTab === "history"}
            {#if detailLoading}
              <p class="pull-status">Loading history…</p>
            {:else if historyData.length === 0}
              <p class="pull-status">No history.</p>
            {:else}
              <div class="layers">
                {#each historyData as layer, i (i)}
                  <div class="layer">
                    <span class="cmd" title={layer.created_by}
                      >{layer.created_by}{layer.comment ? `  (${layer.comment})` : ""}</span
                    >
                    <span class="size num">{imagesApi.humanBytes(layer.size)}</span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else if detailTab === "inspect"}
            {#if detailLoading}
              <p class="pull-status">Loading…</p>
            {:else}
              <div class="outpane">
                <div class="bar">
                  <Boxes aria-hidden="true" />
                  <span>Inspect</span>
                  <span style="flex:1"></span>
                  <button class="copy-btn" title="Copy JSON" onclick={() => copyText(inspectData)}>
                    <Copy aria-hidden="true" />
                  </button>
                </div>
                <pre class="body-out">{inspectData}</pre>
              </div>
            {/if}
          {/if}
        </div>
      </aside>
    {/if}
  </div>
</div>

<style>
  /* Layout-only helpers (tokens only — no raw colours). */
  .img-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .img-pull {
    flex: 1;
    min-width: 240px;
    max-width: 520px;
  }

  .pull-feed {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .pull-status {
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .img-split {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
    align-items: start;
    min-width: 0;
  }
  .img-split.has-detail {
    grid-template-columns: minmax(0, 1fr) clamp(340px, 34%, 392px);
  }
  .img-detail {
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow), inset 0 1px 0 var(--hi);
    max-height: calc(100vh - 210px);
    position: sticky;
    top: 0;
  }

  /* loud name + inline badges */
  .nm-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  /* tag editor inputs */
  .tag-form {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .inp {
    flex: 1;
    min-width: 150px;
    background: var(--s2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 6px 10px;
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    outline: none;
    box-shadow: inset 0 1px 0 var(--hi);
  }
  .inp-tag {
    flex: 0 0 96px;
    min-width: 72px;
  }
  .inp:focus {
    border-color: var(--text-4);
  }
  .inp::placeholder {
    color: var(--text-3);
  }
  .tag-sep {
    color: var(--text-4);
  }

  /* small inline copy button (reuses kv .copy svg sizing) */
  .copy-btn {
    display: inline-grid;
    place-items: center;
    background: transparent;
    border: 0;
    padding: 0;
    cursor: pointer;
    color: var(--text-4);
  }
  .copy-btn:hover {
    color: var(--text-2);
  }
  .copy-btn :global(svg) {
    width: 13px;
    height: 13px;
  }
</style>

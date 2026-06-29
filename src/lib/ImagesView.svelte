<script lang="ts">
  // Rich images view: pull-with-progress, list, and per-image actions (remove,
  // tag, history, inspect) plus a prune control. Owns its own fetch lifecycle
  // (loads on mount, when the engine becomes running, and on a parent refresh
  // token). Talks to the backend only through imagesApi.ts.
  import { onMount } from "svelte";
  import Layers from "@lucide/svelte/icons/layers";
  import Download from "@lucide/svelte/icons/download";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Tag from "@lucide/svelte/icons/tag";
  import History from "@lucide/svelte/icons/history";
  import Search from "@lucide/svelte/icons/search";
  import Info from "@lucide/svelte/icons/info";
  import X from "@lucide/svelte/icons/x";
  import * as imagesApi from "./imagesApi";
  import type { ImageLayer } from "./imagesApi";
  import type { EngineState, ImageDto } from "./types";

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

  // --- expandable detail (one row at a time): tag editor, history, inspect ---
  type DetailMode = "tag" | "history" | "inspect";
  let detailId = $state<string | null>(null);
  let detailMode = $state<DetailMode | null>(null);
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
      await load();
    } catch (e) {
      // Likely in use or has multiple tags — offer a force removal.
      if (confirm(`Remove failed: ${imagesApi.errText(e)}\n\nForce remove "${label}"?`)) {
        try {
          await imagesApi.imageRemove(img.id, true);
          await load();
        } catch (e2) {
          errorMsg = `Force remove failed: ${imagesApi.errText(e2)}`;
        }
      }
    } finally {
      markPending(img.id, false);
    }
  }

  // --- expandable detail toggles ---
  function closeDetail() {
    detailId = null;
    detailMode = null;
    detailError = "";
    historyData = [];
    inspectData = "";
  }

  function openTag(img: ImageDto) {
    if (detailId === img.id && detailMode === "tag") return closeDetail();
    detailId = img.id;
    detailMode = "tag";
    detailError = "";
    // Seed from the first tag if present.
    const first = (img.tags ?? []).find((t) => t && t !== "<none>:<none>");
    if (first && first.includes(":")) {
      const idx = first.lastIndexOf(":");
      tagRepo = first.slice(0, idx);
      tagTag = first.slice(idx + 1) || "latest";
    } else {
      tagRepo = first ?? "";
      tagTag = "latest";
    }
  }

  async function applyTag() {
    if (!detailId) return;
    const repo = tagRepo.trim();
    const tag = tagTag.trim() || "latest";
    if (!repo) {
      detailError = "Repository is required.";
      return;
    }
    detailLoading = true;
    detailError = "";
    try {
      await imagesApi.imageTag(detailId, repo, tag);
      closeDetail();
      await load();
    } catch (e) {
      detailError = imagesApi.errText(e);
    } finally {
      detailLoading = false;
    }
  }

  async function openHistory(img: ImageDto) {
    if (detailId === img.id && detailMode === "history") return closeDetail();
    detailId = img.id;
    detailMode = "history";
    detailError = "";
    historyData = [];
    detailLoading = true;
    try {
      historyData = await imagesApi.imageHistory(img.id);
    } catch (e) {
      detailError = imagesApi.errText(e);
    } finally {
      detailLoading = false;
    }
  }

  async function openInspect(img: ImageDto) {
    if (detailId === img.id && detailMode === "inspect") return closeDetail();
    detailId = img.id;
    detailMode = "inspect";
    detailError = "";
    inspectData = "";
    detailLoading = true;
    try {
      inspectData = await imagesApi.imageInspect(img.id);
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
  function repoTag(img: ImageDto): string {
    const tags = (img.tags ?? []).filter((t) => t && t !== "<none>:<none>");
    return tags.length ? tags.join(", ") : "<none>";
  }

  const COL = 5; // table column count, for full-width detail rows
</script>

<section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
  <!-- Header -->
  <div class="flex items-baseline gap-2.5 border-b border-[#262b34] px-3.5 py-3">
    <h2 class="text-sm font-semibold">Images</h2>
    <span class="text-xs text-[#9aa3af]">
      {images.length ? `${images.length} total` : ""}
    </span>
    <div class="ml-auto flex items-center gap-1.5">
      <div
        class="flex items-center gap-1.5 rounded-md border border-[#262b34] bg-[#0d1117] px-2 py-[3px]"
      >
        <Search size={13} class="text-[#6e7681]" aria-hidden="true" />
        <input
          class="w-36 bg-transparent text-[12px] text-[#e6e8eb] placeholder-[#6e7681] outline-none"
          placeholder="Filter…"
          bind:value={filter}
        />
      </div>
    </div>
  </div>

  <!-- Pull bar -->
  <div class="border-b border-[#262b34] px-3.5 py-3">
    <div class="flex items-center gap-2">
      <div
        class="flex flex-1 items-center gap-1.5 rounded-md border border-[#262b34] bg-[#0d1117] px-2.5 py-[5px]"
      >
        <Download size={14} class="text-[#6e7681]" aria-hidden="true" />
        <input
          class="w-full bg-transparent text-[13px] text-[#e6e8eb] placeholder-[#6e7681] outline-none"
          placeholder="Pull an image, e.g. nginx:latest"
          bind:value={pullRef}
          disabled={pulling || engineState !== "running"}
          onkeydown={onPullKeydown}
        />
      </div>
      <button
        class="flex items-center gap-1.5 rounded-md border border-[#2f81f7] bg-[#1f6feb] px-3.5 py-[6px] text-[13px] font-medium text-white transition-colors hover:not-disabled:bg-[#2f81f7] disabled:cursor-default disabled:opacity-50"
        disabled={pulling || engineState !== "running" || !pullRef.trim()}
        onclick={doPull}
      >
        <Download size={14} aria-hidden="true" />
        {pulling ? "Pulling…" : "Pull"}
      </button>
    </div>

    {#if pulling || pullStatus}
      <div class="mt-2.5">
        {#if pulling}
          <!-- Indeterminate progress bar (driven by the streamed status text). -->
          <div class="h-2 w-full overflow-hidden rounded-full bg-[#21262d]">
            <div
              class="provision-bar h-full w-full animate-pulse rounded-full bg-[#1f6feb]"
            ></div>
          </div>
        {/if}
        <p class="mt-1.5 truncate text-[12px] text-[#c7ccd4]" title={pullStatus}>
          {pullStatus}{pullProgress ? `  ${pullProgress}` : ""}
        </p>
      </div>
    {/if}
  </div>

  <!-- Prune row -->
  <div
    class="flex flex-wrap items-center gap-3 border-b border-[#262b34] px-3.5 py-2.5"
  >
    <button
      class="flex items-center gap-1.5 rounded-md border border-[#f8514966] bg-[#f851491a] px-2.5 py-[5px] text-[12px] text-[#ff9b95] transition-colors hover:not-disabled:bg-[#f8514926] disabled:cursor-default disabled:opacity-45"
      disabled={pruning || engineState !== "running"}
      onclick={doPrune}
    >
      <Trash2 size={14} aria-hidden="true" />
      {pruning ? "Pruning…" : "Prune"}
    </button>
    <label class="flex items-center gap-1.5 text-[12px] text-[#c7ccd4]">
      <input type="checkbox" bind:checked={pruneAll} disabled={pruning} />
      All unused, not just dangling
    </label>
    {#if pruneResult}
      <span class="text-[12px] text-[#9aa3af]">{pruneResult}</span>
    {/if}
  </div>

  <!-- Inline error panel -->
  {#if errorMsg}
    <div
      class="mx-3.5 mt-3 select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
    >
      {errorMsg}
    </div>
  {/if}

  <!-- Table / empty state -->
  {#if shown.length === 0}
    <div
      class="flex flex-col items-center gap-2 px-3.5 py-9 text-center text-[#9aa3af]"
    >
      <Layers size={22} class="opacity-60" aria-hidden="true" />
      <span>
        {#if loading}
          Loading images…
        {:else if engineState === "running"}
          {filter.trim() ? "No images match the filter." : "No images."}
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
              >Repository:Tag</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Image ID</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Size</th
            >
            <th
              class="sticky top-0 whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Created</th
            >
            <th
              class="sticky top-0 w-[1%] whitespace-nowrap border-b border-[#262b34] bg-[#171a21] px-3 py-2.5 text-left font-medium text-[#9aa3af]"
              >Actions</th
            >
          </tr>
        </thead>
        <tbody>
          {#each shown as img (img.id)}
            {@const acting = pending.has(img.id)}
            {@const open = detailId === img.id}
            <tr class="hover:bg-[#1b1f27] {acting ? 'opacity-60' : ''}">
              <td
                class="max-w-[320px] overflow-hidden text-ellipsis whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle font-medium"
                title={repoTag(img)}>{repoTag(img)}</td
              >
              <td
                class="font-mono-app border-b border-[#262b34] px-3 py-2.5 align-middle text-xs text-[#9aa3af]"
                title={img.id}>{imagesApi.shortId(img.id)}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{imagesApi.humanBytes(img.size)}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                title={imagesApi.fullDate(img.created)}
                >{imagesApi.relativeTime(img.created)}</td
              >
              <td class="border-b border-[#262b34] px-3 py-2.5 align-middle">
                <div class="flex justify-end gap-1.5">
                  <button
                    class="inline-flex cursor-pointer items-center gap-1 rounded-md border px-2 py-[3px] text-xs transition-colors disabled:cursor-default disabled:opacity-45 {open &&
                    detailMode === 'tag'
                      ? 'border-[#2f81f7] bg-[#1f6feb1a] text-[#2f81f7]'
                      : 'border-[#262b34] text-[#e6e8eb] hover:not-disabled:bg-[#21262d]'}"
                    disabled={acting}
                    onclick={() => openTag(img)}
                    ><Tag size={13} aria-hidden="true" />Tag</button
                  >
                  <button
                    class="inline-flex cursor-pointer items-center gap-1 rounded-md border px-2 py-[3px] text-xs transition-colors disabled:cursor-default disabled:opacity-45 {open &&
                    detailMode === 'history'
                      ? 'border-[#2f81f7] bg-[#1f6feb1a] text-[#2f81f7]'
                      : 'border-[#262b34] text-[#e6e8eb] hover:not-disabled:bg-[#21262d]'}"
                    disabled={acting}
                    onclick={() => openHistory(img)}
                    ><History size={13} aria-hidden="true" />History</button
                  >
                  <button
                    class="inline-flex cursor-pointer items-center gap-1 rounded-md border px-2 py-[3px] text-xs transition-colors disabled:cursor-default disabled:opacity-45 {open &&
                    detailMode === 'inspect'
                      ? 'border-[#2f81f7] bg-[#1f6feb1a] text-[#2f81f7]'
                      : 'border-[#262b34] text-[#e6e8eb] hover:not-disabled:bg-[#21262d]'}"
                    disabled={acting}
                    onclick={() => openInspect(img)}
                    ><Info size={13} aria-hidden="true" />Inspect</button
                  >
                  <button
                    class="inline-flex cursor-pointer items-center gap-1 rounded-md border border-[#f8514980] px-2 py-[3px] text-xs text-[#f85149] transition-colors hover:not-disabled:bg-[#f851491f] disabled:cursor-default disabled:opacity-45"
                    disabled={acting}
                    onclick={() => doRemove(img)}
                    ><Trash2 size={13} aria-hidden="true" />Remove</button
                  >
                </div>
              </td>
            </tr>

            <!-- Expandable detail row -->
            {#if open && detailMode}
              <tr>
                <td colspan={COL} class="border-b border-[#262b34] bg-[#0d1117] p-0">
                  <div class="px-3.5 py-3">
                    <div class="mb-2 flex items-center gap-2">
                      <span class="text-[12px] font-semibold text-[#c7ccd4]">
                        {#if detailMode === "tag"}Tag image
                        {:else if detailMode === "history"}History
                        {:else}Inspect{/if}
                      </span>
                      <span
                        class="font-mono-app text-[11px] text-[#6e7681]"
                        title={img.id}>{imagesApi.shortId(img.id)}</span
                      >
                      <button
                        class="ml-auto cursor-pointer rounded p-0.5 text-[#9aa3af] hover:bg-[#21262d]"
                        title="Close"
                        onclick={closeDetail}
                      >
                        <X size={14} aria-hidden="true" />
                      </button>
                    </div>

                    {#if detailError}
                      <div
                        class="mb-2 select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-2.5 py-1.5 text-[12px] text-[#ff9b95]"
                      >
                        {detailError}
                      </div>
                    {/if}

                    {#if detailMode === "tag"}
                      <div class="flex flex-wrap items-center gap-2">
                        <input
                          class="min-w-[200px] flex-1 rounded-md border border-[#262b34] bg-[#171a21] px-2.5 py-[5px] text-[13px] text-[#e6e8eb] placeholder-[#6e7681] outline-none focus:border-[#2f81f7]"
                          placeholder="repository (e.g. myrepo/app)"
                          bind:value={tagRepo}
                        />
                        <span class="text-[#6e7681]">:</span>
                        <input
                          class="w-32 rounded-md border border-[#262b34] bg-[#171a21] px-2.5 py-[5px] text-[13px] text-[#e6e8eb] placeholder-[#6e7681] outline-none focus:border-[#2f81f7]"
                          placeholder="tag"
                          bind:value={tagTag}
                        />
                        <button
                          class="rounded-md border border-[#2f81f7] bg-[#1f6feb] px-3 py-[5px] text-[12px] font-medium text-white transition-colors hover:not-disabled:bg-[#2f81f7] disabled:cursor-default disabled:opacity-50"
                          disabled={detailLoading || !tagRepo.trim()}
                          onclick={applyTag}
                        >
                          {detailLoading ? "Tagging…" : "Apply tag"}
                        </button>
                        <button
                          class="rounded-md border border-[#262b34] bg-[#21262d] px-3 py-[5px] text-[12px] text-[#e6e8eb] transition-colors hover:bg-[#2b3138]"
                          onclick={closeDetail}>Cancel</button
                        >
                      </div>
                    {:else if detailMode === "history"}
                      {#if detailLoading}
                        <p class="text-[12px] text-[#9aa3af]">Loading history…</p>
                      {:else if historyData.length === 0}
                        <p class="text-[12px] text-[#9aa3af]">No history.</p>
                      {:else}
                        <div
                          class="max-h-72 select-text overflow-auto rounded-md border border-[#262b34]"
                        >
                          <table class="w-full border-collapse text-[12px]">
                            <tbody>
                              {#each historyData as layer, i (i)}
                                <tr class="align-top">
                                  <td
                                    class="whitespace-nowrap border-b border-[#262b34] px-2.5 py-1.5 text-[#9aa3af]"
                                    >{imagesApi.relativeTime(layer.created)}</td
                                  >
                                  <td
                                    class="whitespace-nowrap border-b border-[#262b34] px-2.5 py-1.5 text-right text-[#9aa3af]"
                                    >{imagesApi.humanBytes(layer.size)}</td
                                  >
                                  <td
                                    class="font-mono-app w-full border-b border-[#262b34] px-2.5 py-1.5 break-all text-[#c7ccd4]"
                                    >{layer.created_by}{layer.comment
                                      ? `  (${layer.comment})`
                                      : ""}</td
                                  >
                                </tr>
                              {/each}
                            </tbody>
                          </table>
                        </div>
                      {/if}
                    {:else if detailMode === "inspect"}
                      {#if detailLoading}
                        <p class="text-[12px] text-[#9aa3af]">Loading…</p>
                      {:else}
                        <pre
                          class="font-mono-app max-h-96 select-text overflow-auto rounded-md border border-[#262b34] bg-[#171a21] p-2.5 text-[11.5px] leading-relaxed text-[#c7ccd4] whitespace-pre">{inspectData}</pre>
                      {/if}
                    {/if}
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

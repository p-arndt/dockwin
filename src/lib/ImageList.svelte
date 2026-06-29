<script lang="ts">
  // Read-only images view. Owns its own fetch lifecycle: loads on mount, when the
  // view becomes active, and on an explicit refresh request from the parent.
  // Talks to dockwin-core only through api.ts.
  import Layers from "@lucide/svelte/icons/layers";
  import * as api from "./api";
  import type { EngineState, ImageDto } from "./types";

  interface Props {
    engineState?: EngineState;
    // Monotonically increasing token; bump it from the parent to force a reload.
    refreshKey?: number;
  }

  let { engineState = "unknown", refreshKey = 0 }: Props = $props();

  let images = $state<ImageDto[]>([]);
  let errorMsg = $state("");
  let loading = $state(false);

  let busy = false; // non-reactive guard against overlapping loads

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
      const raw = await api.imageList(true);
      const list = Array.isArray(raw) ? raw : [];
      // Newest first.
      list.sort((a, b) => (b.created ?? 0) - (a.created ?? 0));
      images = list;
      errorMsg = "";
    } catch (e) {
      errorMsg = `Failed to load images: ${api.errText(e)}`;
    } finally {
      loading = false;
      busy = false;
    }
  }

  // Reload on mount and whenever the engine state or the parent's refresh token
  // changes. (Reading both inside the effect registers them as dependencies.)
  $effect(() => {
    void engineState;
    void refreshKey;
    load();
  });

  // --- pure formatting helpers ---
  function repoTag(img: ImageDto): string {
    const tags = (img.tags ?? []).filter((t) => t && t !== "<none>:<none>");
    return tags.length ? tags.join(", ") : "<none>";
  }

  function shortId(id: string): string {
    return (id ?? "").replace(/^sha256:/, "").slice(0, 12);
  }

  function humanSize(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let n = bytes;
    let i = 0;
    while (n >= 1024 && i < units.length - 1) {
      n /= 1024;
      i++;
    }
    const val = i === 0 ? n : n.toFixed(n < 10 ? 1 : 0);
    return `${val} ${units[i]}`;
  }

  function relativeDate(unixSecs: number): string {
    if (!Number.isFinite(unixSecs) || unixSecs <= 0) return "—";
    const ms = unixSecs * 1000;
    const diff = Date.now() - ms;
    const sec = Math.floor(diff / 1000);
    if (sec < 60) return "just now";
    const min = Math.floor(sec / 60);
    if (min < 60) return `${min} min ago`;
    const hr = Math.floor(min / 60);
    if (hr < 24) return `${hr} hour${hr === 1 ? "" : "s"} ago`;
    const day = Math.floor(hr / 24);
    if (day < 30) return `${day} day${day === 1 ? "" : "s"} ago`;
    return new Date(ms).toLocaleDateString();
  }

  function fullDate(unixSecs: number): string {
    if (!Number.isFinite(unixSecs) || unixSecs <= 0) return "";
    return new Date(unixSecs * 1000).toLocaleString();
  }
</script>

<section class="overflow-hidden rounded-md border border-[#262b34] bg-[#171a21]">
  <div class="flex items-baseline gap-2.5 border-b border-[#262b34] px-3.5 py-3">
    <h2 class="text-sm font-semibold">Images</h2>
    <span class="text-xs text-[#9aa3af]">
      {images.length ? `${images.length} total` : ""}
    </span>
  </div>

  {#if errorMsg}
    <div
      class="mx-3.5 mt-3 select-text rounded-md border border-[#f8514966] bg-[#f851491a] px-3 py-2 text-[13px] text-[#ff9b95]"
    >
      {errorMsg}
    </div>
  {/if}

  {#if images.length === 0}
    <div class="flex flex-col items-center gap-2 px-3.5 py-9 text-center text-[#9aa3af]">
      <Layers size={22} class="opacity-60" aria-hidden="true" />
      <span>
        {#if loading}
          Loading images…
        {:else if engineState === "running"}
          No images.
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
          </tr>
        </thead>
        <tbody>
          {#each images as img (img.id)}
            <tr class="hover:bg-[#1b1f27]">
              <td
                class="max-w-[320px] overflow-hidden text-ellipsis whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle font-medium"
                title={repoTag(img)}>{repoTag(img)}</td
              >
              <td
                class="font-mono-app border-b border-[#262b34] px-3 py-2.5 align-middle text-xs text-[#9aa3af]"
                title={img.id}>{shortId(img.id)}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                >{humanSize(img.size)}</td
              >
              <td
                class="whitespace-nowrap border-b border-[#262b34] px-3 py-2.5 align-middle text-[#9aa3af]"
                title={fullDate(img.created)}>{relativeDate(img.created)}</td
              >
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<script lang="ts">
  // Custom window title bar (native decorations are disabled in tauri.conf.json).
  // Owns window dragging + the minimize / maximize-restore / close controls, and
  // carries the dockwin brand that used to live in the sidebar header.
  //
  // The bar itself is the drag region (data-tauri-drag-region) — Tauri handles
  // press-drag-to-move and double-click-to-maximize for us. Buttons opt out of
  // the drag region so clicks register normally. All window calls are guarded so
  // plain-browser dev (no Tauri runtime) degrades to no-ops.
  import { onMount } from "svelte";

  // The current window handle, or null when the Tauri runtime isn't present.
  let appWindow = $state<import("@tauri-apps/api/window").Window | null>(null);
  let maximized = $state(false);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        appWindow = win;
        maximized = await win.isMaximized();
        // Keep the maximize/restore glyph in sync with the real window state.
        unlisten = await win.onResized(async () => {
          maximized = await win.isMaximized();
        });
      } catch {
        // Not running under Tauri (browser dev) — leave controls inert.
      }
    })();
    return () => unlisten?.();
  });
</script>

<div
  data-tauri-drag-region
  class="flex h-[var(--titlebar-h,34px)] shrink-0 items-center gap-[9px] border-b border-border bg-card pl-[12px] select-none"
>
  <!-- Brand (pointer-events pass through to the drag region) -->
  <span class="pointer-events-none flex items-center gap-[7px]">
    <img src="/dockwin-mark.png" alt="" width="20" height="20" class="size-[20px] shrink-0" />
    <span class="text-[12.5px] font-semibold tracking-tight leading-none">dockwin</span>
  </span>

  <span class="pointer-events-none flex-1"></span>

  <!-- Window controls (Windows order: minimize, maximize/restore, close) -->
  <div class="flex items-center self-stretch">
    <button
      type="button"
      title="Minimize"
      aria-label="Minimize"
      onclick={() => appWindow?.minimize()}
      class="grid h-full w-[44px] place-items-center text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none"
    >
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M0 5 H10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>

    <button
      type="button"
      title={maximized ? "Restore" : "Maximize"}
      aria-label={maximized ? "Restore" : "Maximize"}
      onclick={() => appWindow?.toggleMaximize()}
      class="grid h-full w-[44px] place-items-center text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none"
    >
      {#if maximized}
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none">
          <rect x="0.5" y="2.5" width="7" height="7" stroke="currentColor" stroke-width="1" />
          <path d="M2.5 2.5 V0.5 H9.5 V7.5 H7.5" stroke="currentColor" stroke-width="1" />
        </svg>
      {:else}
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true" fill="none">
          <rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" stroke-width="1" />
        </svg>
      {/if}
    </button>

    <button
      type="button"
      title="Close"
      aria-label="Close"
      onclick={() => appWindow?.close()}
      class="grid h-full w-[44px] place-items-center text-muted-foreground transition-colors hover:bg-destructive hover:text-white focus-visible:outline-none"
    >
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
        <path d="M0 0 L10 10 M10 0 L0 10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
  </div>
</div>

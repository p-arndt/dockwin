<script lang="ts">
  // Quiet status dot — a small coloured circle, nothing more. Pair it with a
  // single word + muted sub-line in markup; do NOT build glowing pill-bands.
  type Tone = "ok" | "warn" | "err" | "off" | "run";

  interface Props {
    tone?: Tone;
    /** Diameter in px. */
    size?: number;
    /** Soft halo (used on the "live engine" indicator). */
    halo?: boolean;
  }

  let { tone = "off", size = 7, halo = false }: Props = $props();

  // "run" is an alias of "ok". Map tone → colour utility.
  const colorClass = $derived(
    tone === "ok" || tone === "run"
      ? "bg-chart-2"
      : tone === "warn"
        ? "bg-chart-3"
        : tone === "err"
          ? "bg-destructive"
          : "bg-chart-5"
  );
</script>

<span class="relative inline-block shrink-0" style="width:{size}px; height:{size}px;" aria-hidden="true">
  {#if halo}
    <span class="absolute inset-0 rounded-full {colorClass} eng-dot-ring"></span>
  {/if}
  <span class="absolute inset-0 rounded-full {colorClass}"></span>
</span>

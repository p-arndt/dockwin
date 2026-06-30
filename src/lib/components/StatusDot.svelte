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

  // "run" is an alias of "ok".
  const colorVar = $derived(
    tone === "ok" || tone === "run"
      ? "var(--ok)"
      : tone === "warn"
        ? "var(--warn)"
        : tone === "err"
          ? "var(--err)"
          : "var(--off)"
  );
  const dimVar = $derived(
    tone === "ok" || tone === "run"
      ? "var(--ok-dim)"
      : tone === "warn"
        ? "var(--warn-dim)"
        : tone === "err"
          ? "var(--err-dim)"
          : "transparent"
  );
</script>

<span
  class="sd"
  style="--sd-size:{size}px; --sd-color:{colorVar}; --sd-dim:{dimVar};"
  class:halo
  aria-hidden="true"
></span>

<style>
  .sd {
    display: inline-block;
    width: var(--sd-size);
    height: var(--sd-size);
    border-radius: 50%;
    background: var(--sd-color);
    flex: none;
  }
  .sd.halo {
    box-shadow: 0 0 0 3px var(--sd-dim);
  }
</style>

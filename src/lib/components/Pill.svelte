<script lang="ts">
  // Quiet status/count pill (vulnerabilities, health, ok/warn/err counts).
  // A small dot + content. Restrained colour: tinted text + faint tint bg,
  // never a loud filled band.
  import type { Snippet } from "svelte";

  type Tone = "ok" | "warn" | "err" | "neutral";

  interface Props {
    tone?: Tone;
    /** Show the leading dot. */
    dot?: boolean;
    children?: Snippet;
  }

  let { tone = "neutral", dot = true, children }: Props = $props();
</script>

<span class="pill {tone}">
  {#if dot}<span class="d"></span>{/if}
  {@render children?.()}
</span>

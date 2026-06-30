<script lang="ts">
  // One row of a key/value details list (matches the .kv .r layout). Wrap a set
  // of these in a <div class="kv"> (optionally with a <div class="sec">Title</div>).
  import type { Snippet } from "svelte";

  interface Props {
    /** Left-hand label. */
    label: string;
    /** Render the value with the mono font (ids / hashes / commands). */
    mono?: boolean;
    /** Plain string value (use the `children` snippet for rich content). */
    value?: string;
    children?: Snippet;
  }

  let { label, mono = false, value, children }: Props = $props();
</script>

<div class="r">
  <span class="k">{label}</span>
  <span class="v" class:mono>
    {#if children}{@render children()}{:else}{value}{/if}
  </span>
</div>

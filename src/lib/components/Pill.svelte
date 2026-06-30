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

  // Pill container colours (text / bg tint / border tint) per tone.
  const pillTone: Record<Tone, string> = {
    ok: "text-chart-2 bg-chart-2/15 border-chart-2/30",
    warn: "text-chart-3 bg-chart-3/15 border-chart-3/30",
    err: "text-destructive bg-destructive/15 border-destructive/30",
    neutral: "text-muted-foreground bg-muted border-border",
  };

  // Leading dot colour per tone.
  const dotTone: Record<Tone, string> = {
    ok: "bg-chart-2",
    warn: "bg-chart-3",
    err: "bg-destructive",
    neutral: "bg-muted-foreground",
  };
</script>

<span
  class="inline-flex items-center gap-[6px] px-[8px] py-[2px] rounded-[6px] border text-[11px] font-semibold tabular-nums {pillTone[tone]}"
>
  {#if dot}<span class="w-[6px] h-[6px] rounded-full shrink-0 {dotTone[tone]}"></span>{/if}
  {@render children?.()}
</span>

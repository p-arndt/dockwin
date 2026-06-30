<script lang="ts">
  // Minimal area+line sparkline. Neutral by default; pass focused=true ONLY for
  // the single "focused chart" accent role (per the restraint rules). A unique
  // gradient id is generated per instance so multiple sparks don't collide.

  interface Props {
    /** Series values; rendered left→right, auto-scaled to min/max. */
    points?: number[];
    /** Use the lime accent. Reserve for ONE chart per screen. */
    focused?: boolean;
    width?: number;
    height?: number;
    /** Fill the area under the line. */
    area?: boolean;
  }

  let {
    points = [],
    focused = false,
    width = 120,
    height = 34,
    area = true,
  }: Props = $props();

  // Stable-enough unique id for the gradient.
  const uid = "spark-" + Math.random().toString(36).slice(2, 9);
  const stroke = $derived(focused ? "var(--lime)" : "var(--text-3)");

  const geom = $derived.by(() => {
    const n = points.length;
    if (n === 0) return { line: "", fill: "" };
    const min = Math.min(...points);
    const max = Math.max(...points);
    const span = max - min || 1;
    const stepX = n > 1 ? width / (n - 1) : 0;
    const coords = points.map((p, i) => {
      const x = i * stepX;
      const y = height - ((p - min) / span) * (height - 2) - 1;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    const line = "M" + coords.join(" L");
    const fill = `${line} L${width},${height} L0,${height} Z`;
    return { line, fill };
  });
</script>

<svg
  class="spark-c"
  viewBox="0 0 {width} {height}"
  preserveAspectRatio="none"
  width={width}
  height={height}
  aria-hidden="true"
>
  {#if area}
    <defs>
      <linearGradient id={uid} x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color={stroke} stop-opacity="0.35" />
        <stop offset="1" stop-color={stroke} stop-opacity="0" />
      </linearGradient>
    </defs>
    <path d={geom.fill} fill="url(#{uid})" />
  {/if}
  <path
    d={geom.line}
    fill="none"
    stroke={stroke}
    stroke-width="1.5"
    vector-effect="non-scaling-stroke"
  />
</svg>

<style>
  .spark-c {
    display: block;
  }
</style>

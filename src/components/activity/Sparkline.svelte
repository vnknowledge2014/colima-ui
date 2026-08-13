<script lang="ts">
  /**
   * A tiny history chart.
   *
   * `null` in the series means "no sample" — the stream lagged and those points
   * were never seen. The line is broken there rather than joined across, because
   * a straight segment over a gap asserts data that does not exist, and reads as
   * a period of steady load when it might have been a spike.
   */
  interface Props {
    /** Oldest first. `null` marks a gap. */
    values: Array<number | null>;
    /** Fixed upper bound, e.g. 100 for a percentage. Omit to scale to the data. */
    max?: number;
    width?: number;
    height?: number;
    color?: string;
  }

  let { values, max, width = 90, height = 22, color = "var(--color-primary, #3b82f6)" }: Props =
    $props();

  const PAD = 1;

  const scaleMax = $derived.by(() => {
    if (max !== undefined) return max > 0 ? max : 1;
    const present = values.filter((v): v is number => v !== null);
    const peak = present.length ? Math.max(...present) : 0;
    // Never zero: a flat-zero series would divide by it.
    return peak > 0 ? peak : 1;
  });

  /** Contiguous runs of real samples, each becoming its own polyline. */
  const segments = $derived.by(() => {
    const n = values.length;
    if (n === 0) return [] as string[];
    const step = n > 1 ? (width - PAD * 2) / (n - 1) : 0;
    const usable = height - PAD * 2;

    const runs: string[] = [];
    let current: string[] = [];
    values.forEach((value, i) => {
      if (value === null) {
        if (current.length) runs.push(current.join(" "));
        current = [];
        return;
      }
      const x = PAD + i * step;
      const y = PAD + usable - Math.min(value / scaleMax, 1) * usable;
      current.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    });
    if (current.length) runs.push(current.join(" "));
    return runs;
  });

  /** A lone sample has no line to draw, so it is shown as a dot. */
  const dots = $derived(
    segments.filter((s) => !s.includes(" ")).map((s) => s.split(",").map(Number))
  );
</script>

<svg class="sparkline" {width} {height} viewBox="0 0 {width} {height}" aria-hidden="true">
  {#each segments as points (points)}
    {#if points.includes(" ")}
      <polyline {points} fill="none" stroke={color} stroke-width="1.5" stroke-linejoin="round" />
    {/if}
  {/each}
  {#each dots as [cx, cy] (cx + "-" + cy)}
    <circle {cx} {cy} r="1.4" fill={color} />
  {/each}
</svg>

<style>
  .sparkline {
    display: block;
    overflow: visible;
  }
</style>

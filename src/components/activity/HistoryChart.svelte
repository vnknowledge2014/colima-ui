<script lang="ts">
  /**
   * Stored metrics over a time range, as a line per container.
   *
   * Plain SVG. A charting library would be several hundred kilobytes for four
   * polylines and an axis, and the one behaviour that matters here — never
   * joining two points across missing data — is the thing those libraries
   * default to getting wrong.
   *
   * ## Gaps are drawn as gaps
   *
   * The machine sleeps, the daemon restarts, a slow client drops frames. Each
   * leaves a hole. Interpolating across it draws load that nobody observed, and
   * the resulting flat line is indistinguishable from a genuinely idle
   * container. So a jump larger than the expected step ends the current path and
   * starts a new one.
   */
  import type { HistorySeries } from "../../lib/api/metrics";
  import { t } from "../../lib/i18n.svelte";

  type Field = "cpuPct" | "memPct" | "memBytes";

  interface Props {
    series: HistorySeries;
    field: Field;
    /** Ids to draw. Empty draws every container in the series. */
    containerIds?: string[];
    height?: number;
  }

  let { series, field, containerIds = [], height = 220 }: Props = $props();

  const WIDTH = 800;
  const PAD_LEFT = 48;
  const PAD_BOTTOM = 22;
  const PAD_TOP = 8;

  /**
   * Distinct enough to tell apart, and readable on the dark background the app
   * uses. Cycled: beyond six containers a legend is doing the work anyway.
   */
  const COLOURS = ["#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#a855f7", "#06b6d4"];

  const shown = $derived.by(() => {
    const ids = containerIds.length
      ? containerIds
      : [...new Set(series.points.map((p) => p.containerId))];
    return ids.slice(0, COLOURS.length * 2);
  });

  const bounds = $derived.by(() => {
    const values = series.points.map((p) => p[field]);
    const maxValue = values.length ? Math.max(...values) : 0;
    return {
      minTs: series.points.length ? series.points[0].ts : 0,
      maxTs: series.points.length ? series.points[series.points.length - 1].ts : 1,
      // A little headroom, and never zero — a flat zero line still needs a scale.
      maxValue: maxValue > 0 ? maxValue * 1.1 : 1,
    };
  });

  function x(ts: number): number {
    const span = Math.max(1, bounds.maxTs - bounds.minTs);
    return PAD_LEFT + ((ts - bounds.minTs) / span) * (WIDTH - PAD_LEFT - 8);
  }

  function y(value: number): number {
    const usable = height - PAD_TOP - PAD_BOTTOM;
    return PAD_TOP + usable - (value / bounds.maxValue) * usable;
  }

  /**
   * One `path` per unbroken run of samples.
   *
   * The threshold is generous — two and a half expected steps — because the
   * collector's period is nominal: a busy daemon delivers late without that
   * being a gap in the data.
   */
  function pathsFor(containerId: string): string[] {
    const points = series.points.filter((p) => p.containerId === containerId);
    if (points.length === 0) return [];
    const maxStep = Math.max(series.expectedStepMs * 2.5, 1000);

    const paths: string[] = [];
    let current: string[] = [];
    let previousTs: number | null = null;

    for (const p of points) {
      if (previousTs !== null && p.ts - previousTs > maxStep) {
        if (current.length) paths.push(current.join(" "));
        current = [];
      }
      current.push(`${current.length === 0 ? "M" : "L"}${x(p.ts).toFixed(1)},${y(p[field]).toFixed(1)}`);
      previousTs = p.ts;
    }
    if (current.length) paths.push(current.join(" "));
    return paths;
  }

  function label(containerId: string): string {
    return series.names[containerId] ?? containerId.slice(0, 12);
  }

  function formatValue(v: number): string {
    if (field === "memBytes") {
      const units = ["B", "KB", "MB", "GB", "TB"];
      let value = v;
      let unit = 0;
      while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit++;
      }
      return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
    }
    return `${v.toFixed(0)}%`;
  }

  function formatTime(ts: number): string {
    const d = new Date(ts);
    // Date included once the range is longer than a day, where a bare clock
    // would put Tuesday and Thursday on the same tick.
    return bounds.maxTs - bounds.minTs > 86_400_000
      ? d.toLocaleDateString(undefined, { month: "short", day: "numeric" })
      : d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }
</script>

{#if series.points.length === 0}
  <p class="empty">
    {t("activity.history_empty", {
      default: "No history recorded for this range yet.",
    })}
  </p>
{:else}
  <svg
    viewBox="0 0 {WIDTH} {height}"
    preserveAspectRatio="none"
    role="img"
    aria-label={t("activity.history_chart_label", {
      default: "History chart, {count} containers",
      count: shown.length,
    })}
  >
    <!-- Horizontal guides at quarters. Enough to read a value off, few enough
         not to compete with the data. -->
    {#each [0, 0.25, 0.5, 0.75, 1] as fraction (fraction)}
      {@const value = bounds.maxValue * (1 - fraction)}
      {@const gy = PAD_TOP + (height - PAD_TOP - PAD_BOTTOM) * fraction}
      <line x1={PAD_LEFT} x2={WIDTH - 8} y1={gy} y2={gy} class="grid" />
      <text x={PAD_LEFT - 6} y={gy + 4} class="axis" text-anchor="end">{formatValue(value)}</text>
    {/each}

    {#each [0, 0.5, 1] as fraction (fraction)}
      {@const ts = bounds.minTs + (bounds.maxTs - bounds.minTs) * fraction}
      <text
        x={x(ts)}
        y={height - 6}
        class="axis"
        text-anchor={fraction === 0 ? "start" : fraction === 1 ? "end" : "middle"}
      >
        {formatTime(ts)}
      </text>
    {/each}

    {#each shown as id, i (id)}
      {#each pathsFor(id) as d, segment (segment)}
        <path {d} fill="none" stroke={COLOURS[i % COLOURS.length]} stroke-width="1.5" />
      {/each}
    {/each}
  </svg>

  <div class="legend">
    {#each shown as id, i (id)}
      <span class="entry">
        <span class="swatch" style="background: {COLOURS[i % COLOURS.length]}"></span>
        {label(id)}
      </span>
    {/each}
    <span class="resolution">
      {series.resolution === "raw"
        ? t("activity.resolution_raw", { default: "raw samples" })
        : t("activity.resolution_1m", { default: "1-minute averages" })}
    </span>
  </div>
{/if}

<style>
  svg {
    width: 100%;
    display: block;
  }
  .grid {
    stroke: var(--border-subtle, #30363d);
    stroke-width: 1;
  }
  .axis {
    fill: var(--text-muted);
    font-size: 10px;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin-top: 8px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  .entry {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .swatch {
    width: 10px;
    height: 3px;
    border-radius: 2px;
  }
  .resolution {
    margin-left: auto;
    color: var(--text-muted);
  }
  .empty {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
</style>

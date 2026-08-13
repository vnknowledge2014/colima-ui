<script lang="ts">
  /**
   * This image's score over time.
   *
   * The line breaks wherever the rule pack or the scanner changed, because two
   * scores measured by different rulers are not points on the same line. Each
   * stretch is labelled with what measured it, and the break is left visible
   * rather than smoothed over.
   */
  import { securityApi, type ScanRun } from "../../lib/api";
  import { t } from "../../lib/i18n.svelte";
  import { segmentDelta, segmentRuns, type TrendSegment } from "./score-trend";

  let { imageDigest }: { imageDigest: string } = $props();

  let runs = $state<ScanRun[]>([]);
  let loading = $state(false);
  let error = $state("");

  const segments = $derived<TrendSegment[]>(segmentRuns(runs));

  $effect(() => {
    void imageDigest;
    void load();
  });

  async function load() {
    loading = true;
    error = "";
    try {
      runs = (await securityApi.history(imageDigest)) || [];
    } catch (e) {
      error = String(e);
      runs = [];
    } finally {
      loading = false;
    }
  }

  const WIDTH = 100;
  const HEIGHT = 40;

  /** Map one segment's runs onto the shared 0–100 score axis. */
  function points(segment: TrendSegment, index: number, total: number): string {
    // Each segment gets its own horizontal band, in time order, so a break is
    // a visible gap rather than a jump in the same line.
    const bandWidth = WIDTH / Math.max(total, 1);
    const left = bandWidth * index;
    const n = segment.runs.length;
    return segment.runs
      .map((run, i) => {
        const x = left + (n === 1 ? bandWidth / 2 : (bandWidth * i) / (n - 1));
        const y = HEIGHT - (run.score / 100) * HEIGHT;
        return `${x.toFixed(2)},${y.toFixed(2)}`;
      })
      .join(" ");
  }

  function when(ms: number): string {
    return new Date(ms).toLocaleDateString();
  }
</script>

{#if loading}
  <p class="hint-text">{t("security.history.loading", { default: "Loading history…" })}</p>
{:else if error}
  <p class="hint-text">{error}</p>
{:else if runs.length === 0}
  <p class="hint-text">
    {t("security.history.empty", {
      default: "No history yet. Each scan of this image adds a point.",
    })}
  </p>
{:else}
  <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="none" style="width: 100%; height: 120px;" role="img" aria-label={t("security.history.title", { default: "Score over time" })}>
    {#each segments as segment, i (segment.packVersion + i)}
      {#if segment.runs.length > 1}
        <polyline
          points={points(segment, i, segments.length)}
          fill="none"
          stroke="var(--accent-green)"
          stroke-width="1"
          vector-effect="non-scaling-stroke"
        />
      {/if}
      {#each segment.runs as run, j (run.scannedAt)}
        {@const coords = points(segment, i, segments.length).split(" ")[j].split(",")}
        <circle cx={coords[0]} cy={coords[1]} r="1" fill="var(--accent-green)" vector-effect="non-scaling-stroke" />
      {/each}
    {/each}
  </svg>

  <div style="display: flex; flex-direction: column; gap: 4px; font-size: var(--text-xs); color: var(--text-muted);">
    {#each segments as segment, i (segment.packVersion + i)}
      {@const delta = segmentDelta(segment)}
      <div>
        {#if i > 0}
          <!-- The break, stated. Without this the gap looks like missing data
               rather than a deliberate refusal to compare. -->
          <strong style="color: var(--accent-yellow);">
            {t("security.history.break", { default: "Ruler changed — scores before and after are not comparable." })}
          </strong>
        {/if}
        {t("security.history.measured_by", { default: "rule pack" })} {segment.packVersion} · {segment.scanner}
        · {when(segment.runs[0].scannedAt)}–{when(segment.runs[segment.runs.length - 1].scannedAt)}
        · {segment.runs.length} {t("security.history.scans", { default: "scans" })}
        {#if delta !== null}
          · <span style="color: {delta >= 0 ? 'var(--accent-green)' : 'var(--accent-red)'};">
            {delta >= 0 ? "+" : ""}{delta}
          </span>
        {/if}
      </div>
    {/each}
  </div>
{/if}

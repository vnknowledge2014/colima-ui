<script lang="ts">
  /**
   * The state of the machine, above everything you can do to it.
   *
   * The number is never shown alone. A score means nothing without the pack that
   * defined it, the scanner that produced it, and how much of the machine it
   * covers — so those sit next to it rather than behind a tooltip.
   */
  import { t } from "../../lib/i18n.svelte";
  import type { Level } from "../../lib/api/security";
  import { SEVERITY_ORDER, type Posture } from "./security-posture";

  let {
    posture,
    level,
    busy = false,
    onChangeLevel,
    onReload,
  }: {
    posture: Posture;
    level: Level;
    busy?: boolean;
    onChangeLevel: (next: Level) => void;
    onReload: () => void;
  } = $props();

  /**
   * Bands, not a gradient: a score is a summary of rules that passed or failed,
   * and pretending to a precision of one point would oversell it.
   */
  const band = $derived(
    posture.average === null ? "none" : posture.average >= 80 ? "good" : posture.average >= 55 ? "fair" : "poor",
  );

  const shown = $derived(SEVERITY_ORDER.filter((s) => posture.severity[s] > 0));
</script>

<section class="posture card" aria-label={t("security.posture", { default: "Security posture" })}>
  <div class="headline">
    <div class="score-block">
      {#if posture.average === null}
        <div class="score none">—</div>
        <div class="score-label">{t("security.posture_empty", { default: "Nothing scanned yet" })}</div>
      {:else}
        <div class="score {band}">
          {posture.average}<span class="max">/100</span>
        </div>
        <div class="score-label">
          {t("security.posture_coverage", { default: "average over scanned images" })}
          · {posture.scanned}/{posture.total}
        </div>
      {/if}
    </div>

    <div class="controls">
      <label class="level">
        {t("security.level", { default: "Strictness" })}
        <select
          class="input select"
          value={level}
          onchange={(e) => onChangeLevel(e.currentTarget.value as Level)}
          disabled={busy}
        >
          <option value="l1">L1 · {t("security.level_l1", { default: "essentials" })}</option>
          <option value="l2">L2 · {t("security.level_l2", { default: "recommended" })}</option>
          <option value="l3">L3 · {t("security.level_l3", { default: "strict" })}</option>
        </select>
      </label>
      <button class="btn btn-ghost" onclick={onReload} disabled={busy}>
        {t("security.reload", { default: "Reload images" })}
      </button>
    </div>
  </div>

  {#if posture.average === null}
    <p class="note">
      {t("security.posture_empty_text", {
        default: "Scanning is something you ask for — pick an image and scan it, and its score lands here.",
      })}
    </p>
  {:else}
    <div class="facts">
      {#if shown.length > 0}
        <ul class="severities">
          {#each shown as sev (sev)}
            <li class="sev sev-{sev}">
              <span class="dot" aria-hidden="true"></span>
              {posture.severity[sev]}
              {t(`security.severity_${sev}`, { default: sev })}
            </li>
          {/each}
        </ul>
      {:else}
        <span class="clean">{t("security.no_findings", { default: "No known vulnerabilities" })}</span>
      {/if}

      {#if posture.lowest}
        <span class="lowest">
          {t("security.posture_lowest", { default: "Lowest" })}:
          <code>{posture.lowest.ref}</code>
          <strong>{posture.lowest.score}</strong>
        </span>
      {/if}
    </div>

    {#if posture.inputs}
      <p class="provenance">
        {posture.inputs.scanner}
        {posture.inputs.scannerVersion} · {t("security.pack", { default: "pack" })}
        {posture.inputs.packVersion}
        {#if posture.inputs.dbSnapshotDate}
          · {t("security.db", { default: "database" })}
          {new Date(posture.inputs.dbSnapshotDate).toLocaleDateString()}
        {/if}
      </p>
    {/if}

    {#if posture.mixed}
      <p class="mixed">
        {t("security.posture_mixed", {
          default:
            "These scores came from different scanner or rule-pack versions, so they are not comparable. Scan again to line them up.",
        })}
      </p>
    {/if}
  {/if}
</section>

<style>
  .posture {
    padding: 16px;
    margin-bottom: 16px;
  }
  .headline {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .score {
    font-size: 2.25rem;
    line-height: 1.1;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .score .max {
    font-size: var(--text-base);
    font-weight: 400;
    color: var(--text-muted);
  }
  .score.good {
    color: var(--accent-green, #22c55e);
  }
  .score.fair {
    color: var(--accent-yellow, #f59e0b);
  }
  .score.poor {
    color: var(--accent-red, #ef4444);
  }
  .score.none {
    color: var(--text-muted);
  }
  .score-label {
    margin-top: 2px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .level {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  /* .input is width:100% for form columns; inline next to a label it should
     size to its own content instead of eating the row. */
  .level :global(select) {
    width: auto;
  }
  .facts {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    margin-top: 12px;
  }
  .severities {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sev {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }
  /* The count and the word carry the meaning; the dot only repeats it, so a
     reader who cannot separate the colours loses nothing. */
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: currentColor;
  }
  .sev-critical .dot {
    color: var(--sev-critical);
  }
  .sev-high .dot {
    color: var(--sev-high);
  }
  .sev-medium .dot {
    color: var(--sev-medium);
  }
  .sev-low .dot {
    color: var(--sev-low);
  }
  .sev-unknown .dot {
    color: var(--sev-unknown);
  }
  .clean,
  .lowest {
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .lowest code {
    font-family: var(--font-mono, ui-monospace, monospace);
    word-break: break-all;
  }
  .note,
  .provenance,
  .mixed {
    margin: 10px 0 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .provenance {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .mixed {
    color: var(--accent-yellow, #f59e0b);
  }
</style>

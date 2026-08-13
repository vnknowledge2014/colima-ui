<script lang="ts">
  /**
   * The score, as four numbers and the three facts that make them mean anything.
   *
   * The total is here to sort a list. On its own it says nothing useful: 46/100
   * could be a pile of CVEs or a root user on a moving tag, and those call for
   * completely different work. So the components are the headline and the total
   * is the small print, not the other way round.
   *
   * `ScoreInputs` is rendered in the open, never in a tooltip. The same image
   * scores differently against a newer vulnerability database, and two scanners
   * disagree by more than tenfold on identical input — a bare number invites a
   * comparison that is not valid.
   */
  import type { ScoreBreakdown, ComponentScore } from "../../lib/api/security";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    score: ScoreBreakdown;
  }

  let { score }: Props = $props();

  const components = $derived([
    {
      key: "vulnerabilities",
      label: t("security.component_vulnerabilities", { default: "Vulnerabilities" }),
      value: score.vulnerabilities,
    },
    {
      key: "hardening",
      label: t("security.component_hardening", { default: "Hardening" }),
      value: score.hardening,
    },
    {
      key: "provenance",
      label: t("security.component_provenance", { default: "Provenance" }),
      value: score.provenance,
    },
    {
      key: "freshness",
      label: t("security.component_freshness", { default: "Freshness" }),
      value: score.freshness,
    },
  ]);

  /** Colour follows the ratio, and the number is always spelled out beside it. */
  function tone(c: ComponentScore): string {
    if (c.max === 0) return "unknown";
    const ratio = c.earned / c.max;
    if (ratio >= 0.8) return "good";
    if (ratio >= 0.5) return "fair";
    return "poor";
  }

  function percent(c: ComponentScore): number {
    return c.max === 0 ? 0 : Math.round((c.earned / c.max) * 100);
  }
</script>

<div class="card score-card">
  <div class="total">
    <div class="total-number tone-{tone({ earned: score.total, max: 100, failedRules: [] })}">
      {score.total}
      <span class="total-max">/100</span>
    </div>
    <p class="total-caption">
      {t("security.total_caption", {
        default: "For sorting a list. The four numbers below are the answer.",
      })}
    </p>
  </div>

  <div class="bars">
    {#each components as c (c.key)}
      <div class="bar-row">
        <span class="bar-label">{c.label}</span>
        <!-- A component whose rules were all skipped has a maximum of zero, and
             a meter whose range is 0..0 describes nothing. -->
        <div
          class="bar"
          role="meter"
          aria-valuemin="0"
          aria-valuemax={Math.max(c.value.max, 1)}
          aria-valuenow={c.value.earned}
          aria-label={t("security.component_aria", {
            default: "{label}: {earned} of {max}",
            label: c.label,
            earned: c.value.earned,
            max: c.value.max,
          })}
        >
          <div class="fill tone-{tone(c.value)}" style="width: {percent(c.value)}%"></div>
        </div>
        <span class="bar-value">{c.value.earned}<span class="bar-max">/{c.value.max}</span></span>
      </div>
    {/each}
  </div>

  <!-- Always visible. A score without these cannot be compared to another one,
       and hiding them behind a hover is how that gets forgotten. -->
  <dl class="inputs">
    <div><dt>{t("security.input_scanner", { default: "Scanner" })}</dt><dd>{score.inputs.scanner} {score.inputs.scannerVersion}</dd></div>
    <div>
      <dt>{t("security.input_db", { default: "Vulnerability DB" })}</dt>
      <dd>{score.inputs.dbSnapshotDate?.slice(0, 10) ?? t("security.input_db_unknown", { default: "unknown" })}</dd>
    </div>
    <div><dt>{t("security.input_pack", { default: "Rule pack" })}</dt><dd>{score.inputs.packVersion}</dd></div>
    <div><dt>{t("security.input_level", { default: "Level" })}</dt><dd>{score.inputs.level.toUpperCase()}</dd></div>
  </dl>

  {#if score.inputs.skippedRules?.length}
    <p class="skipped">
      {t("security.skipped_rules", {
        default:
          "{count} rule(s) in this pack could not be run by this build, so their points are out of the total rather than counted as passed.",
        count: score.inputs.skippedRules.length,
      })}
    </p>
  {/if}
</div>

<style>
  .score-card {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .total {
    display: flex;
    align-items: baseline;
    gap: 12px;
    flex-wrap: wrap;
  }
  .total-number {
    font-size: 2.5rem;
    font-weight: 700;
    line-height: 1;
  }
  .total-max {
    font-size: var(--text-sm);
    font-weight: 400;
    color: var(--text-muted);
  }
  .total-caption {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    max-width: 32ch;
  }
  .bars {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .bar-row {
    display: grid;
    grid-template-columns: 9rem 1fr 4rem;
    align-items: center;
    gap: 10px;
  }
  .bar-label {
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .bar {
    height: 8px;
    background: var(--bg-app);
    border-radius: 4px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 4px;
  }
  .bar-value {
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .bar-max {
    color: var(--text-muted);
  }
  .tone-good {
    color: var(--accent-green, #22c55e);
    background: var(--accent-green, #22c55e);
  }
  .tone-fair {
    color: var(--accent-yellow, #f59e0b);
    background: var(--accent-yellow, #f59e0b);
  }
  .tone-poor {
    color: var(--accent-red, #ef4444);
    background: var(--accent-red, #ef4444);
  }
  .tone-unknown {
    color: var(--text-muted);
    background: var(--text-muted);
  }
  .total-number.tone-good,
  .total-number.tone-fair,
  .total-number.tone-poor {
    background: none;
  }
  .inputs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 20px;
    margin: 0;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle, #30363d);
  }
  .inputs div {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .inputs dt {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .inputs dd {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .skipped {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
</style>

<script lang="ts">
  /**
   * The configuration checklist: what this image does, and what to change.
   *
   * Failures first, and each one carries the remediation text from the rule pack
   * — the point of the checklist is the next action, not the verdict. Passing
   * rules stay visible but collapsed, because "we checked and it was fine" is
   * also information, just not urgent information.
   *
   * Rule text is rendered as plain text. It comes from a pack that will one day
   * be downloadable, and `{@html}` on downloadable content is how a rule pack
   * becomes a script.
   */
  import type { Evaluation, Rule, RulePack, RuleResult } from "../../lib/api/security";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    evaluation: Evaluation;
    pack: RulePack | null;
    /** Opens the offline Knowledge Base article that explains these rules. */
    onOpenGuide?: () => void;
  }

  let { evaluation, pack, onOpenGuide }: Props = $props();

  let showPassed = $state(false);

  function ruleOf(result: RuleResult): Rule | undefined {
    return pack?.rules.find((r) => r.id === result.ruleId);
  }

  const failed = $derived(evaluation.results.filter((r) => !r.passed));
  const passed = $derived(evaluation.results.filter((r) => r.passed));
</script>

<div class="checklist">
  {#if failed.length === 0}
    <p class="all-clear">
      {t("security.rules_all_pass", {
        default: "Every configuration rule at this level passes.",
      })}
    </p>
  {/if}

  {#each failed as result (result.ruleId)}
    {@const rule = ruleOf(result)}
    <div class="rule failed sev-{result.severity}">
      <div class="rule-head">
        <span class="marker" aria-hidden="true">✕</span>
        <span class="rule-title">{rule?.title ?? result.ruleId}</span>
        <span class="weight">−{result.weight}</span>
      </div>
      {#if result.evidence}
        <p class="evidence">{result.evidence}</p>
      {/if}
      {#if rule}
        <p class="rationale">{rule.rationale}</p>
        <p class="remediation">{rule.remediation}</p>
        {#if rule.standardRefs.length}
          <p class="refs">
            {t("security.references", { default: "References" })}:
            {#each rule.standardRefs as ref, i (ref.standard + ref.id)}{i > 0 ? " · " : ""}{ref.standard}{ref.version ? ` v${ref.version}` : ""} §{ref.id}{/each}
          </p>
        {/if}
      {/if}
    </div>
  {/each}

  {#if evaluation.skipped.length}
    <p class="skipped">
      {t("security.rules_skipped", {
        default:
          "{count} rule(s) in the pack are newer than this build and were not run.",
        count: evaluation.skipped.length,
      })}
    </p>
  {/if}

  {#if passed.length}
    <button class="link" onclick={() => (showPassed = !showPassed)}>
      {showPassed
        ? t("security.hide_passed", { default: "Hide passing rules" })
        : t("security.show_passed", { default: "Show {count} passing rules", count: passed.length })}
    </button>
    {#if showPassed}
      <ul class="passed">
        {#each passed as result (result.ruleId)}
          <li><span class="marker" aria-hidden="true">✓</span> {ruleOf(result)?.title ?? result.ruleId}</li>
        {/each}
      </ul>
    {/if}
  {/if}

  {#if onOpenGuide}
    <button class="link" onclick={onOpenGuide}>
      {t("security.open_guide", { default: "Read the image hardening guide" })}
    </button>
  {/if}
</div>

<style>
  .checklist {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .rule {
    border-left: 3px solid var(--border-primary);
    padding: 8px 0 8px 12px;
  }
  .rule.sev-critical {
    border-left-color: var(--accent-red, #ef4444);
  }
  .rule.sev-high {
    border-left-color: var(--accent-orange, #f97316);
  }
  .rule.sev-medium {
    border-left-color: var(--accent-yellow, #f59e0b);
  }
  .rule-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .rule-title {
    flex: 1;
    font-weight: 600;
    font-size: var(--text-sm);
  }
  .weight {
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .marker {
    color: var(--text-muted);
  }
  .failed .marker {
    color: var(--accent-red, #ef4444);
  }
  .evidence,
  .rationale,
  .remediation,
  .refs {
    margin: 4px 0 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .evidence {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-xs);
    color: var(--text-muted);
    word-break: break-all;
  }
  .remediation {
    color: var(--text-primary);
  }
  .refs {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .all-clear,
  .skipped {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .passed {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .link {
    align-self: flex-start;
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-blue, #3b82f6);
    cursor: pointer;
    font: inherit;
    font-size: var(--text-sm);
  }
  .link:hover {
    text-decoration: underline;
  }
</style>

<script lang="ts">
  /**
   * Everything self-healing did, and everything it declined to do.
   *
   * Blocked firings are listed beside executed ones rather than filtered out.
   * A log that only records successes cannot answer the question people
   * actually bring to it — "why did nothing happen?" — and the answers here
   * (quota spent, switch off) are the ones that make the difference between a
   * broken feature and one working as configured.
   */
  import { t } from "../../lib/i18n.svelte";
  import type { HealLogEntry, HealOutcome } from "../../lib/api/self-heal";

  let { entries }: { entries: HealLogEntry[] } = $props();

  function outcomeLabel(outcome: HealOutcome): string {
    switch (outcome) {
      case "executed":
        return t("self_heal.outcome_executed", { default: "done" });
      case "failed":
        return t("self_heal.outcome_failed", { default: "failed" });
      case "suggested":
        return t("self_heal.outcome_suggested", { default: "suggested" });
      case "quota_blocked":
        return t("self_heal.outcome_quota", { default: "over hourly limit" });
      case "switched_off":
        return t("self_heal.outcome_off", { default: "switched off" });
    }
  }

  function when(ts: number): string {
    return new Date(ts).toLocaleString();
  }
</script>

{#if entries.length === 0}
  <p class="hint-text">
    {t("self_heal.log_empty", {
      default: "Nothing yet. Entries appear here whenever a rule fires, including when one is held back.",
    })}
  </p>
{:else}
  <ul class="log">
    {#each entries as entry (entry.id)}
      <li>
        <div class="line">
          <span class="badge {entry.outcome}">{outcomeLabel(entry.outcome)}</span>
          <span class="rule">{entry.ruleName}</span>
          {#if entry.containerName}
            <code class="target">{entry.containerName}</code>
          {/if}
          <time datetime={new Date(entry.ts).toISOString()}>{when(entry.ts)}</time>
        </div>
        <p class="detail">{entry.detail}</p>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .log {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  li {
    padding: 8px 10px;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: var(--text-sm);
  }
  .badge {
    flex-shrink: 0;
    padding: 1px 7px;
    border: 1px solid var(--border-primary);
    border-radius: 999px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  /* Only the two outcomes that changed the machine are coloured. Painting the
     held-back ones red would read as breakage rather than as a limit doing
     its job. */
  .badge.executed {
    color: var(--accent-green, #3fb950);
    border-color: currentColor;
  }
  .badge.failed {
    color: var(--sev-high, #f85149);
    border-color: currentColor;
  }
  .rule {
    font-weight: 500;
  }
  .target {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  time {
    margin-left: auto;
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .detail {
    margin: 4px 0 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
</style>

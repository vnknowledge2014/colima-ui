<script lang="ts">
  /**
   * Create and edit threshold rules.
   *
   * ## The backtest is the point
   *
   * "80% for five minutes" is a guess until you know it would have fired eleven
   * times last week. Without that number people either set a threshold nothing
   * ever crosses, or one that pages them hourly until they turn the feature off.
   * The preview runs the same evaluator the live path uses, over stored history.
   *
   * ## Two conditions, both enforced
   *
   * The duration is what makes an alert mean "still happening" rather than
   * "briefly touched"; the cooldown is what keeps one incident from becoming
   * forty notifications. The backend refuses a zero for either, and the inputs
   * here carry the same minimums so the refusal is not a surprise.
   */
  import {
    alertsApi,
    type AlertMetric,
    type AlertRule,
    type BacktestResult,
  } from "../../lib/api/metrics";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    rules: AlertRule[];
    /** Containers to offer by name; a rule may also apply to all of them. */
    containers: Array<{ id: string; name: string }>;
    onChanged: () => void;
  }

  let { rules, containers, onChanged }: Props = $props();

  function blank(): AlertRule {
    return {
      id: 0,
      name: "",
      metric: "cpu_pct",
      threshold: 80,
      durationSecs: 300,
      cooldownSecs: 900,
      enabled: true,
    };
  }

  let draft = $state<AlertRule>(blank());
  let saving = $state(false);
  let backtest = $state<BacktestResult | null>(null);
  let backtesting = $state(false);

  const METRICS: Array<{ value: AlertMetric; label: string }> = [
    { value: "cpu_pct", label: t("activity.metric_cpu", { default: "CPU %" }) },
    { value: "mem_pct", label: t("activity.metric_mem_pct", { default: "Memory %" }) },
    { value: "mem_bytes", label: t("activity.metric_mem_bytes", { default: "Memory bytes" }) },
  ];

  function edit(rule: AlertRule) {
    draft = { ...rule };
    backtest = null;
  }

  async function save() {
    saving = true;
    try {
      await alertsApi.saveRule(draft);
      draft = blank();
      backtest = null;
      onChanged();
    } catch (e) {
      // Shown, not swallowed: the backend refuses durations and cooldowns that
      // would defeat the rule, and the user needs to know which one it was.
      globalToast("error", String(e));
    } finally {
      saving = false;
    }
  }

  async function remove(rule: AlertRule) {
    try {
      await alertsApi.deleteRule(rule.id);
      if (draft.id === rule.id) draft = blank();
      onChanged();
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function toggle(rule: AlertRule) {
    try {
      await alertsApi.saveRule({ ...rule, enabled: !rule.enabled });
      onChanged();
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  /** Replay the draft over the last seven days of stored history. */
  async function preview() {
    backtesting = true;
    backtest = null;
    try {
      const to = Date.now();
      backtest = await alertsApi.backtest(draft, to - 7 * 86_400_000, to);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      backtesting = false;
    }
  }

  function describe(rule: AlertRule): string {
    const metric = METRICS.find((m) => m.value === rule.metric)?.label ?? rule.metric;
    const scope = rule.containerId
      ? containers.find((c) => c.id === rule.containerId)?.name ?? rule.containerId.slice(0, 12)
      : t("activity.all_containers", { default: "any container" });
    return `${metric} > ${rule.threshold} · ${Math.round(rule.durationSecs / 60)}m · ${scope}`;
  }
</script>

<div class="editor">
  <ul class="rules">
    {#each rules as rule (rule.id)}
      <li class:disabled={!rule.enabled}>
        <div class="rule-main">
          <span class="rule-name">{rule.name}</span>
          <span class="rule-detail">{describe(rule)}</span>
        </div>
        <div class="rule-actions">
          <button class="link" onclick={() => toggle(rule)}>
            {rule.enabled
              ? t("activity.disable", { default: "Disable" })
              : t("activity.enable", { default: "Enable" })}
          </button>
          <button class="link" onclick={() => edit(rule)}>{t("activity.edit", { default: "Edit" })}</button>
          <button class="link danger" onclick={() => remove(rule)}>
            {t("activity.delete", { default: "Delete" })}
          </button>
        </div>
      </li>
    {:else}
      <li class="none">
        {t("activity.no_rules", { default: "No rules yet. The form below creates one." })}
      </li>
    {/each}
  </ul>

  <form class="form" onsubmit={(e) => { e.preventDefault(); save(); }}>
    <label>
      {t("activity.rule_name", { default: "Name" })}
      <input class="input" bind:value={draft.name} placeholder="web is pegged" required />
    </label>

    <label>
      {t("activity.rule_metric", { default: "Metric" })}
      <select class="input" bind:value={draft.metric}>
        {#each METRICS as m (m.value)}
          <option value={m.value}>{m.label}</option>
        {/each}
      </select>
    </label>

    <label>
      {t("activity.rule_threshold", { default: "Above" })}
      <input class="input" type="number" step="any" bind:value={draft.threshold} required />
    </label>

    <label>
      {t("activity.rule_duration", { default: "For (minutes)" })}
      <input
        class="input"
        type="number"
        min="1"
        value={Math.round(draft.durationSecs / 60)}
        oninput={(e) => (draft.durationSecs = Math.max(5, Number(e.currentTarget.value) * 60))}
      />
    </label>

    <label>
      {t("activity.rule_cooldown", { default: "Then quiet for (minutes)" })}
      <input
        class="input"
        type="number"
        min="1"
        value={Math.round(draft.cooldownSecs / 60)}
        oninput={(e) => (draft.cooldownSecs = Math.max(30, Number(e.currentTarget.value) * 60))}
      />
    </label>

    <label>
      {t("activity.rule_scope", { default: "Container" })}
      <select class="input" bind:value={draft.containerId}>
        <option value={undefined}>{t("activity.all_containers", { default: "any container" })}</option>
        {#each containers as c (c.id)}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
    </label>

    <div class="actions">
      <button type="button" class="btn btn-ghost" onclick={preview} disabled={backtesting}>
        {backtesting
          ? t("activity.backtesting", { default: "Checking…" })
          : t("activity.backtest", { default: "Test against last 7 days" })}
      </button>
      <button type="submit" class="btn btn-primary" disabled={saving || !draft.name.trim()}>
        {draft.id
          ? t("activity.update_rule", { default: "Update rule" })
          : t("activity.add_rule", { default: "Add rule" })}
      </button>
      {#if draft.id}
        <button type="button" class="link" onclick={() => (draft = blank())}>
          {t("activity.cancel_edit", { default: "Cancel" })}
        </button>
      {/if}
    </div>
  </form>

  {#if backtest}
    <p class="backtest">
      {#if backtest.sampleCount === 0}
        {t("activity.backtest_no_data", {
          default: "No history stored for that window yet, so there is nothing to test against.",
        })}
      {:else}
        {t("activity.backtest_result", {
          default: "Would have fired {count} times across {samples} stored samples.",
          count: backtest.wouldFire,
          samples: backtest.sampleCount,
        })}
        <span class="caveat">
          {t("activity.backtest_caveat", {
            default:
              "Anything older than an hour is stored as per-minute averages, so brief spikes count for less here than they will live.",
          })}
        </span>
      {/if}
    </p>
  {/if}
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .rules {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rules li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border: 1px solid var(--border-subtle, #30363d);
    border-radius: 6px;
    font-size: var(--text-sm);
  }
  .rules li.disabled {
    opacity: 0.55;
  }
  .rules li.none {
    justify-content: flex-start;
    color: var(--text-secondary);
    border-style: dashed;
  }
  .rule-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .rule-name {
    font-weight: 600;
  }
  .rule-detail {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .rule-actions {
    display: flex;
    gap: 10px;
    flex: none;
  }
  .form {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    align-items: end;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .actions {
    display: flex;
    gap: 10px;
    align-items: center;
    grid-column: 1 / -1;
  }
  .backtest {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .caveat {
    display: block;
    margin-top: 4px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .link {
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
  .link.danger {
    color: var(--accent-red, #ef4444);
  }
</style>

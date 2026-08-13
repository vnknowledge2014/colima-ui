<script lang="ts">
  /**
   * The bar the user sets for their own images.
   *
   * No default rule is created, and no default threshold is suggested. A number
   * chosen without the score distribution behind it would be arbitrary, and an
   * arbitrary rule that fires teaches the user to ignore the alert — which is
   * worse than having no policy at all.
   *
   * Every rule warns. Blocking a container is a separate decision that has not
   * been taken; the UI says so rather than leaving an action dropdown that has
   * one entry and hints at more.
   */
  import { securityApi, type PolicyRule, type Severity } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  let rules = $state<PolicyRule[]>([]);
  let loaded = $state(false);

  let draftName = $state("");
  let draftPattern = $state("*");
  let draftMinScore = $state<number | null>(null);
  let draftMaxSeverity = $state<Severity | "">("");

  const SEVERITIES: Severity[] = ["critical", "high", "medium", "low"];

  async function load() {
    try {
      rules = (await securityApi.policyList()) || [];
    } catch {
      // Not entitled, or nothing stored. Either way there is nothing to show
      // and an error toast on page load would be noise.
      rules = [];
    }
    loaded = true;
  }

  $effect(() => {
    if (!loaded) void load();
  });

  async function add() {
    if (!draftName.trim()) return;
    if (draftMinScore === null && !draftMaxSeverity) {
      globalToast("error", t("security.policy.needs_a_bar", { default: "Set a minimum score or a maximum severity — a rule with neither checks nothing." }));
      return;
    }
    try {
      await securityApi.policySave({
        id: 0,
        name: draftName.trim(),
        pattern: draftPattern.trim() || "*",
        minScore: draftMinScore ?? undefined,
        maxSeverity: draftMaxSeverity || undefined,
        action: "warn",
        enabled: true,
      });
      draftName = "";
      draftMinScore = null;
      draftMaxSeverity = "";
      await load();
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function toggle(rule: PolicyRule) {
    try {
      await securityApi.policySave({ ...rule, enabled: !rule.enabled });
      await load();
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function remove(rule: PolicyRule) {
    try {
      await securityApi.policyDelete(rule.id);
      await load();
    } catch (e) {
      globalToast("error", String(e));
    }
  }
</script>

<div style="display: flex; flex-direction: column; gap: 10px;">
  {#if rules.length === 0}
    <p class="hint-text">
      {t("security.policy.empty", {
        default: "No policy yet. A policy warns you when an image falls below a bar you set — it never stops a container.",
      })}
    </p>
  {:else}
    {#each rules as rule (rule.id)}
      <div style="display: flex; align-items: center; gap: 8px; font-size: var(--text-sm);">
        <input type="checkbox" checked={rule.enabled} onchange={() => toggle(rule)} />
        <span style="flex: 1;">
          <strong>{rule.name}</strong>
          <span style="color: var(--text-muted); font-family: var(--font-mono); font-size: var(--text-xs);">
            {rule.pattern}
          </span>
          <span style="color: var(--text-secondary); font-size: var(--text-xs);">
            {#if rule.minScore !== undefined}
              · {t("security.policy.min_score", { default: "min score" })} {rule.minScore}
            {/if}
            {#if rule.maxSeverity}
              · {t("security.policy.max_severity", { default: "max severity" })} {rule.maxSeverity}
            {/if}
          </span>
        </span>
        <button class="btn btn-sm" onclick={() => remove(rule)}>
          {t("security.policy.remove", { default: "Remove" })}
        </button>
      </div>
    {/each}
  {/if}

  <div style="display: flex; gap: 6px; flex-wrap: wrap; align-items: center;">
    <input
      class="input"
      style="flex: 1; min-width: 140px;"
      placeholder={t("security.policy.name_placeholder", { default: "Rule name" })}
      bind:value={draftName}
    />
    <input
      class="input"
      style="width: 130px; font-family: var(--font-mono); font-size: var(--text-xs);"
      placeholder={t("security.policy.pattern_placeholder", { default: "* or nginx*" })}
      bind:value={draftPattern}
    />
    <input
      class="input"
      style="width: 110px;"
      type="number"
      min="0"
      max="100"
      placeholder={t("security.policy.min_score", { default: "min score" })}
      bind:value={draftMinScore}
    />
    <select class="input" style="width: 150px;" bind:value={draftMaxSeverity}>
      <option value="">{t("security.policy.any_severity", { default: "any severity" })}</option>
      {#each SEVERITIES as severity (severity)}
        <option value={severity}>{t("security.policy.max_severity", { default: "max severity" })}: {severity}</option>
      {/each}
    </select>
    <button class="btn btn-sm btn-primary" onclick={add}>
      {t("security.policy.add", { default: "Add rule" })}
    </button>
  </div>

  <p class="hint-text" style="font-size: var(--text-xs);">
    {t("security.policy.warn_only", {
      default: "Policies warn. They never stop or block a container.",
    })}
  </p>
</div>

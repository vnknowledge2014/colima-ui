<script lang="ts">
  /**
   * Self-healing: the few repairs this app may make on its own.
   *
   * ## The brake sits above everything it stops
   *
   * The master switch renders first and unconditionally. It is the control that
   * stops every rule at once, including anything already waiting to run, so it
   * must never be nested inside something that can stop rendering.
   *
   * ## Turning a rule to Auto is a decision, so it is asked as one
   *
   * Switching a rule from Suggest to Auto is confirmed against a sentence that
   * says what will happen to the machine — not "enable automatic mode?", which
   * describes a setting rather than a consequence.
   *
   * ## Two rules have no Auto to offer
   *
   * `autoCapable` comes from the backend, which has no code path that could run
   * them. The toggle is not rendered for those, rather than rendered disabled:
   * a greyed-out switch suggests a permission that could be granted.
   */
  import { onMount, tick } from "svelte";
  import {
    selfHealApi,
    type HealLogEntry,
    type HealRule,
  } from "../../lib/api/self-heal";
  import { uiState } from "../../store.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import SettingsSection from "../../components/settings/SettingsSection.svelte";
  import HealLog from "../../components/activity/HealLog.svelte";

  let rules = $state<HealRule[]>([]);
  let log = $state<HealLogEntry[]>([]);
  let enabled = $state(true);
  let loaded = $state(false);
  /** The rule awaiting confirmation of its move to Auto. */
  let confirming = $state<HealRule | null>(null);

  /** The section card, so the Activity banner's link can scroll to it. */
  let sectionEl = $state<HTMLDivElement | null>(null);

  onMount(load);

  /**
   * An `$effect` rather than `onMount`: Settings is not remounted when the user
   * is already on the page, so a second click from the banner has to scroll
   * again. Clearing the flag re-runs this once with null and settles.
   */
  $effect(() => {
    if (uiState.settingsSection !== "self-healing") return;
    uiState.settingsSection = null;
    tick().then(() => {
      sectionEl?.scrollIntoView({ behavior: "smooth", block: "center" });
    });
  });

  async function load() {
    try {
      [rules, log, enabled] = await Promise.all([
        selfHealApi.listRules(),
        selfHealApi.recentLog(50),
        selfHealApi.isEnabled(),
      ]);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      loaded = true;
    }
  }

  async function toggleMaster(on: boolean) {
    // Optimistic, then corrected: the switch has to feel immediate, but the
    // backend's answer is the one that decides whether anything acts.
    enabled = on;
    try {
      await selfHealApi.setEnabled(on);
    } catch (e) {
      globalToast("error", String(e));
      enabled = await selfHealApi.isEnabled().catch(() => !on);
    }
  }

  async function save(rule: HealRule, patch: Partial<HealRule>) {
    const next = { ...rule, ...patch };
    try {
      await selfHealApi.saveRule(
        next.id,
        next.mode,
        next.threshold,
        next.maxPerHour,
        next.enabled,
      );
      rules = rules.map((r) => (r.id === next.id ? next : r));
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  /** Suggest → Auto asks first; Auto → Suggest is a retreat and does not. */
  function requestMode(rule: HealRule, auto: boolean) {
    if (!auto) {
      void save(rule, { mode: "suggest" });
      return;
    }
    confirming = rule;
  }

  function confirmAuto() {
    if (!confirming) return;
    void save(confirming, { mode: "auto" });
    confirming = null;
  }

  /** What this rule will do to the machine, in one sentence. */
  function consequence(rule: HealRule): string {
    switch (rule.action) {
      case "restart_container":
        return t("self_heal.consequence_restart", {
          default:
            "This rule will restart the container by itself, without asking you first.",
        });
      case "stop_container":
        return t("self_heal.consequence_stop", {
          default:
            "This rule will stop the container by itself. It will stay stopped until you start it.",
        });
      case "restart_vm":
        return t("self_heal.consequence_vm", {
          default:
            "This rule will restart Colima by itself. Every container on this machine stops while it does.",
        });
      default:
        return "";
    }
  }

  /**
   * The rule's name in the reader's language.
   *
   * Seeded names are stored in English, because the database is written once by
   * the backend and read by every locale. The trigger is the stable identity,
   * so the display name is looked up from it and falls back to the stored name
   * — which is also what the log shows, where the recorded wording is the
   * honest one.
   */
  function ruleName(rule: HealRule): string {
    return t(`self_heal.rule_${rule.trigger}`, { default: rule.name });
  }

  /** The threshold's unit, which differs per trigger. */
  function unit(rule: HealRule): string {
    switch (rule.trigger) {
      case "unhealthy":
        return t("self_heal.unit_minutes", { default: "minutes" });
      case "crash_loop":
        return t("self_heal.unit_deaths", { default: "restarts" });
      case "disk_full":
        return t("self_heal.unit_percent", { default: "% full" });
      default:
        return "";
    }
  }
</script>

<SettingsSection
  bind:el={sectionEl}
  title={t("self_heal.title", { default: "Self-healing" })}
  icon="activity"
  description={t("self_heal.description", {
    default:
      "Rules that repair a container for you. Every rule only suggests until you say otherwise.",
  })}
>
  <label class="master">
    <input
      class="checkbox"
      type="checkbox"
      checked={enabled}
      onchange={(e) => toggleMaster(e.currentTarget.checked)}
    />
    <span>
      <strong>{t("self_heal.master", { default: "Allow self-healing to act" })}</strong>
      <small>
        {t("self_heal.master_hint", {
          default:
            "Off stops every rule immediately, including anything waiting to run. Rules keep their settings.",
        })}
      </small>
    </span>
  </label>

  {#if !loaded}
    <p class="hint-text">{t("self_heal.loading", { default: "Loading…" })}</p>
  {:else}
    <ul class="rules">
        {#each rules as rule (rule.id)}
          <li class="rule" class:off={!rule.enabled}>
            <div class="rule-head">
              <label class="rule-on">
                <input
                  class="checkbox"
                  type="checkbox"
                  checked={rule.enabled}
                  onchange={(e) => save(rule, { enabled: e.currentTarget.checked })}
                />
                <span class="rule-name">{ruleName(rule)}</span>
              </label>

              {#if rule.autoCapable}
                <label class="mode">
                  <input
                    class="checkbox"
                    type="checkbox"
                    checked={rule.mode === "auto"}
                    disabled={!rule.enabled}
                    onchange={(e) => requestMode(rule, e.currentTarget.checked)}
                  />
                  {t("self_heal.mode_auto", { default: "Act automatically" })}
                </label>
              {:else}
                <!-- Not a disabled switch: there is nothing here to grant. -->
                <span class="advisory">
                  {t("self_heal.advisory", { default: "Always suggests only" })}
                </span>
              {/if}
            </div>

            <div class="rule-body">
              {#if unit(rule)}
                <label class="field">
                  {t("self_heal.threshold", { default: "Trigger at" })}
                  <input
                    class="input"
                    type="number"
                    min="1"
                    value={rule.threshold}
                    disabled={!rule.enabled}
                    onchange={(e) => save(rule, { threshold: Number(e.currentTarget.value) })}
                  />
                  <span class="unit">{unit(rule)}</span>
                </label>
              {/if}

              <label class="field">
                {t("self_heal.quota", { default: "At most" })}
                <input
                  class="input"
                  type="number"
                  min="1"
                  value={rule.maxPerHour}
                  disabled={!rule.enabled}
                  onchange={(e) => save(rule, { maxPerHour: Number(e.currentTarget.value) })}
                />
                <span class="unit">{t("self_heal.per_hour", { default: "times per hour" })}</span>
              </label>
            </div>
          </li>
        {/each}
      </ul>

    <h3 class="log-heading">{t("self_heal.log_heading", { default: "What it has done" })}</h3>
    <HealLog entries={log} />
  {/if}
</SettingsSection>

{#if confirming}
  <div class="overlay" role="presentation" onclick={() => (confirming = null)}>
    <div
      class="card dialog"
      role="alertdialog"
      aria-modal="true"
      aria-label={ruleName(confirming)}
      onclick={(e) => e.stopPropagation()}
    >
      <h2>{ruleName(confirming)}</h2>
      <p class="consequence">{consequence(confirming)}</p>
      <p class="limit">
        {t("self_heal.confirm_quota", {
          default: "It will do this at most {count} times an hour.",
          count: confirming.maxPerHour,
        })}
      </p>
      <div class="dialog-actions">
        <button class="btn btn-ghost" onclick={() => (confirming = null)}>
          {t("self_heal.cancel", { default: "Cancel" })}
        </button>
        <button class="btn btn-primary" onclick={confirmAuto}>
          {t("self_heal.confirm_auto", { default: "Let it act" })}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .master {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    margin-bottom: 12px;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    cursor: pointer;
  }
  .master span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .master small {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .rules {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .rule {
    padding: 10px 12px;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
  }
  .rule.off {
    opacity: 0.55;
  }
  .rule-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .rule-on,
  .mode {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: var(--text-sm);
  }
  .rule-name {
    font-weight: 500;
  }
  .advisory {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .rule-body {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .field :global(input) {
    width: 5.5rem;
  }
  .unit {
    color: var(--text-muted);
  }
  .log-heading {
    margin: 18px 0 8px;
    font-size: var(--text-base);
  }
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    padding: 24px;
  }
  .dialog {
    max-width: 440px;
    padding: 20px;
  }
  .dialog h2 {
    margin: 0 0 10px;
    font-size: var(--text-lg);
  }
  .consequence {
    margin: 0 0 8px;
    font-size: var(--text-sm);
  }
  .limit {
    margin: 0 0 16px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>

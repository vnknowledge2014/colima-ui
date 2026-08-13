<script lang="ts">
  /**
   * This image's failing rules, ordered by what fixing each one is worth.
   *
   * The distinction from the free "Next actions" list is the number. That list
   * ranks by a rule's raw weight across every image; this one re-scores the
   * image with each rule flipped to passing and reports the actual difference.
   * Weight and score delta are not the same figure — components are normalised
   * — so the paid list is genuinely more accurate rather than merely longer.
   *
   * No number on this screen comes from a model. The optional AI pass writes
   * the paragraph explaining the order, and nothing else.
   */
  import { securityApi, type SecurityAudit, type Level, type TriageResult, type Effort } from "../../lib/api";
  import { settingsState } from "../../lib/settingsStore.svelte";
  import { aiApi } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";

  let { audit, level }: { audit: SecurityAudit; level: Level } = $props();

  let result = $state<TriageResult | null>(null);
  let loading = $state(false);
  let askingAi = $state(false);
  let aiAnswer = $state("");
  let showPayload = $state(false);

  async function run() {
    loading = true;
    aiAnswer = "";
    showPayload = false;
    try {
      result = await securityApi.triage(audit, level);
    } catch (e) {
      globalToast("error", String(e));
      result = null;
    } finally {
      loading = false;
    }
  }

  // Re-triage whenever the audit or strictness changes: a list computed from a
  // previous scan shown beside a new score is worse than no list.
  $effect(() => {
    void audit;
    void level;
    void run();
  });

  async function askAi() {
    if (!result) return;
    const provider = settingsState["ai_provider"];
    const model = settingsState["ai_model"];
    if (!provider || !model) {
      globalToast("error", t('security.triage.no_ai', { default: 'Configure an AI provider in Settings first.' }));
      return;
    }
    askingAi = true;
    try {
      aiAnswer = await aiApi.chat(
        provider,
        model,
        settingsState["ai_api_key"] || "",
        [{ role: "user", content: result.llmPayloadPreview }],
        settingsState["ai_endpoint"] || "",
      );
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      askingAi = false;
    }
  }

  function effortLabel(effort: Effort): string {
    if (effort === "oneclick") return t('security.triage.effort_oneclick', { default: 'patch ready' });
    if (effort === "review") return t('security.triage.effort_review', { default: 'needs review' });
    return t('security.triage.effort_manual', { default: 'manual' });
  }
</script>

<div style="display: flex; flex-direction: column; gap: 10px;">
  {#if loading}
    <div style="font-size: var(--text-sm); color: var(--text-muted);">
      {t('security.triage.working', { default: 'Ranking the fixes…' })}
    </div>
  {:else if result && result.items.length === 0}
    <div style="font-size: var(--text-sm); color: var(--text-secondary);">
      {t('security.triage.clean', { default: 'Every rule passes at this strictness. Nothing to rank.' })}
    </div>
  {:else if result}
    <div style="font-size: var(--text-sm); color: var(--text-secondary);">
      {t('security.triage.headline', { default: 'Score' })}:
      <strong>{result.currentScore}</strong> →
      <strong style="color: var(--accent-green);">{result.potentialScore}</strong>
      <span style="color: var(--text-muted); font-size: var(--text-xs);">
        {t('security.triage.headline_note', { default: 'if every fix below is done' })}
      </span>
    </div>

    <ol style="display: flex; flex-direction: column; gap: 8px; margin: 0; padding-left: 18px;">
      {#each result.items as item (item.ruleId)}
        <li>
          <div style="display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap;">
            <span style="font-weight: 600; font-size: var(--text-sm);">{item.title}</span>
            <span style="font-size: var(--text-xs); color: var(--accent-green);">
              +{item.scoreDeltaEst} {t('security.triage.points', { default: 'points' })}
            </span>
            <span style="font-size: var(--text-xs); color: var(--text-muted);">
              {item.severity} · {effortLabel(item.effort)}
            </span>
          </div>
          <div style="font-size: var(--text-xs); color: var(--text-secondary); margin-top: 2px;">
            {item.whyFirst}
          </div>
          {#if item.evidence}
            <div style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono);">
              {item.evidence}
            </div>
          {/if}
        </li>
      {/each}
    </ol>

    <!-- The AI half. Opt-in, and the payload is shown before it goes anywhere —
         the same order of operations the compose diagnosis established. -->
    <div style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap;">
      <button class="btn btn-sm" onclick={askAi} disabled={askingAi}>
        {askingAi
          ? t('security.triage.asking', { default: 'Asking…' })
          : t('security.triage.ask_ai', { default: 'Explain this order' })}
      </button>
      <button class="btn btn-sm" onclick={() => (showPayload = !showPayload)}>
        {showPayload
          ? t('security.triage.hide_payload', { default: 'Hide what would be sent' })
          : t('security.triage.show_payload', { default: 'Show what would be sent' })}
      </button>
    </div>

    {#if showPayload}
      <pre style="font-size: var(--text-xs); font-family: var(--font-mono); white-space: pre-wrap; margin: 0; color: var(--text-muted);">{result.llmPayloadPreview}</pre>
    {/if}

    {#if aiAnswer}
      <div style="font-size: var(--text-sm); color: var(--text-secondary); white-space: pre-wrap;">{aiAnswer}</div>
    {/if}
  {/if}
</div>

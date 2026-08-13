<script lang="ts">
  import { composeApi, knowledgeBankApi, aiApi, type ComposeDiagnosis } from "../../lib/api";
  import { settingsState } from "../../lib/settingsStore.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import Icon from "../Icon.svelte";

  let { configFiles } = $props<{ configFiles: string }>();

  let running = $state(false);
  let result = $state<ComposeDiagnosis | null>(null);
  let error = $state("");

  // The AI step is opt-in and shown only after the user has seen the payload.
  let showPayload = $state(false);
  let askingAi = $state(false);
  let aiAnswer = $state("");
  let votedIds = $state<Record<number, "up" | "down">>({});

  // `docker compose ls` reports config files comma-separated; the first one is
  // the project's entry point.
  const composeFile = $derived((configFiles || "").split(",")[0]?.trim() || "");

  async function runDiagnosis() {
    if (!composeFile) {
      error = t('compose.diagnose.no_file', { default: 'No compose file recorded for this project.' });
      return;
    }
    running = true;
    error = "";
    aiAnswer = "";
    showPayload = false;
    try {
      result = await composeApi.diagnose(composeFile);
    } catch (e) {
      error = String(e);
      result = null;
    } finally {
      running = false;
    }
  }

  async function askAi() {
    if (!result) return;
    const provider = settingsState["ai_provider"];
    const model = settingsState["ai_model"];
    const apiKey = settingsState["ai_api_key"] || "";
    const endpoint = settingsState["ai_endpoint"] || "";

    if (!provider || !model) {
      globalToast("error", t('compose.diagnose.no_ai_config', { default: 'Configure an AI provider in Settings first.' }));
      return;
    }

    askingAi = true;
    try {
      aiAnswer = await aiApi.chat(provider, model, apiKey, [
        { role: "user", content: result.llm_payload_preview },
      ], endpoint);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      askingAi = false;
    }
  }

  async function vote(solutionId: number, isLike: boolean) {
    try {
      await knowledgeBankApi.feedback(solutionId, isLike);
      votedIds = { ...votedIds, [solutionId]: isLike ? "up" : "down" };
    } catch (e) {
      globalToast("error", String(e));
    }
  }
</script>

<div style="display: flex; flex-direction: column; gap: 16px;">

  <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px;">
    <div style="font-size: var(--text-sm); color: var(--text-secondary);">
      {#if composeFile}
        <span style="font-family: var(--font-mono);">{composeFile}</span>
      {:else}
        {t('compose.diagnose.no_file', { default: 'No compose file recorded for this project.' })}
      {/if}
    </div>
    <button class="btn btn-primary" disabled={running || !composeFile} onclick={runDiagnosis}>
      {running
        ? t('compose.diagnose.running', { default: 'Checking…' })
        : t('compose.diagnose.run', { default: 'Analyze' })}
    </button>
  </div>

  {#if error}
    <div class="card" style="border-color: var(--accent-red);">
      <div style="color: var(--accent-red); font-size: var(--text-sm);">{error}</div>
    </div>
  {/if}

  {#if result}
    {#if result.validation.valid}
      <div class="card" style="display: flex; align-items: center; gap: 8px;">
        <Icon name="Check" size={16} />
        <span style="font-size: var(--text-sm);">
          {t('compose.diagnose.valid', { default: 'docker compose config reports no problems with this file.' })}
        </span>
      </div>
    {:else}
      <!-- Docker's own message, verbatim: it is usually the most precise thing we have -->
      <div class="card">
        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 8px;">
          <span style="font-weight: 600;">{t('compose.diagnose.error_title', { default: 'Reported by Docker' })}</span>
          <span class="badge">{result.validation.category}</span>
        </div>
        <pre style="font-size: var(--text-xs); font-family: var(--font-mono); white-space: pre-wrap; margin: 0; color: var(--text-secondary);">{result.validation.raw_error}</pre>
      </div>


      {#if result.kb_solutions.length > 0}
        <div style="font-weight: 600; font-size: var(--text-sm);">
          {t('compose.diagnose.kb_title', { default: 'Known solutions' })}
        </div>
        {#each result.kb_solutions as sol (sol.id)}
          <div class="card">
            {#if sol.root_cause}
              <div style="font-size: var(--text-xs); color: var(--text-muted); margin-bottom: 6px;">{sol.root_cause}</div>
            {/if}
            <div style="font-size: var(--text-sm); white-space: pre-wrap;">{sol.solution_text}</div>
            {#if sol.commands}
              <pre style="font-size: var(--text-xs); font-family: var(--font-mono); white-space: pre-wrap; margin: 8px 0 0; color: var(--text-secondary);">{sol.commands}</pre>
            {/if}
            <div style="display: flex; gap: 8px; margin-top: 10px;">
              <button class="btn btn-ghost" style="font-size: var(--text-xs);"
                      disabled={!!votedIds[sol.id]} onclick={() => vote(sol.id, true)}>
                👍 {sol.likes}
              </button>
              <button class="btn btn-ghost" style="font-size: var(--text-xs);"
                      disabled={!!votedIds[sol.id]} onclick={() => vote(sol.id, false)}>
                👎 {sol.dislikes}
              </button>
            </div>
          </div>
        {/each}
      {/if}

      {#each result.kb_anti_patterns as ap (ap.id)}
        <div class="card" style="border-color: var(--accent-orange, var(--accent-red));">
          <div style="font-size: var(--text-xs); font-weight: 600; margin-bottom: 4px;">
            {t('compose.diagnose.anti_pattern', { default: 'Known bad advice — do not do this' })}
          </div>
          <div style="font-size: var(--text-sm);">{ap.bad_suggestion}</div>
          <div style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 4px;">{ap.reason}</div>
        </div>
      {/each}

      {#if result.needs_llm}
        <div class="card">
          <div style="font-size: var(--text-sm); margin-bottom: 10px;">
            {t('compose.diagnose.no_kb_match', { default: 'Nothing in the Knowledge Bank matches this error. You can ask your configured AI provider instead.' })}
          </div>
          <div style="font-size: var(--text-xs); color: var(--text-muted); margin-bottom: 10px;">
            {t('compose.diagnose.redaction_note', { default: 'Environment values, secrets and env_file contents are removed before sending. Review the exact text below first — nothing leaves your machine until you press Ask AI.' })}
          </div>
          <div style="display: flex; gap: 8px; flex-wrap: wrap;">
            <button class="btn btn-ghost" onclick={() => showPayload = !showPayload}>
              {showPayload
                ? t('compose.diagnose.hide_payload', { default: 'Hide what will be sent' })
                : t('compose.diagnose.show_payload', { default: 'Show what will be sent' })}
            </button>
            <button class="btn btn-primary" disabled={askingAi} onclick={askAi}>
              {askingAi
                ? t('compose.diagnose.asking', { default: 'Asking…' })
                : t('compose.diagnose.ask_ai', { default: 'Ask AI' })}
            </button>
          </div>
          {#if showPayload}
            <pre style="font-size: var(--text-xs); font-family: var(--font-mono); white-space: pre-wrap; margin: 12px 0 0; padding: 12px; background: var(--bg-secondary); border-radius: var(--radius-md); max-height: 300px; overflow: auto;">{result.llm_payload_preview}</pre>
          {/if}
        </div>
      {/if}

      {#if aiAnswer}
        <div class="card">
          <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 8px;">
            <span style="font-weight: 600; font-size: var(--text-sm);">
              {t('compose.diagnose.ai_title', { default: 'AI suggestion' })}
            </span>
            <span class="badge" style="background: rgba(188, 140, 255, 0.1); color: var(--accent-purple);">
              {t('compose.diagnose.ai_badge', { default: 'UNVERIFIED' })}
            </span>
          </div>
          <div style="font-size: var(--text-sm); white-space: pre-wrap;">{aiAnswer}</div>
        </div>
      {/if}
    {/if}
  {/if}
</div>

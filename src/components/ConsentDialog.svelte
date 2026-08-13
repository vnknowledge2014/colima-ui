<script lang="ts">
  import { onMount } from "svelte";
  import { telemetryApi } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  // Self-managing: on mount, ask the backend whether consent was ever decided.
  // Shown only when it was never asked; a prior decline is final.
  let open = $state(false);

  onMount(async () => {
    try {
      open = await telemetryApi.shouldPrompt();
    } catch {
      // Browser mode without a server, or any failure → do not nag.
      open = false;
    }
  });

  async function decide(granted: boolean) {
    try {
      await telemetryApi.setConsent(granted);
    } catch { /* best effort; closing regardless */ }
    open = false;
  }
</script>

{#if open}
  <div style="position: fixed; inset: 0; z-index: 10001; background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;" role="presentation">
    <div style="background: var(--bg-primary); border-radius: var(--radius-lg); border: 1px solid var(--border-primary); box-shadow: 0 20px 60px rgba(0,0,0,0.5); padding: 24px; min-width: 400px; max-width: 500px;">
      <div style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 8px;">
        {t('telemetry.consent.title', { default: 'Help improve ColimaUI?' })}
      </div>
      <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.6; margin: 0 0 12px;">
        {t('telemetry.consent.body', { default: 'Share anonymous usage events so we know which features to improve. Never your container names, paths, environment variables, chat contents, or keys.' })}
      </p>
      <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6; margin: 0 0 20px;">
        {t('telemetry.consent.default_off', { default: "It's off unless you turn it on, and you can change this anytime in Settings." })}
      </p>
      <div style="display: flex; justify-content: flex-end; gap: 12px;">
        <button class="btn btn-ghost" onclick={() => decide(false)}>
          {t('telemetry.consent.decline', { default: 'No thanks' })}
        </button>
        <button class="btn btn-primary" onclick={() => decide(true)}>
          {t('telemetry.consent.accept', { default: 'Share anonymous data' })}
        </button>
      </div>
    </div>
  </div>
{/if}

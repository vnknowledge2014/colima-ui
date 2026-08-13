<script lang="ts">
  import { onMount } from "svelte";
  import { telemetryApi, type Consent, type TelemetryEvent } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import SettingsSection from "./SettingsSection.svelte";

  let consent = $state<Consent>("unset");
  let showData = $state(false);
  let preview = $state<TelemetryEvent[]>([]);

  const enabled = $derived(consent === "granted");

  onMount(async () => {
    try {
      consent = await telemetryApi.consent();
    } catch { /* browser mode / no server — leave as unset */ }
  });

  async function toggle() {
    const next = !enabled;
    try {
      await telemetryApi.setConsent(next);
      consent = next ? "granted" : "declined";
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function loadPreview() {
    showData = !showData;
    if (showData) {
      try {
        preview = await telemetryApi.preview();
      } catch {
        preview = [];
      }
    }
  }
</script>

<SettingsSection
  title={t('telemetry.settings.title', { default: 'Privacy & Telemetry' })}
  description={t('telemetry.settings.desc', { default: 'Anonymous usage events, off by default. Never container names, paths, environment variables, chat contents, or keys.' })}
>

  <div style="display: flex; justify-content: space-between; align-items: center; padding: 12px 0; border-top: 1px solid var(--border-subtle);">
    <div>
      <div style="font-weight: 500;">{t('telemetry.settings.toggle', { default: 'Share anonymous usage data' })}</div>
      <div style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 2px;">
        {enabled
          ? t('telemetry.settings.on', { default: 'On — nothing is transmitted yet; there is no endpoint.' })
          : t('telemetry.settings.off', { default: 'Off' })}
      </div>
    </div>
    <button class="btn {enabled ? 'btn-primary' : 'btn-ghost'}" onclick={toggle}>
      {enabled ? t('common.on', { default: 'On' }) : t('common.off', { default: 'Off' })}
    </button>
  </div>

  <div style="padding-top: 12px;">
    <button class="btn btn-ghost" style="font-size: var(--text-sm);" onclick={loadPreview}>
      {showData
        ? t('telemetry.settings.hide_data', { default: 'Hide data' })
        : t('telemetry.settings.show_data', { default: 'Show what would be sent' })}
    </button>
    {#if showData}
      {#if preview.length > 0}
        <pre style="font-size: var(--text-xs); font-family: var(--font-mono); white-space: pre-wrap; margin: 12px 0 0; padding: 12px; background: var(--bg-secondary); border-radius: var(--radius-md); max-height: 240px; overflow: auto;">{JSON.stringify(preview, null, 2)}</pre>
      {:else}
        <div style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 12px;">
          {t('telemetry.settings.no_data', { default: 'No events recorded.' })}
        </div>
      {/if}
    {/if}
  </div>
</SettingsSection>

<script lang="ts">
  import { isRunningInTauri } from "../../lib/env";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import SettingsSection from "./SettingsSection.svelte";

  const inApp = isRunningInTauri();

  let checking = $state(false);
  let installing = $state(false);
  // null = not checked yet; false = up to date; object = update available
  let available = $state<{ version: string; notes: string } | null | false>(null);
  // The updater Update handle, kept so we can install what we just found.
  let pending: { version: string; body?: string; downloadAndInstall: () => Promise<void> } | null = null;

  async function check() {
    checking = true;
    available = null;
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        pending = update;
        available = { version: update.version, notes: update.body ?? "" };
      } else {
        available = false;
      }
    } catch (e) {
      // No manifest hosted yet, offline, or signature mismatch — report, don't crash.
      available = false;
      globalToast("error", t('updates.check_failed', { default: 'Could not check for updates.' }) + " " + String(e));
    } finally {
      checking = false;
    }
  }

  async function install() {
    if (!pending) return;
    installing = true;
    try {
      await pending.downloadAndInstall();
      globalToast("success", t('updates.installed', { default: 'Update installed. Restart ColimaUI to finish.' }));
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      installing = false;
    }
  }
</script>

<SettingsSection
  title={t('updates.title', { default: 'Updates' })}
  description={inApp ? t('updates.desc', { default: 'Check for a newer signed build. Updates are verified before they are applied.' }) : t('updates.browser_only', { default: 'Automatic updates are available in the desktop app.' })}
>

  {#if inApp}
    <button class="btn btn-ghost" disabled={checking || installing} onclick={check}>
      {checking ? t('updates.checking', { default: 'Checking…' }) : t('updates.check', { default: 'Check for updates' })}
    </button>

    {#if available === false}
      <div style="font-size: var(--text-sm); color: var(--text-muted); margin-top: 12px;">
        {t('updates.up_to_date', { default: 'You are on the latest version.' })}
      </div>
    {:else if available}
      <div class="card" style="margin-top: 12px;">
        <div style="font-weight: 600; font-size: var(--text-sm);">
          {t('updates.new_version', { default: 'New version' })}: {available.version}
        </div>
        {#if available.notes}
          <pre style="font-size: var(--text-xs); white-space: pre-wrap; color: var(--text-secondary); margin: 8px 0 0;">{available.notes}</pre>
        {/if}
        <button class="btn btn-primary" style="margin-top: 12px;" disabled={installing} onclick={install}>
          {installing ? t('updates.installing', { default: 'Installing…' }) : t('updates.install', { default: 'Download & install' })}
        </button>
      </div>
    {/if}
  {/if}
</SettingsSection>

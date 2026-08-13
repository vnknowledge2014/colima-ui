<script lang="ts">
  import { type SystemInfo } from "../lib/api";
  import { setLanguage, getLanguage, t } from "../lib/i18n.svelte";
  import AIPanelSettings from "../components/settings/AIPanelSettings.svelte";
  import SettingsSection from "../components/settings/SettingsSection.svelte";
  import UpdateSettings from "../components/settings/UpdateSettings.svelte";
  import ResourceSaverSettings from "../components/settings/ResourceSaverSettings.svelte";
  import TraySettings from "../components/settings/TraySettings.svelte";
  import NotificationSettings from "../components/settings/NotificationSettings.svelte";
  import SelfHealing from "./settings/SelfHealing.svelte";
  import ColimaConfig from "./settings/ColimaConfig.svelte";

  let { systemInfo } = $props<{ systemInfo: SystemInfo | null }>();

  // Fix: use version string presence as fallback for installed status in case backend boolean is unreliable
  const deps = $derived([
    { name: "Colima", desc: "Container runtime manager", installed: systemInfo?.colima_installed === true || !!(systemInfo?.colima_version), version: systemInfo?.colima_version },
    { name: "Docker", desc: "Container engine client", installed: systemInfo?.docker_installed === true || !!(systemInfo?.docker_version), version: systemInfo?.docker_version },
    { name: "Lima", desc: "Linux virtual machine manager", installed: systemInfo?.lima_installed === true || !!(systemInfo?.lima_version), version: systemInfo?.lima_version },
  ]);
</script>

<div class="content-header" data-tauri-drag-region>
  <div>
    <h1>{t('settings.title', { default: 'Settings' })}</h1>
    <div class="content-header-subtitle">{t('settings.subtitle', { default: 'Configure ColimaUI, AI behavior, and resources' })}</div>
  </div>
</div>

<div class="content-body">
  <div style="max-width: 800px; padding-bottom: 60px;">
  
    <!-- Appearance Settings -->
    <SettingsSection title={t('settings.appearance', { default: 'Appearance' })} icon="Settings">
      <div style="display: flex; flex-direction: column; gap: 16px;">
        <div style="display: flex; justify-content: space-between; align-items: center;">
          <div>
            <div style="font-weight: 500;">{t('settings.language', { default: 'Language' })}</div>
            <div style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 2px;">{t('settings.language_desc', { default: 'Change the application language' })}</div>
          </div>
          <select class="input select" style="width: 200px;" value={getLanguage()} onchange={(e) => {
            setLanguage(e.currentTarget.value);
          }}>
            <option value="en">English</option>
            <option value="vi">Tiếng Việt</option>
            <option value="zh">中文</option>
            <option value="ja">日本語</option>
          </select>
        </div>
      </div>
    </SettingsSection>

  <!-- System Dependencies -->
  <SettingsSection title="System Dependencies">
    <div style="display: flex; flex-direction: column; gap: 0;">
      {#each deps as dep, i (dep.name)}
        <div style="display: flex; justify-content: space-between; align-items: center; padding: 12px 0; border-bottom: {i < deps.length - 1 ? '1px solid var(--border-subtle)' : 'none'};">
          <div>
            <div style="font-weight: 500;">{dep.name}</div>
            <div style="font-size: var(--text-xs); color: var(--text-muted);">{dep.desc}</div>
          </div>
          <div style="text-align: right;">
            <span class="badge {dep.installed ? 'badge-running' : 'badge-stopped'}">
              {dep.installed ? "INSTALLED" : "NOT FOUND"}
            </span>
            {#if dep.version}
              <div style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono); margin-top: 4px;">
                {dep.version.split("\n")[0]}
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </SettingsSection>

  <ColimaConfig />
  <TraySettings />

  <NotificationSettings />
  <SelfHealing />
  <ResourceSaverSettings />
  <AIPanelSettings />
  <UpdateSettings />

  <!-- About -->
  <SettingsSection title="About ColimaUI">
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.7; margin: 0;">
      ColimaUI is a cross-platform graphical interface for managing Colima instances,
      Docker containers, Kubernetes clusters, and Linux VMs. Built with Tauri v2 and Svelte 5.
    </p>
    <div style="margin-top: 16px; display: flex; gap: 12px; flex-wrap: wrap;">
      <!-- Each tint is mixed from the colour it sits next to, so text and
           background can never come from two different palettes. Svelte's
           orange is a brand colour and stays a literal. -->
      <span class="badge" style="background: color-mix(in srgb, var(--accent-blue) 12%, transparent); color: var(--accent-blue);">v0.1.0</span>
      <span class="badge" style="background: color-mix(in srgb, var(--accent-purple) 12%, transparent); color: var(--accent-purple);">Tauri v2</span>
      <span class="badge" style="background: color-mix(in srgb, #ff3e00 12%, transparent); color: #ff3e00;">Svelte 5</span>
      <span class="badge" style="background: color-mix(in srgb, var(--accent-green) 12%, transparent); color: var(--accent-green);">Rust</span>
    </div>
  </SettingsSection>

  <!-- Third-party notices. EPL-2.0 obliges us to name the component, state the
       licence, and point at the source we redistribute. -->
  <SettingsSection title="Open Source Licenses">
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.7; margin: 0;">
      ColimaUI bundles third-party components. Their licenses apply to those components only.
    </p>
    <ul style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.8; margin: 12px 0 0; padding-left: 18px;">
      <li>
        <strong>ELK (elkjs)</strong> — graph layout for the Topology view.
        Eclipse Public License 2.0 —
        <a href="https://github.com/kieler/elkjs" target="_blank" rel="noreferrer noopener">github.com/kieler/elkjs</a>
      </li>
      <li>
        <strong>xterm.js</strong> — terminal emulator. MIT License —
        <a href="https://github.com/xtermjs/xterm.js" target="_blank" rel="noreferrer noopener">github.com/xtermjs/xterm.js</a>
      </li>
    </ul>
  </SettingsSection>
  </div>
</div>

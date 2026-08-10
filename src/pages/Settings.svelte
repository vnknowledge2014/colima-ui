<script lang="ts">
  import { onMount } from "svelte";
  import { type SystemInfo, dockerApi, aiApi, knowledgeBankApi, sysMethods } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { confirm } from "../store/confirm.svelte";
  import Icon from "../components/Icon.svelte";
  import { setLanguage, getLanguage, t } from "../lib/i18n.svelte";
  import AIPanelSettings from "../components/settings/AIPanelSettings.svelte";
  import ResourceSaverSettings from "../components/settings/ResourceSaverSettings.svelte";

  let { systemInfo } = $props<{ systemInfo: SystemInfo | null }>();

  // Dependency state
  let diskUsage = $state<DiskUsage[]>([]);
  let pruning = $state(false);

  onMount(() => {
    fetchDiskUsage();
  });

  async function fetchDiskUsage() {
    try {
      const raw = await dockerApi.systemDf();
      if (!raw) return;
      const text = typeof raw === 'string' ? raw : String(raw);
      const lines = text.split("\n").filter(l => l.trim());
      const rows: any[] = [];
      for (const line of lines) {
        if (line.startsWith("TYPE") || line.startsWith("---")) continue;
        const parts = line.split(/\s{2,}/);
        if (parts.length >= 4) {
          rows.push({
            type: parts[0],
            total: parts[1],
            active: parts[2],
            size: parts[3],
            reclaimable: parts[4] || "0B",
          });
        }
      }
      diskUsage = rows;
    } catch { /* ignore */ }
  }

  async function handlePrune() {
    const ok = await confirm({ title: "System Prune", message: "Remove all unused Docker data (stopped containers, unused networks, dangling images, build cache)?", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    pruning = true;
    try {
      await dockerApi.systemPrune();
      globalToast("success", "System pruned successfully");
      fetchDiskUsage();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      pruning = false;
    }
  }

  // Fix: use version string presence as fallback for installed status in case backend boolean is unreliable
  const deps = $derived([
    { name: "Colima", desc: "Container runtime manager", installed: systemInfo?.colima_installed === true || !!(systemInfo?.colima_version), version: systemInfo?.colima_version },
    { name: "Docker", desc: "Container engine client", installed: systemInfo?.docker_installed === true || !!(systemInfo?.docker_version), version: systemInfo?.docker_version },
    { name: "Lima", desc: "Linux virtual machine manager", installed: systemInfo?.lima_installed === true || !!(systemInfo?.lima_version), version: systemInfo?.lima_version },
  ]);
</script>

<div class="content-header">
  <div>
    <h1>{t('settings.title', { default: 'Settings' })}</h1>
    <div class="content-header-subtitle">{t('settings.subtitle', { default: 'Configure ColimaUI, AI behavior, and resources' })}</div>
  </div>
</div>

<div class="content-body">
  <div style="max-width: 800px; padding-bottom: 60px;">
  
    <!-- Appearance Settings -->
    <div class="card" style="margin-bottom: 24px; padding: 0;">
      <div style="padding: 16px 20px; border-bottom: 1px solid var(--border-primary); font-weight: 600; font-size: var(--text-lg); display: flex; align-items: center; gap: 8px;">
        <Icon name="Settings" size={18} />
        {t('settings.appearance', { default: 'Appearance' })}
      </div>
      <div style="padding: 24px 20px;">
        <div style="display: flex; flex-direction: column; gap: 16px;">
          <div style="border-top: 1px solid var(--border-subtle); padding-top: 16px; display: flex; justify-content: space-between; align-items: center;">
            <div>
              <div style="font-weight: 500;">{t('settings.language', { default: 'Language' })}</div>
              <div style="font-size: var(--text-sm); color: var(--text-secondary); margin-top: 2px;">{t('settings.language_desc', { default: 'Change the application language' })}</div>
            </div>
            <!-- Fix: .select adds proper arrow icon + appearance:none styling -->
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
      </div>
    </div>

  <!-- System Dependencies -->
  <div class="card" style="margin-bottom: 24px;">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 20px;">System Dependencies</h3>
    <div style="display: flex; flex-direction: column; gap: 0;">
      {#each deps as dep, i}
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
  </div>

  <ResourceSaverSettings />
  <AIPanelSettings />

  <!-- About -->
  <div class="card">
    <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 16px;">About ColimaUI</h3>
    <p style="font-size: var(--text-sm); color: var(--text-secondary); line-height: 1.7; margin: 0;">
      ColimaUI is a cross-platform graphical interface for managing Colima instances,
      Docker containers, Kubernetes clusters, and Linux VMs. Built with Tauri v2 and Svelte 5.
    </p>
    <div style="margin-top: 16px; display: flex; gap: 12px; flex-wrap: wrap;">
      <span class="badge" style="background: rgba(88, 166, 255, 0.1); color: var(--accent-blue);">v0.1.0</span>
      <span class="badge" style="background: rgba(188, 140, 255, 0.1); color: var(--accent-purple);">Tauri v2</span>
      <span class="badge" style="background: rgba(255, 62, 0, 0.1); color: #ff3e00;">Svelte 5</span>
      <span class="badge" style="background: rgba(63,185,80,0.1); color: var(--accent-green);">Rust</span>
    </div>
    </div>
  </div>
</div>

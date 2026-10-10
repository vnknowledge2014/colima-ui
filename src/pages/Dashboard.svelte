<script lang="ts">
   
  import { composeApi, k8sApi, limaApi, kindApi, systemApi, type SystemInfo } from "../lib/api";
  import { dashboardState, dockerState, resourceState } from "../store.svelte";
  import { t } from "../lib/i18n.svelte";

  let { systemInfo, loading, onNavigate } = $props<{
    systemInfo: SystemInfo | null;
    loading: boolean;
    onNavigate: (page: string) => void;
  }>();

  const STALE_MS = 30_000;

  let runningCount = $derived(dashboardState.colimaInstances.filter(i => i.status === "Running").length);
  let stoppedCount = $derived(dashboardState.colimaInstances.filter(i => i.status !== "Running").length);
  let runningInstances = $derived(dashboardState.colimaInstances.filter(i => i.status === "Running"));
  let engine = $derived(dashboardState.engineResources);

  /**
   * Resource figures come from the container engine when it is reachable, and
   * only fall back to the Colima VM allocation when it is not. The VM numbers
   * are read from colima.yaml, which does not exist for Docker Desktop,
   * OrbStack, or Rancher — showing them unconditionally reported 0 CPU / 0 RAM
   * on those engines even while containers were running.
   */
  let resources = $derived.by(() => {
    if (engine?.available) {
      return {
        source: engine.engineName || "engine",
        cpuCores: engine.cpuCores,
        cpuPercent: engine.cpuPercent,
        memoryTotal: engine.memoryTotalBytes,
        memoryUsed: engine.memoryUsedBytes,
        diskUsed: engine.diskUsedBytes,
        diskReclaimable: engine.diskReclaimableBytes,
        live: true,
      };
    }
    return {
      source: "vm",
      cpuCores: runningInstances.reduce((sum, i) => sum + i.cpus, 0),
      cpuPercent: 0,
      memoryTotal: runningInstances.reduce((sum, i) => sum + i.memory, 0),
      memoryUsed: 0,
      diskUsed: runningInstances.reduce((sum, i) => sum + i.disk, 0),
      diskReclaimable: 0,
      live: false,
    };
  });

  let dockerCounts = $derived({
    containers: dockerState.containers.length,
    running: dockerState.containers.filter(c => c.State === "running").length,
    images: dockerState.images.length,
    volumes: resourceState.volumes.length,
    networks: resourceState.networks.length,
    composeProjects: dashboardState.composeProjectsCount
  });

  function formatBytes(bytes: number) {
    if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
    if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
    return `${bytes} B`;
  }

  /** Like formatBytes but keeps one decimal, so 7.8 GiB does not read as "8 GiB". */
  function formatSize(bytes: number) {
    if (!bytes) return "0 B";
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GiB`;
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MiB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
    return `${bytes} B`;
  }

  function percentOf(used: number, total: number) {
    if (!total) return 0;
    return Math.min(100, Math.round((used / total) * 100));
  }

  async function fetchEngineResources() {
    try {
      dashboardState.engineResources = await systemApi.engineResources();
    } catch {
      dashboardState.engineResources = null;
    }
  }

  async function fetchDockerCounts() {
    try {
      const compose = await composeApi.list();
      dashboardState.composeProjectsCount = compose ? compose.length : 0;
    } catch {
      // A transient backend hiccup should not break the whole dashboard.
    }
  }

  async function fetchK8sStatus() {
    try {
      const [checkResult, kindRaw] = await Promise.allSettled([k8sApi.check(), kindApi.list()]);
      const connected = checkResult.status === "fulfilled";
      let pods = 0, namespaces = 0;
      if (connected) {
        const [nsRaw, podsRaw] = await Promise.allSettled([k8sApi.namespaces(), k8sApi.pods("")]);
        if (nsRaw.status === "fulfilled") {
          const ns = nsRaw.value;
          namespaces = Array.isArray(ns) ? ns.length : (typeof ns === "string" ? (ns.match(/"name"/gi) || []).length : 0);
        }
        if (podsRaw.status === "fulfilled") {
          const p = podsRaw.value;
          pods = Array.isArray(p) ? p.length : (typeof p === "string" ? (p.match(/"name"/gi) || []).length : 0);
        }
      }
      const kindClusters = kindRaw.status === "fulfilled"
        ? kindRaw.value.trim().split("\n").filter(Boolean).filter(c => c !== "No kind clusters found.").length
        : 0;
      dashboardState.k8sStatus = { connected, pods, namespaces, kindClusters };
    } catch {
      dashboardState.k8sStatus = { connected: false, pods: 0, namespaces: 0, kindClusters: 0 };
    }
  }

  async function fetchLinuxVMs() {
    try {
      dashboardState.linuxVMs = await limaApi.list();
    } catch {
      dashboardState.linuxVMs = [];
    }
  }

  $effect(() => {
    if (!loading) {
      const now = Date.now();
      if (now - dashboardState.lastFetch > STALE_MS) {
        fetchDockerCounts();
        fetchK8sStatus();
        fetchLinuxVMs();
        fetchEngineResources();
        dashboardState.lastFetch = now;
      }
    }
  });
</script>



<div class="content-header" data-tauri-drag-region>
  <div class="header-left">
    <h1>{t('dashboard.title', { default: 'Dashboard' })}</h1>
  </div>
</div>

<div class="content-body" style="padding: 24px;">
  {#if loading}
    <!-- Same grid and tile height as the real content below, so nothing shifts
         position when the numbers land. A centred spinner in a fixed 200px box
         made the whole page jump once the sections replaced it. -->
    {#each [0, 1] as section (section)}
      <div class="dash-section" aria-busy="true" aria-label={t('dashboard.loading', { default: 'Loading...' })}>
        <div class="dash-grid-3">
          {#each [0, 1, 2] as tile (tile)}
            <div class="skeleton skeleton-tile"></div>
          {/each}
        </div>
      </div>
    {/each}
  {:else}
    <!-- SECTION 1: VM Instances -->
    <div class="dash-section">
      <div class="dash-section-header">
        <div class="dash-section-icon dash-icon-blue">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
            <line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/>
          </svg>
        </div>
        <span class="dash-section-title">{t('dashboard.vm_instances', { default: 'VM Instances' })}</span>
        <span class="badge" style="background: rgba(88,166,255,0.15); color: var(--accent-blue); border: 1px solid rgba(88,166,255,0.25); padding: 1px 8px; font-size: 11px;">{dashboardState.colimaInstances.length}</span>
      </div>
      <div class="dash-grid-3">
        <button class="metric-card metric-card-green" onclick={() => onNavigate("instances")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.running_vms', { default: 'Running VMs' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{runningCount}</span>
            <span class="metric-sub">Active instances</span>
          </div>
        </button>

        <button class="metric-card metric-card-red" onclick={() => onNavigate("instances")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.stopped_vms', { default: 'Stopped VMs' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/><rect x="9" y="9" width="6" height="6"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{stoppedCount}</span>
            <span class="metric-sub">Idle instances</span>
          </div>
        </button>

        <div class="metric-card metric-card-orange">
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.total_cpus', { default: 'Total CPUs' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/>
                <line x1="9" y1="1" x2="9" y2="4"/><line x1="15" y1="1" x2="15" y2="4"/>
                <line x1="9" y1="20" x2="9" y2="23"/><line x1="15" y1="20" x2="15" y2="23"/>
                <line x1="20" y1="9" x2="23" y2="9"/><line x1="20" y1="14" x2="23" y2="14"/>
                <line x1="1" y1="9" x2="4" y2="9"/><line x1="1" y1="14" x2="4" y2="14"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{resources.cpuCores}</span>
            <span class="metric-sub">
              {resources.live
                ? `${resources.cpuPercent.toFixed(1)}% in use`
                : t('dashboard.allocated_cores', { default: 'Allocated cores' })}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- SECTION 1b: Engine resources (CPU / RAM / Disk) -->
    <div class="dash-section">
      <div class="dash-section-header">
        <div class="dash-section-icon dash-icon-blue">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 3v18h18"/><path d="M7 15l4-6 4 3 4-7"/>
          </svg>
        </div>
        <span class="dash-section-title">{t('dashboard.resources', { default: 'Resources' })}</span>
        {#if resources.live}
          <span class="live-badge">
            <span class="badge-dot"></span>
            {resources.source} · live
          </span>
        {:else}
          <span class="metric-sub" style="margin-left: 8px;">
            {t('dashboard.resources_vm_fallback', { default: 'VM allocation (engine unreachable)' })}
          </span>
        {/if}
      </div>
      <div class="dash-grid-3">
        <div class="metric-card metric-card-orange">
          <div class="metric-header">
            <span class="metric-title">CPU</span>
          </div>
          <div class="metric-body">
            <span class="metric-value">{resources.live ? `${resources.cpuPercent.toFixed(1)}%` : resources.cpuCores}</span>
            <span class="metric-sub">
              {resources.live
                ? `${resources.cpuCores} ${t('dashboard.cores_available', { default: 'cores available' })}`
                : t('dashboard.allocated_cores', { default: 'Allocated cores' })}
            </span>
          </div>
        </div>

        <div class="metric-card metric-card-purple">
          <div class="metric-header">
            <span class="metric-title">RAM</span>
          </div>
          <div class="metric-body">
            <span class="metric-value">{formatSize(resources.live ? resources.memoryUsed : resources.memoryTotal)}</span>
            <span class="metric-sub">
              {resources.live
                ? `${t('dashboard.of', { default: 'of' })} ${formatSize(resources.memoryTotal)} (${percentOf(resources.memoryUsed, resources.memoryTotal)}%)`
                : t('dashboard.allocated_memory', { default: 'Allocated memory' })}
            </span>
          </div>
        </div>

        <div class="metric-card metric-card-blue">
          <div class="metric-header">
            <span class="metric-title">Disk</span>
          </div>
          <div class="metric-body">
            <span class="metric-value">{formatSize(resources.diskUsed)}</span>
            <span class="metric-sub">
              {resources.live
                ? `${formatSize(resources.diskReclaimable)} ${t('dashboard.reclaimable', { default: 'reclaimable' })}`
                : t('dashboard.allocated_disk', { default: 'Allocated disk' })}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- SECTION 2: Docker Engine -->
    <div class="dash-section">
      <div class="dash-section-header">
        <div class="dash-section-icon" style="background: rgba(36, 150, 237, 0.15); border-color: rgba(36, 150, 237, 0.25);">
          <svg viewBox="0 0 24 24" fill="none" stroke="#2496ed" stroke-width="2">
            <path d="M22 12.8c-.8 0-1.6-.3-2.3-.7-2 1.4-4.5 1.7-6.8 1-2.3.7-4.8.4-6.8-1-.7.4-1.5.7-2.3.7-1.1 0-2.2-.4-3-1.1v4c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2v-4c-.8.7-1.9 1.1-3 1.1z"/>
            <path d="M12 9c1.7 0 3-1.3 3-3s-1.3-3-3-3-3 1.3-3 3 1.3 3 3 3z"/>
          </svg>
        </div>
        <span class="dash-section-title">Docker Engine</span>
      </div>
      <div class="dash-grid-5">
        <button class="metric-card metric-card-blue" onclick={() => onNavigate("containers")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.containers', { default: 'Containers' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dockerCounts.containers}</span>
            <span class="metric-sub" style="color: var(--accent-green);">{dockerCounts.running} {t('dashboard.running', { default: 'running' })}</span>
          </div>
        </button>

        <button class="metric-card metric-card-purple" onclick={() => onNavigate("images")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.images', { default: 'Images' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dockerCounts.images}</span>
            <span class="metric-sub">Pulled images</span>
          </div>
        </button>

        <button class="metric-card metric-card-orange" onclick={() => onNavigate("volumes")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.volumes', { default: 'Volumes' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
                <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dockerCounts.volumes}</span>
            <span class="metric-sub">Persistent storage</span>
          </div>
        </button>

        <button class="metric-card metric-card-blue" onclick={() => onNavigate("networks")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.networks', { default: 'Networks' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="16" y="16" width="6" height="6" rx="1"/><rect x="2" y="16" width="6" height="6" rx="1"/>
                <rect x="9" y="2" width="6" height="6" rx="1"/>
                <path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"/><path d="M12 12V8"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dockerCounts.networks}</span>
            <span class="metric-sub">Virtual networks</span>
          </div>
        </button>

        <button class="metric-card metric-card-green" onclick={() => onNavigate("compose")}>
          <div class="metric-header">
            <span class="metric-title">{t('dashboard.compose_projects', { default: 'Compose' })}</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="12 2 2 7 12 12 22 7 12 2"/>
                <polyline points="2 17 12 22 22 17"/>
                <polyline points="2 12 12 17 22 12"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dockerCounts.composeProjects}</span>
            <span class="metric-sub">Active projects</span>
          </div>
        </button>
      </div>
    </div>

    <!-- SECTION 3: Kubernetes & Infrastructure -->
    <div class="dash-section">
      <div class="dash-section-header">
        <div class="dash-section-icon" style="background: rgba(188, 140, 255, 0.15); border-color: rgba(188, 140, 255, 0.25);">
          <svg viewBox="0 0 24 24" fill="none" stroke="var(--accent-purple)" stroke-width="2">
            <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
          </svg>
        </div>
        <span class="dash-section-title">Infrastructure</span>
      </div>
      <div class="dash-grid-3">
        <button onclick={() => onNavigate("kubernetes")} class="metric-card {dashboardState.k8sStatus.connected ? 'metric-card-green' : 'metric-card-red'}">
          <div class="metric-header">
            <span class="metric-title">Kubernetes</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dashboardState.k8sStatus.connected ? dashboardState.k8sStatus.pods : 0}</span>
            <span class="metric-sub">{dashboardState.k8sStatus.connected ? `${dashboardState.k8sStatus.namespaces} namespaces` : "Disconnected"}</span>
          </div>
        </button>

        <button onclick={() => onNavigate("instances")} class="metric-card metric-card-purple">
          <div class="metric-header">
            <span class="metric-title">Kind Clusters</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
                <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
                <line x1="12" y1="22.08" x2="12" y2="12"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dashboardState.k8sStatus.kindClusters}</span>
            <span class="metric-sub">Local clusters</span>
          </div>
        </button>

        <button onclick={() => onNavigate("linux-vms")} class="metric-card metric-card-orange">
          <div class="metric-header">
            <span class="metric-title">Linux VMs</span>
            <div class="metric-icon-wrapper">
              <svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                <line x1="8" y1="21" x2="16" y2="21"/>
                <line x1="12" y1="17" x2="12" y2="21"/>
              </svg>
            </div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dashboardState.linuxVMs.length}</span>
            <span class="metric-sub">{dashboardState.linuxVMs.filter(v => v.status === "Running").length} running</span>
          </div>
        </button>
      </div>
    </div>

    <!-- System Status -->
    {#if systemInfo}
      <div class="card" style="margin-bottom: 24px;">
        <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 16px;">System Status</h3>
        <div style="display: flex; flex-wrap: wrap; gap: 16px; align-items: center;">
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="badge {systemInfo.colima_installed ? 'badge-running' : 'badge-stopped'}">
              <span class="badge-dot" style="animation: none;"></span>
              <span>{systemInfo.colima_installed ? 'Installed' : 'Not Found'}</span>
            </span>
            <span style="font-size: var(--text-sm); color: var(--text-secondary);">Colima{systemInfo.colima_version ? ` v${systemInfo.colima_version.split('\n')[0].replace(/.*version\s*/i, "")}` : ''}</span>
          </div>
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="badge {systemInfo.docker_installed ? 'badge-running' : 'badge-stopped'}">
              <span class="badge-dot" style="animation: none;"></span>
              <span>{systemInfo.docker_installed ? 'Installed' : 'Not Found'}</span>
            </span>
            <span style="font-size: var(--text-sm); color: var(--text-secondary);">Docker</span>
          </div>
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="badge {systemInfo.lima_installed ? 'badge-running' : 'badge-stopped'}">
              <span class="badge-dot" style="animation: none;"></span>
              <span>{systemInfo.lima_installed ? 'Installed' : 'Not Found'}</span>
            </span>
            <span style="font-size: var(--text-sm); color: var(--text-secondary);">Lima</span>
          </div>
        </div>
      </div>
    {/if}

    <!-- Quick Instance List -->
    {#if dashboardState.colimaInstances.length > 0}
      <div class="card">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px;">
          <h3 style="font-size: var(--text-lg); font-weight: 600;">Instances</h3>
          <button class="btn btn-ghost" onclick={() => onNavigate("instances")} style="font-size: var(--text-xs);">View All →</button>
        </div>
        <table class="data-table">
          <thead>
            <tr>
              <th>Profile</th><th>Status</th><th>Runtime</th><th>Arch</th><th>Resources</th>
            </tr>
          </thead>
          <tbody>
            {#each dashboardState.colimaInstances as inst (inst.name)}
              <tr>
                <td style="font-weight: 500;">{inst.name}</td>
                <td>
                  <span class="badge {inst.status === 'Running' ? 'badge-running' : 'badge-stopped'}">
                    <span class="badge-dot" style="animation: none;"></span>
                    <span>{inst.status}</span>
                  </span>
                </td>
                <td style="color: var(--text-secondary);">{inst.runtime}</td>
                <td style="color: var(--text-secondary);">{inst.arch}</td>
                <td style="color: var(--text-secondary); font-family: var(--font-mono); font-size: var(--text-xs);">
                  {inst.cpus || '—'} CPU · {formatBytes(inst.memory)} · {formatBytes(inst.disk)}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="color: var(--text-muted);">
            <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
          </svg>
        </div>
        <div class="empty-state-title">No instances found</div>
        <div class="empty-state-text">Create your first Colima instance to get started.</div>
        <button class="btn btn-primary" onclick={() => onNavigate("instances")}>Create Instance</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .dash-section {
    margin-bottom: 28px;
  }

  .dash-section-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .dash-section-icon {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(88, 166, 255, 0.15);
    border: 1px solid rgba(88, 166, 255, 0.25);
    flex-shrink: 0;
  }

  .dash-section-icon svg {
    width: 15px;
    height: 15px;
  }

  .dash-icon-blue {
    background: rgba(88, 166, 255, 0.15);
    border-color: rgba(88, 166, 255, 0.25);
    color: var(--accent-blue);
  }

  .dash-section-title {
    font-size: var(--text-base);
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: 0.01em;
  }

  .dash-grid-3 {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
  }

  /* Placeholder sized to the tile it becomes, so the grid does not reflow. */
  .skeleton {
    background: linear-gradient(
      90deg,
      var(--bg-card) 25%,
      var(--bg-card-hover) 50%,
      var(--bg-card) 75%
    );
    background-size: 200% 100%;
    animation: skeleton-shimmer 1.5s linear infinite;
    border-radius: var(--radius-md);
  }
  .skeleton-tile {
    height: 96px;
  }

  @keyframes skeleton-shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  /* A looping shimmer is exactly the motion "reduce motion" exists to stop. */
  @media (prefers-reduced-motion: reduce) {
    .skeleton { animation: none; }
  }

  .dash-grid-5 {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 14px;
  }

  @media (max-width: 900px) {
    .dash-grid-3 {
      grid-template-columns: repeat(2, 1fr);
    }
    .dash-grid-5 {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  @media (max-width: 600px) {
    .dash-grid-3,
    .dash-grid-5 {
      grid-template-columns: 1fr;
    }
  }
</style>

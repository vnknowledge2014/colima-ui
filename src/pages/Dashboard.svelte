<script lang="ts">
   
  import { composeApi, k8sApi, limaApi, kindApi, type SystemInfo } from "../lib/api";
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
  let totalCpus = $derived(dashboardState.colimaInstances.filter(i => i.status === "Running").reduce((sum, i) => sum + i.cpus, 0));

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

  async function fetchDockerCounts() {
    try {
      const compose = await composeApi.list();
      dashboardState.composeProjectsCount = compose ? compose.length : 0;
    } catch {}
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
        dashboardState.lastFetch = now;
      }
    }
  });
</script>



<div class="content-header">
  <div class="header-left">
    <h1>{t('dashboard.title', { default: 'Dashboard' })}</h1>
  </div>
</div>

<div class="content-body" style="padding: 24px; max-width: 1200px;">
  {#if loading}
    <div style="display: flex; justify-content: center; align-items: center; height: 200px;">
      <div class="spinner"></div>
      <span style="margin-left: 10px; color: var(--text-muted);">{t('dashboard.loading', { default: 'Loading...' })}</span>
    </div>
  {:else}
    <!-- SECTION 1: Colima Instances -->
    <div class="stat-section">
      <h2 class="section-title">
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: var(--accent-blue);"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
        {t('dashboard.vm_instances', { default: 'VM Instances' })} 
        <span class="badge">{dashboardState.colimaInstances.length}</span>
      </h2>
      <div class="stats-grid">
        <button class="stat-card" onclick={() => onNavigate("instances")}>
          <div class="stat-title">{t('dashboard.running_vms', { default: 'Running VMs' })}</div>
          <div class="stat-value">{runningCount}</div>
          <div class="stat-icon" style="color: var(--accent-green);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>
          </div>
        </button>
        <button class="stat-card" onclick={() => onNavigate("instances")}>
          <div class="stat-title">{t('dashboard.stopped_vms', { default: 'Stopped VMs' })}</div>
          <div class="stat-value">{stoppedCount}</div>
          <div class="stat-icon" style="color: var(--text-muted);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="16"></line><line x1="8" y1="12" x2="16" y2="12"></line></svg>
          </div>
        </button>
        <div class="stat-card">
          <div class="stat-title">{t('dashboard.total_cpus', { default: 'Total CPUs' })}</div>
          <div class="stat-value">{totalCpus}</div>
          <div class="stat-icon" style="color: var(--accent-yellow);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect><rect x="9" y="9" width="6" height="6"></rect><line x1="9" y1="1" x2="9" y2="4"></line><line x1="15" y1="1" x2="15" y2="4"></line><line x1="9" y1="20" x2="9" y2="23"></line><line x1="15" y1="20" x2="15" y2="23"></line><line x1="20" y1="9" x2="23" y2="9"></line><line x1="20" y1="14" x2="23" y2="14"></line><line x1="1" y1="9" x2="4" y2="9"></line><line x1="1" y1="14" x2="4" y2="14"></line></svg>
          </div>
        </div>
      </div>
    </div>

    <!-- SECTION 2: Docker Engine -->
    <div class="stat-section">
      <h2 class="section-title">
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: #2496ed;"><path d="M22 12.8c-.8 0-1.6-.3-2.3-.7-2 1.4-4.5 1.7-6.8 1-2.3.7-4.8.4-6.8-1-.7.4-1.5.7-2.3.7-1.1 0-2.2-.4-3-1.1v4c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2v-4c-.8.7-1.9 1.1-3 1.1z"></path><path d="M12 9c1.7 0 3-1.3 3-3s-1.3-3-3-3-3 1.3-3 3 1.3 3 3 3z"></path></svg>
        Docker Engine
      </h2>
      <div class="stats-grid">
        <button class="stat-card" onclick={() => onNavigate("containers")}>
          <div class="stat-title">{t('dashboard.containers', { default: 'Containers' })}</div>
          <div class="stat-value">
            {dockerCounts.containers}
            <span class="stat-subtitle" style="color: var(--accent-green); margin-left: 8px; font-size: 0.6em;">{dockerCounts.running} {t('dashboard.running', { default: 'Running' })}</span>
          </div>
          <div class="stat-icon" style="color: #2496ed;">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect><rect x="9" y="9" width="6" height="6"></rect></svg>
          </div>
        </button>
        <button class="stat-card" onclick={() => onNavigate("images")}>
          <div class="stat-title">{t('dashboard.images', { default: 'Images' })}</div>
          <div class="stat-value">{dockerCounts.images}</div>
          <div class="stat-icon" style="color: var(--text-muted);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path></svg>
          </div>
        </button>
        <button class="stat-card" onclick={() => onNavigate("volumes")}>
          <div class="stat-title">{t('dashboard.volumes', { default: 'Volumes' })}</div>
          <div class="stat-value">{dockerCounts.volumes}</div>
          <div class="stat-icon" style="color: var(--text-muted);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"></ellipse><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path></svg>
          </div>
        </button>
        <button class="stat-card" onclick={() => onNavigate("networks")}>
          <div class="stat-title">{t('dashboard.networks', { default: 'Networks' })}</div>
          <div class="stat-value">{dockerCounts.networks}</div>
          <div class="stat-icon" style="color: var(--text-muted);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="16" y="16" width="6" height="6" rx="1"></rect><rect x="2" y="16" width="6" height="6" rx="1"></rect><rect x="9" y="2" width="6" height="6" rx="1"></rect><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"></path><path d="M12 12V8"></path></svg>
          </div>
        </button>
        <button class="stat-card" onclick={() => onNavigate("compose")}>
          <div class="stat-title">{t('dashboard.compose_projects', { default: 'Compose Projects' })}</div>
          <div class="stat-value">{dockerCounts.composeProjects}</div>
          <div class="stat-icon" style="color: var(--text-muted);">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 2 7 12 12 22 7 12 2"></polygon><polyline points="2 17 12 22 22 17"></polyline><polyline points="2 12 12 17 22 12"></polyline></svg>
          </div>
        </button>
      </div>
    </div>

    <!-- Kubernetes & Infrastructure -->
    <div class="card" style="margin-bottom: 24px;">
      <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 16px;">Infrastructure</h3>
      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 16px;">
        
        <button onclick={() => onNavigate("kubernetes")} class="metric-card {dashboardState.k8sStatus.connected ? 'metric-card-green' : 'metric-card-red'}">
          <div class="metric-header">
            <span class="metric-title">Kubernetes</span>
            <div class="metric-icon-wrapper"><svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg></div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dashboardState.k8sStatus.connected ? dashboardState.k8sStatus.pods : 0}</span>
            <span class="metric-sub">{dashboardState.k8sStatus.connected ? `${dashboardState.k8sStatus.namespaces} namespaces` : "Disconnected"}</span>
          </div>
        </button>

        <button onclick={() => onNavigate("instances")} class="metric-card metric-card-purple">
          <div class="metric-header">
            <span class="metric-title">Kind Clusters</span>
            <div class="metric-icon-wrapper"><svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg></div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dashboardState.k8sStatus.kindClusters}</span>
            <span class="metric-sub">Local clusters</span>
          </div>
        </button>

        <button onclick={() => onNavigate("linux-vms")} class="metric-card metric-card-orange">
          <div class="metric-header">
            <span class="metric-title">Linux VMs</span>
            <div class="metric-icon-wrapper"><svg class="metric-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg></div>
          </div>
          <div class="metric-body">
            <span class="metric-value">{dashboardState.linuxVMs.length}</span>
            <span class="metric-sub">{dashboardState.linuxVMs.filter(v => v.status === "Running").length} running</span>
          </div>
        </button>
      </div>
    </div>

    <!-- System Info -->
    {#if systemInfo}
      <div class="card" style="margin-bottom: 24px;">
        <h3 style="font-size: var(--text-lg); font-weight: 600; margin-bottom: 16px;">System Status</h3>
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="badge {systemInfo.colima_installed ? 'badge-running' : 'badge-stopped'}"><span class="badge-dot"></span>{systemInfo.colima_installed ? 'Installed' : 'Not Found'}</span>
            <span style="font-size: var(--text-sm); color: var(--text-secondary);">Colima</span>
          </div>
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="badge {systemInfo.docker_installed ? 'badge-running' : 'badge-stopped'}"><span class="badge-dot"></span>{systemInfo.docker_installed ? 'Installed' : 'Not Found'}</span>
            <span style="font-size: var(--text-sm); color: var(--text-secondary);">Docker</span>
          </div>
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="badge {systemInfo.lima_installed ? 'badge-running' : 'badge-stopped'}"><span class="badge-dot"></span>{systemInfo.lima_installed ? 'Installed' : 'Not Found'}</span>
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
            {#each dashboardState.colimaInstances as inst}
              <tr>
                <td style="font-weight: 500;">{inst.name}</td>
                <td>
                  <span class="badge {inst.status === 'Running' ? 'badge-running' : 'badge-stopped'}">
                    <span class="badge-dot"></span>{inst.status}
                  </span>
                </td>
                <td style="color: var(--text-secondary);">{inst.runtime}</td>
                <td style="color: var(--text-secondary);">{inst.arch}</td>
                <td style="color: var(--text-secondary); font-family: var(--font-mono); font-size: var(--text-xs);">
                  {inst.cpus} CPU · {formatBytes(inst.memory)} · {formatBytes(inst.disk)}
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

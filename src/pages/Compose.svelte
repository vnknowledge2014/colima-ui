<script lang="ts">
  import { onMount } from "svelte";
  import { composeApi, type ComposeProject } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";

  let projects = $state<ComposeProject[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actionLoading = $state<string | null>(null);
  let selectedProject = $state<ComposeProject | null>(null);
  let logs = $state("");
  let services = $state("");
  let detailTab = $state<"services" | "logs">("services");

  async function fetchProjects() {
    try {
      error = null;
      const list = await composeApi.list();
      projects = list;
    } catch (e) {
      error = String(e);
      projects = [];
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    fetchProjects();
    const interval = setInterval(() => {
      if (document.visibilityState === "visible") fetchProjects();
    }, 15000);
    return () => clearInterval(interval);
  });

  async function handleAction(name: string, action: "down" | "restart") {
    actionLoading = `${name}-${action}`;
    try {
      if (action === "down") {
        await composeApi.down(name);
        globalToast("success", `Project '${name}' stopped`);
      } else {
        await composeApi.restart(name);
        globalToast("success", `Project '${name}' restarted`);
      }
      await fetchProjects();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  async function openProject(p: ComposeProject) {
    selectedProject = p;
    detailTab = "services";
    try {
      const [svc, lg] = await Promise.all([
        composeApi.ps(p.Name),
        composeApi.logs(p.Name, 100),
      ]);
      services = svc;
      logs = lg;
    } catch (e) {
      services = `Error: ${e}`;
      logs = `Error: ${e}`;
    }
  }

  function parseStatus(status: string) {
    if (!status) return { running: 0, display: "" };
    const match = status.match(/running\((\d+)\)/);
    const running = match ? parseInt(match[1]) : 0;
    return { running, display: status };
  }

</script>

<div class="content-header">
  <h1>
    {t('compose.title', { default: 'Docker Compose' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {projects.length} {t('compose.projects_count', { default: 'projects' })}
    </span>
  </h1>
  <div class="content-header-actions">
    <button class="btn btn-ghost" onclick={fetchProjects} aria-label="Refresh Projects" title="Refresh Projects">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
      </svg>
    </button>
  </div>
</div>

<div class="content-body">
  {#if loading}
    <div class="loading-screen"><div class="spinner"></div><span>Loading projects...</span></div>
  {:else}
    {#if error}
      <div class="card" style="border-color: var(--accent-red); margin-bottom: 20px; padding: 12px;">
        <p style="color: var(--accent-red); font-size: var(--text-sm); display: flex; align-items: center; gap: 6px;">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          {error}
        </p>
      </div>
    {/if}

    {#if projects.length > 0}
      <div style="display: flex; flex-direction: column; gap: 8px;">
        {#each projects as p (p.Name)}
          {@const { running } = parseStatus(p.Status)}
          {@const isLoading = actionLoading?.startsWith(p.Name)}
          {@const hasRunning = running > 0}
          <div role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => openProject(p)} style="padding: 16px; background: var(--bg-secondary); border-radius: 12px; border: 1px solid var(--border-primary); cursor: pointer; opacity: {isLoading ? 0.6 : 1}; transition: all 200ms;">
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <div>
                <div style="display: flex; align-items: center; gap: 8px;">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={hasRunning ? "var(--accent-blue)" : "var(--text-muted)"} stroke-width="2">
                    <path d="M22 8.35V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8.35A2 2 0 0 1 3.26 6.5l8-3.2a2 2 0 0 1 1.48 0l8 3.2A2 2 0 0 1 22 8.35Z"/>
                    <path d="M6 18h12M6 14h12M6 10h12"/>
                  </svg>
                  <span style="font-weight: 600; font-size: var(--text-md);">{p.Name}</span>
                  <span class="badge badge-{hasRunning ? 'running' : 'stopped'}">
                    <span class="badge-dot"></span>
                    {p.Status}
                  </span>
                </div>
                {#if p.ConfigFiles}
                  <div style="color: var(--text-muted); font-size: var(--text-xs); margin-top: 4px; font-family: var(--font-mono);">
                    {p.ConfigFiles}
                  </div>
                {/if}
              </div>
              <div role="button" tabindex="0" style="display: flex; gap: 6px;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={e => e.stopPropagation()}>
                <button class="btn btn-ghost" style="font-size: var(--text-xs);" disabled={!!isLoading} onclick={() => handleAction(p.Name, "restart")}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
                  {t('compose.restart', { default: 'Restart' })}
                </button>
                <button class="btn btn-ghost" style="font-size: var(--text-xs); color: var(--accent-red);" disabled={!!isLoading} onclick={() => handleAction(p.Name, "down")}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/></svg>
                  {t('compose.down', { default: 'Down' })}
                </button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="color: var(--text-muted);">
            <path d="M22 8.35V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8.35A2 2 0 0 1 3.26 6.5l8-3.2a2 2 0 0 1 1.48 0l8 3.2A2 2 0 0 1 22 8.35Z"/>
          </svg>
        </div>
        <div class="empty-state-title">{error ? t('compose.error_title', { default: 'Docker not connected' }) : t('compose.empty_title', { default: 'No Compose Projects' })}</div>
        <div class="empty-state-text">{error ? t('compose.error_text', { default: 'Docker daemon is not connected. Make sure a Colima profile is running.' }) : t('compose.empty_text', { default: 'Start a docker-compose project to see it listed here.' })}</div>
      </div>
    {/if}
  {/if}
</div>

{#if selectedProject}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => selectedProject = null}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={e => e.stopPropagation()} style="width: min(800px, 95vw); max-height: 80vh;">
      <div class="modal-header">
        <h2 class="modal-title">{selectedProject.Name}</h2>
        <button class="btn btn-icon btn-ghost" onclick={() => selectedProject = null} aria-label="Close modal" title="Close modal">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>

      <div style="display: flex; gap: 2px; border-bottom: 1px solid var(--border-primary); margin-bottom: 16px;">
        <button class="btn" style="background: transparent; border: none; border-bottom: {detailTab === 'services' ? '2px solid var(--accent-blue)' : '2px solid transparent'}; color: {detailTab === 'services' ? 'var(--text-primary)' : 'var(--text-secondary)'}; border-radius: 0; padding: 8px 16px; font-weight: {detailTab === 'services' ? 600 : 400};" onclick={() => detailTab = "services"}>Services</button>
        <button class="btn" style="background: transparent; border: none; border-bottom: {detailTab === 'logs' ? '2px solid var(--accent-blue)' : '2px solid transparent'}; color: {detailTab === 'logs' ? 'var(--text-primary)' : 'var(--text-secondary)'}; border-radius: 0; padding: 8px 16px; font-weight: {detailTab === 'logs' ? 600 : 400};" onclick={() => detailTab = "logs"}>Logs</button>
      </div>

      {#if detailTab === "services"}
        <pre style="padding: 12px; background: var(--bg-primary); border-radius: 8px; font-size: var(--text-xs); overflow: auto; max-height: 50vh; color: var(--text-secondary); margin: 0; font-family: var(--font-mono);">
          {services || "No services running"}
        </pre>
      {/if}

      {#if detailTab === "logs"}
        <div class="log-viewer" style="max-height: 50vh;">
          {#each logs.split("\n") as line, i}
            {@const cls = /error|fatal|panic/i.test(line) ? "log-error" : /warn/i.test(line) ? "log-warn" : ""}
            <div class="log-line {cls}">
              <span style="color: var(--text-muted); margin-right: 8px; user-select: none;">{i + 1}</span>
              {line}
            </div>
          {/each}
        </div>
      {/if}

      <div class="modal-footer">
        <button class="btn btn-primary" onclick={() => selectedProject = null}>Close</button>
      </div>
    </div>
  </div>
{/if}

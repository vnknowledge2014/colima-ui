<script lang="ts">
  import { onMount } from "svelte";
  import { composeApi, type ComposeProject } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import DiagnosePanel from "../components/compose/DiagnosePanel.svelte";
  import RowActions from "../components/RowActions.svelte";
  import * as Icons from "../components/Icons.svelte";
  import { viewInTopology, consumeFocus } from "../lib/topology-link";

  let projects = $state<ComposeProject[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let actionLoading = $state<string | null>(null);
  let selectedProject = $state<ComposeProject | null>(null);
  let logs = $state("");
  let services = $state("");
  let detailTab = $state<"services" | "logs" | "diagnose">("services");

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
    fetchProjects().then(() => {
      // Arrived from the topology graph's "Open in Compose": open that
      // project's panel. A project that has since gone is ignored.
      const focus = consumeFocus("compose");
      const match = focus ? projects.find((p) => p.Name === focus) : undefined;
      if (match) openProject(match);
    });
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
      services = formatServices(svc);
      logs = lg;
    } catch (e) {
      services = `Error: ${e}`;
      logs = `Error: ${e}`;
    }
  }

  // `docker compose ps --format json` emits JSONL (one object per line).
  // Parse and pretty-print so the Services tab shows readable, indented JSON
  // instead of raw lines; fall back to the original text if anything is off.
  function formatServices(raw: string): string {
    const lines = raw
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.startsWith("{"));
    if (lines.length === 0) return raw;
    try {
      const parsed = lines.map((l) => JSON.parse(l));
      return JSON.stringify(parsed, null, 2);
    } catch {
      return raw;
    }
  }

  function parseStatus(status: string) {
    if (!status) return { running: 0, display: "" };
    const match = status.match(/running\((\d+)\)/);
    const running = match ? parseInt(match[1]) : 0;
    return { running, display: status };
  }

</script>

<div class="content-header" data-tauri-drag-region>
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
      <div class="resource-card-list">
        {#each projects as p (p.Name)}
          {@const { running } = parseStatus(p.Status)}
          {@const isLoading = actionLoading?.startsWith(p.Name)}
          {@const hasRunning = running > 0}
          <div role="button" tabindex="0" class="resource-card" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => openProject(p)} style="cursor: pointer; opacity: {isLoading ? 0.6 : 1};">
            <div class="resource-card-body">
              <div style="min-width: 0;">
                <div style="display: flex; align-items: center; gap: 8px;">
                  <span class="compose-project-icon" style="color: {hasRunning ? 'var(--accent-blue)' : 'var(--text-muted)'};">{@html Icons.Container}</span>
                  <span class="resource-card-title">{p.Name}</span>
                  <span class="badge badge-{hasRunning ? 'running' : 'stopped'}">
                    <span class="badge-dot"></span>
                    {p.Status}
                  </span>
                </div>
                {#if p.ConfigFiles}
                  <div class="resource-card-meta" style="font-family: var(--font-mono);">
                    {p.ConfigFiles}
                  </div>
                {/if}
              </div>
              <!-- Restart stays inline as the routine move; Down tears the whole
                   project off, so it sits behind the overflow menu. -->
              <RowActions
                inline={[
                  {
                    icon: Icons.Topology,
                    label: t('common.view_in_topology', { default: 'View in topology' }),
                    onclick: () => viewInTopology("project", p.Name),
                  },
                  {
                    icon: Icons.Refresh,
                    label: t('compose.restart', { default: 'Restart' }),
                    disabled: !!isLoading,
                    onclick: () => handleAction(p.Name, "restart"),
                  },
                ]}
                menu={[{
                  label: t('compose.down', { default: 'Down' }),
                  icon: Icons.Stop,
                  danger: true,
                  disabled: !!isLoading,
                  action: () => handleAction(p.Name, "down"),
                }]}
              />
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
        <button class="btn" style="background: transparent; border: none; border-bottom: {detailTab === 'diagnose' ? '2px solid var(--accent-blue)' : '2px solid transparent'}; color: {detailTab === 'diagnose' ? 'var(--text-primary)' : 'var(--text-secondary)'}; border-radius: 0; padding: 8px 16px; font-weight: {detailTab === 'diagnose' ? 600 : 400};" onclick={() => detailTab = "diagnose"}>
          <!-- No PRO badge: this tab is free. The badge sat here while the tab
               was gated, and leaving it would tell free users a feature they can
               use is out of reach. The Pro offer is inside the panel, on the
               auto-fix affordance, which is the part that is actually paid. -->
          {t('compose.diagnose.tab', { default: 'Diagnose' })}
        </button>
      </div>

      {#if detailTab === "services"}
        <pre style="padding: 12px; background: var(--bg-primary); border-radius: 8px; font-size: var(--text-xs); overflow: auto; max-height: 50vh; color: var(--text-secondary); margin: 0; font-family: var(--font-mono);">{services || "No services running"}</pre>
      {/if}

      {#if detailTab === "diagnose"}
        <!--
          Diagnosis is free, and is not gated.

          It was, behind a sidecar capability that nothing declares — so the tab
          was locked for every user including paying ones, while the backend it
          calls (`/api/compose/diagnose`) shipped and worked.

          Diagnosing costs nothing per use: Docker's own validation plus a local
          Knowledge Bank lookup, no network. The AI step inside the panel runs on
          the user's own key.

          The Pro offer lives one level down, in `DiagnosePanel`: applying the
          fix, not reading it.
        -->
        <div style="max-height: 50vh; overflow: auto;">
          <DiagnosePanel configFiles={selectedProject.ConfigFiles} />
        </div>
      {/if}

      {#if detailTab === "logs"}
        <div class="log-viewer" style="max-height: 50vh;">
          {#each logs.split("\n") as line, i (i)}
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

<style>
  /* Project glyph takes its colour from the row state, matching how the status
     dot is coloured on the containers table. */
  .compose-project-icon {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
  }
</style>

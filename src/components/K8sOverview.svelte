<script lang="ts">
  import { onMount } from "svelte";
  import { k8sApi } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { k8sState } from "../store/k8s.svelte";

  interface HealthIssue {
    severity: string;
    category: string;
    resource: string;
    message: string;
  }

  let loading = $state(true);
  let score = $state<number | null>(null);
  let grade = $state("");
  let issues = $state<HealthIssue[]>([]);
  let counts = $state({ nodes: 0, pods: 0, deployments: 0, services: 0, namespaces: 0 });

  function countFrom(raw: string | unknown[]): number {
    if (Array.isArray(raw)) return raw.length;
    try {
      const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
      return (parsed?.items || []).length;
    } catch {
      return 0;
    }
  }

  async function refresh() {
    loading = true;
    try {
      const raw = await k8sApi.clusterHealth();
      const data = JSON.parse(raw);
      score = data.score ?? null;
      grade = data.grade ?? "";
      issues = data.issues ?? [];
    } catch (e) {
      globalToast("error", `Health scan failed: ${e}`);
    }

    try { counts.nodes = countFrom(await k8sApi.nodesJson()); } catch { counts.nodes = 0; }
    try { counts.namespaces = countFrom(await k8sApi.namespaces()); } catch { counts.namespaces = 0; }
    try { counts.pods = countFrom(await k8sApi.resources("pods", k8sState.namespace)); } catch { counts.pods = 0; }
    try { counts.deployments = countFrom(await k8sApi.resources("deployments", k8sState.namespace)); } catch { counts.deployments = 0; }
    try { counts.services = countFrom(await k8sApi.resources("services", k8sState.namespace)); } catch { counts.services = 0; }

    loading = false;
  }

  onMount(() => { refresh(); });

  const gradeColor = $derived(grade === "A" ? "var(--accent-green)" : grade === "B" ? "var(--accent-yellow)" : grade === "C" ? "var(--accent-orange)" : "var(--accent-red)");
  const errorCount = $derived(issues.filter(i => i.severity === "error").length);
  const warnCount = $derived(issues.filter(i => i.severity === "warning").length);
</script>

<div>
  <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
    <h2 style="font-size: var(--text-lg); color: var(--text-primary); margin: 0;">Cluster Overview</h2>
    <div style="display: flex; gap: 8px; align-items: center;">
      <span style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono);">Context: {k8sState.currentCtx || "—"}</span>
      <button class="btn btn-ghost" onclick={refresh} disabled={loading} style="font-size: var(--text-xs);">Refresh</button>
    </div>
  </div>

  {#if loading}
    <div style="display: flex; justify-content: center; padding: 40px;"><div class="spinner"></div></div>
  {:else}
    <!-- Health score + status -->
    <div style="display: grid; grid-template-columns: 200px 1fr; gap: 16px; margin-bottom: 16px;">
      <div class="overview-score-card" style="border: 1px solid {gradeColor};">
        <div class="overview-score-grade" style="color: {gradeColor};">{grade || "—"}</div>
        <div class="overview-score-value" style="color: {gradeColor};">{score ?? "—"}<span style="font-size: 12px;">/100</span></div>
        <div class="overview-score-label">Health Score</div>
      </div>
      <div class="overview-issues-card">
        <div style="display: flex; gap: 20px; align-items: center; height: 100%;">
          <div>
            <div style="font-size: var(--text-2xl); font-weight: 700; color: var(--accent-red);">{errorCount}</div>
            <div style="font-size: 10px; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted);">Errors</div>
          </div>
          <div>
            <div style="font-size: var(--text-2xl); font-weight: 700; color: var(--accent-yellow);">{warnCount}</div>
            <div style="font-size: 10px; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted);">Warnings</div>
          </div>
          <div style="flex: 1; font-size: var(--text-xs); color: var(--text-muted); line-height: 1.6;">
            {errorCount === 0 && warnCount === 0 ? "Cluster looks healthy — no issues found." : `${issues.length} issue(s) found. Check the list below.`}
          </div>
        </div>
      </div>
    </div>

    <!-- Resource counts -->
    <div class="overview-count-grid">
      <div class="overview-count-card"><div class="overview-count-value">{counts.nodes}</div><div class="overview-count-label">Nodes</div></div>
      <div class="overview-count-card"><div class="overview-count-value">{counts.pods}</div><div class="overview-count-label">Pods</div></div>
      <div class="overview-count-card"><div class="overview-count-value">{counts.deployments}</div><div class="overview-count-label">Deployments</div></div>
      <div class="overview-count-card"><div class="overview-count-value">{counts.services}</div><div class="overview-count-label">Services</div></div>
      <div class="overview-count-card"><div class="overview-count-value">{counts.namespaces}</div><div class="overview-count-label">Namespaces</div></div>
    </div>

    <!-- Issues -->
    {#if issues.length > 0}
      <div style="margin-top: 20px;">
        <div style="font-size: var(--text-xs); font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin-bottom: 8px;">Issues ({issues.length})</div>
        <div class="card" style="overflow: auto;">
          {#each issues.slice(0, 20) as issue (issue.resource + issue.category + issue.message)}
            <div style="display: flex; align-items: center; gap: 10px; padding: 8px 14px; border-bottom: 1px solid var(--border-subtle); font-size: var(--text-xs);">
              <span style="padding: 2px 8px; border-radius: 10px; font-size: 10px; font-weight: 600; text-transform: uppercase; background: {issue.severity === 'error' ? 'rgba(248,81,73,0.12)' : 'rgba(210,153,34,0.12)'}; color: {issue.severity === 'error' ? 'var(--accent-red)' : 'var(--accent-yellow)'}; flex-shrink: 0;">{issue.severity}</span>
              <span style="font-family: var(--font-mono); color: var(--accent-blue); flex-shrink: 0;">{issue.category}</span>
              <span style="color: var(--text-primary); flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{issue.resource}</span>
              <span style="color: var(--text-muted); flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{issue.message}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .overview-score-card {
    background: var(--bg-card);
    border-radius: var(--radius-lg);
    padding: 18px;
    text-align: center;
  }
  .overview-score-grade {
    font-size: var(--text-2xl);
    font-weight: 800;
  }
  .overview-score-value {
    font-size: var(--text-xl);
    font-weight: 700;
    font-family: var(--font-mono);
    margin-top: 2px;
  }
  .overview-score-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin-top: 4px;
  }
  .overview-issues-card {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: 18px;
  }
  .overview-count-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 10px;
  }
  .overview-count-card {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 12px;
    text-align: center;
  }
  .overview-count-value {
    font-size: var(--text-xl);
    font-weight: 700;
    font-family: var(--font-mono);
    color: var(--accent-blue);
  }
  .overview-count-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin-top: 2px;
  }
</style>

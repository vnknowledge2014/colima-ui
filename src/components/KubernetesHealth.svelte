<script lang="ts">
  import { k8sApi } from "../lib/api";
  import { globalToast } from "../lib/globalToast";

  interface HealthIssue {
    severity: string;
    category: string;
    resource: string;
    message: string;
  }

  interface HealthData {
    score: number;
    grade: string;
    issues: HealthIssue[];
  }

  let healthLoading = $state(false);
  let healthData = $state<HealthData | null>(null);

  async function runHealthScan() {
    healthLoading = true;
    try {
      const raw = await k8sApi.clusterHealth();
      healthData = JSON.parse(raw) as HealthData;
    } catch (e) {
      globalToast("error", `Health scan failed: ${e}`);
    } finally {
      healthLoading = false;
    }
  }
</script>

<div>
  <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
    <h2 style="font-size: var(--text-lg); color: var(--text-primary); margin: 0;">Cluster Health</h2>
    <button class="btn btn-primary" onclick={runHealthScan} disabled={healthLoading}>
      {#if healthLoading}
        <div class="spinner" style="width: 14px; height: 14px;"></div> Scanning...
      {:else}
        Run Health Scan
      {/if}
    </button>
  </div>
  {#if !healthData}
    <div class="empty-state">
      <div class="empty-state-icon" style="font-size: 48px;">🩺</div>
      <div class="empty-state-title">Cluster Health Analysis</div>
      <div class="empty-state-text">
        Analyze your cluster for misconfigurations, unhealthy resources, and potential issues.<br/>
        Inspired by <strong>Popeye</strong> — a Kubernetes cluster sanitizer.
      </div>
      <button class="btn btn-primary" onclick={runHealthScan} disabled={healthLoading}>Run Scan</button>
    </div>
  {:else}
    <!-- Score card -->
    <div class="card" style="display: flex; gap: 24px; padding: 20px; margin-bottom: 16px; align-items: center;">
      <div style="position: relative; width: 80px; height: 80px;">
        <svg width="80" height="80" viewBox="0 0 80 80">
          <circle cx="40" cy="40" r="36" fill="none" stroke="var(--border-primary)" stroke-width="6" />
          <circle cx="40" cy="40" r="36" fill="none"
            stroke={healthData.score >= 90 ? "var(--accent-green)" : healthData.score >= 60 ? "var(--accent-yellow)" : "var(--accent-red)"}
            stroke-width="6" stroke-linecap="round" stroke-dasharray="{(healthData.score / 100) * 226} 226"
            transform="rotate(-90 40 40)" />
        </svg>
        <div style="position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: var(--text-xl); font-weight: 700; color: var(--text-primary);">
          {healthData.score}
        </div>
      </div>
      <div>
        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 4px;">
          <span style="padding: 2px 10px; border-radius: 6px; font-weight: 700; font-size: var(--text-lg);
            background: {healthData.grade === 'A' ? 'rgba(63,185,80,0.15)' : healthData.grade === 'B' ? 'rgba(88,166,255,0.15)' : healthData.grade === 'C' ? 'rgba(210,153,34,0.15)' : 'rgba(248,81,73,0.15)'};
            color: {healthData.grade === 'A' ? 'var(--accent-green)' : healthData.grade === 'B' ? 'var(--accent-blue)' : healthData.grade === 'C' ? 'var(--accent-yellow)' : 'var(--accent-red)'};
          ">Grade: {healthData.grade}</span>
        </div>
        <div style="font-size: var(--text-sm); color: var(--text-muted);">
          {healthData.issues?.filter((i: HealthIssue) => i.severity === "error").length || 0} errors,
          {healthData.issues?.filter((i: HealthIssue) => i.severity === "warning").length || 0} warnings
        </div>
      </div>
    </div>

    <!-- Issues list -->
    <div class="card" style="padding: 0; overflow: hidden;">
      <div style="padding: 10px 16px; border-bottom: 1px solid var(--border-primary); font-size: var(--text-sm); font-weight: 600; color: var(--text-primary);">
        Issues ({healthData.issues?.length || 0})
      </div>
      <div style="max-height: 50vh; overflow: auto;">
        {#each healthData.issues || [] as issue (issue.message + issue.resource + issue.category)}
          <div style="display: flex; align-items: flex-start; gap: 12px; padding: 8px 16px; border-bottom: 1px solid var(--border-subtle);
            background: {issue.severity === 'error' ? 'rgba(248,81,73,0.03)' : issue.severity === 'warning' ? 'rgba(210,153,34,0.03)' : 'transparent'};
          ">
            <svg width="8" height="8" viewBox="0 0 24 24" fill={issue.severity === 'error' ? 'var(--accent-red)' : issue.severity === 'warning' ? 'var(--accent-yellow)' : 'var(--accent-blue)'} style="margin-top: 5px; flex-shrink: 0;"><circle cx="12" cy="12" r="10"/></svg>
            <div style="flex: 1; min-width: 0;">
              <div style="display: flex; gap: 8px; align-items: center; margin-bottom: 2px;">
                <span style="padding: 1px 6px; border-radius: 4px; font-size: 10px; font-weight: 600; background: rgba(88,166,255,0.1); color: var(--accent-blue);">
                  {issue.category}
                </span>
                <span style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono);">
                  {issue.resource}
                </span>
              </div>
              <div style="font-size: var(--text-sm); color: var(--text-secondary);">{issue.message}</div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

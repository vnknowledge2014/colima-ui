<script lang="ts">
  import { onMount } from "svelte";
  import { colimaApi, type ColimaInstance } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  let instances = $state<ColimaInstance[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let masters = $derived(instances.filter(i => !i.name.toLowerCase().includes('worker')));
  let workers = $derived(instances.filter(i => i.name.toLowerCase().includes('worker')));

  async function fetchTopology() {
    try {
      loading = true;
      const allInstances = await colimaApi.listInstances();
      // Filter only kubernetes instances
      instances = allInstances.filter(i => i.kubernetes);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    fetchTopology();
  });
</script>

<div style="padding: 24px; height: 100%; display: flex; flex-direction: column;">
  <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;">
    <div>
      <h2 style="margin: 0; font-size: var(--text-xl); font-weight: 700;">{t('topology.title', { default: 'Cluster Topology' })}</h2>
      <div style="font-size: var(--text-sm); color: var(--text-muted); margin-top: 4px;">{t('topology.subtitle', { default: 'Visual representation of K3s Master and Worker nodes' })}</div>
    </div>
    <button class="btn btn-ghost" onclick={fetchTopology} disabled={loading} style="display: flex; align-items: center; gap: 6px;">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
      {t('topology.refresh', { default: 'Refresh' })}
    </button>
  </div>

  {#if loading && instances.length === 0}
    <div style="display: flex; justify-content: center; align-items: center; flex: 1;">
      <div class="spinner" style="width: 24px; height: 24px;"></div>
    </div>
  {:else if error}
    <div class="empty-state">
      <div class="empty-state-title" style="color: var(--accent-red);">{t('topology.error_title', { default: 'Error loading topology' })}</div>
      <div class="empty-state-text">{error}</div>
    </div>
  {:else if instances.length === 0}
    <div class="empty-state">
      <div class="empty-state-title">{t('topology.no_nodes', { default: 'No K3s Nodes Found' })}</div>
      <div class="empty-state-text">{t('topology.empty_text', { default: 'Enable Kubernetes on your Colima instances to view topology.' })}</div>
    </div>
  {:else}
    <div style="flex: 1; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 12px; padding: 32px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 40px; overflow: auto;">
      <div style="display: flex; flex-wrap: wrap; gap: 24px; justify-content: center;">
        {#each masters as m}
          <div style="background: var(--bg-primary); border: 2px solid var(--accent-purple); border-radius: 12px; padding: 20px; width: 240px; box-shadow: 0 10px 30px rgba(167,139,250,0.1); position: relative;">
            <div style="position: absolute; top: -12px; left: 50%; transform: translateX(-50%); background: var(--accent-purple); color: #fff; font-size: 10px; font-weight: 700; padding: 2px 10px; border-radius: 12px;">MASTER NODE</div>
            <div style="text-align: center; margin-bottom: 12px;">
              <div style="width: 48px; height: 48px; border-radius: 50%; background: rgba(167,139,250,0.1); display: flex; align-items: center; justify-content: center; margin: 0 auto 12px;">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--accent-purple)" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>
              </div>
              <div style="font-size: var(--text-lg); font-weight: 700;">{m.name}</div>
              <div style="font-size: 11px; color: var(--text-muted); margin-top: 4px;">{m.status}</div>
            </div>
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px; border-top: 1px solid var(--border-primary); padding-top: 12px;">
              <div style="text-align: center;">
                <div style="font-size: 10px; color: var(--text-muted);">CPU</div>
                <div style="font-size: var(--text-sm); font-weight: 600; font-family: var(--font-mono);">{m.cpus} Core</div>
              </div>
              <div style="text-align: center;">
                <div style="font-size: 10px; color: var(--text-muted);">RAM</div>
                <div style="font-size: var(--text-sm); font-weight: 600; font-family: var(--font-mono);">{m.memory / 1073741824} GB</div>
              </div>
            </div>
          </div>
        {/each}
      </div>

      {#if workers.length > 0}
        <div style="width: 2px; height: 40px; background: var(--border-primary); margin: -20px 0;"></div>
        <div style="display: flex; flex-wrap: wrap; gap: 24px; justify-content: center;">
          {#each workers as w}
            <div style="background: var(--bg-primary); border: 2px solid var(--accent-blue); border-radius: 12px; padding: 20px; width: 220px; box-shadow: 0 4px 20px rgba(0,0,0,0.2); position: relative;">
              <div style="position: absolute; top: -12px; left: 50%; transform: translateX(-50%); background: var(--accent-blue); color: #fff; font-size: 10px; font-weight: 700; padding: 2px 10px; border-radius: 12px;">WORKER NODE</div>
              <div style="text-align: center; margin-bottom: 12px;">
                <div style="width: 40px; height: 40px; border-radius: 50%; background: rgba(56, 189, 248, 0.1); display: flex; align-items: center; justify-content: center; margin: 0 auto 12px;">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
                </div>
                <div style="font-size: var(--text-md); font-weight: 700;">{w.name}</div>
                <div style="font-size: 11px; color: var(--text-muted); margin-top: 4px;">{w.status}</div>
              </div>
              <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 8px; border-top: 1px solid var(--border-primary); padding-top: 12px;">
                <div style="text-align: center;">
                  <div style="font-size: 10px; color: var(--text-muted);">CPU</div>
                  <div style="font-size: var(--text-sm); font-weight: 600; font-family: var(--font-mono);">{w.cpus} Core</div>
                </div>
                <div style="text-align: center;">
                  <div style="font-size: 10px; color: var(--text-muted);">RAM</div>
                  <div style="font-size: var(--text-sm); font-weight: 600; font-family: var(--font-mono);">{w.memory / 1073741824} GB</div>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}

    </div>
  {/if}
</div>

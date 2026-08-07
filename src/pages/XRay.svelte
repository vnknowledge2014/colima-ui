<script lang="ts">
  import { onMount } from "svelte";
  import { k8sApi } from "../lib/api";
  import { t } from "../lib/i18n.svelte";

  let { namespace = "all" }: { namespace?: string } = $props();

  let loading = $state(true);
  let error = $state<string | null>(null);

  // Grouped resources
  let ingresses = $state<any[]>([]);
  let services = $state<any[]>([]);
  let deployments = $state<any[]>([]);
  let pods = $state<any[]>([]);

  // Selection for highlighting relationships
  let hoveredId = $state<string | null>(null);

  onMount(() => {
    fetchData();
  });

  async function fetchData() {
    loading = true;
    error = null;
    try {
      // fetch all basic resources and ingress
      const rawAll = await k8sApi.resources("all", namespace);
      const rawIngress = await k8sApi.resources("ingress", namespace);

      let items: any[] = [];
      try {
        const parsedAll = JSON.parse(rawAll);
        if (parsedAll && parsedAll.items) {
          items = items.concat(parsedAll.items);
        }
      } catch (e) {
        console.warn("Failed to parse all resources", e);
      }

      try {
        const parsedIngress = JSON.parse(rawIngress);
        if (parsedIngress && parsedIngress.items) {
          items = items.concat(parsedIngress.items);
        }
      } catch (e) {
        console.warn("Failed to parse ingress resources", e);
      }

      // Group them
      ingresses = items.filter(i => i.kind === "Ingress");
      services = items.filter(i => i.kind === "Service" && i.metadata.name !== "kubernetes");
      deployments = items.filter(i => i.kind === "Deployment" || i.kind === "StatefulSet" || i.kind === "DaemonSet");
      pods = items.filter(i => i.kind === "Pod");

    } catch (e: any) {
      error = e.message || String(e);
    } finally {
      loading = false;
    }
  }

  // Helper to determine if two resources are related
  function isRelated(type: string, item: any, currentHover: string | null): boolean {
    if (!currentHover) return false;
    const hoverParts = currentHover.split(":");
    const hType = hoverParts[0];
    const hName = hoverParts[1];
    const hNs = hoverParts[2];

    if (item.metadata.namespace !== hNs) return false;
    if (type === hType && item.metadata.name === hName) return true;

    // Ingress -> Service
    if (type === "ingress" && hType === "service") {
      return item.spec?.rules?.some((r: any) => 
        r.http?.paths?.some((p: any) => p.backend?.service?.name === hName)
      );
    }
    if (type === "service" && hType === "ingress") {
      const hItem = ingresses.find(i => i.metadata.name === hName && i.metadata.namespace === hNs);
      return hItem?.spec?.rules?.some((r: any) => 
        r.http?.paths?.some((p: any) => p.backend?.service?.name === item.metadata.name)
      );
    }

    // Service -> Pod
    if (type === "service" && hType === "pod") {
      const hItem = pods.find(p => p.metadata.name === hName && p.metadata.namespace === hNs);
      const selector = item.spec?.selector;
      if (!selector || !hItem?.metadata?.labels) return false;
      return Object.keys(selector).every(k => hItem.metadata.labels[k] === selector[k]);
    }
    if (type === "pod" && hType === "service") {
      const hItem = services.find(s => s.metadata.name === hName && s.metadata.namespace === hNs);
      const selector = hItem?.spec?.selector;
      if (!selector || !item.metadata?.labels) return false;
      return Object.keys(selector).every(k => item.metadata.labels[k] === selector[k]);
    }

    // Deployment -> Pod
    if (type === "deployment" && hType === "pod") {
      const hItem = pods.find(p => p.metadata.name === hName && p.metadata.namespace === hNs);
      // Simplify: check if pod name starts with deployment name
      return hItem?.metadata?.name.startsWith(item.metadata.name + "-");
    }
    if (type === "pod" && hType === "deployment") {
      // Simplify: check if pod name starts with deployment name
      return item.metadata?.name.startsWith(hName + "-");
    }

    return false;
  }

</script>

<div class="xray-container">
  <div class="header">
    <h3>{t('xray.title', { default: 'X-Ray Analysis' })}</h3>
    <p>{t('xray.subtitle', { default: 'Visualizing resource dependencies in namespace' })} <code>{namespace}</code>. {t('xray.hover_hint', { default: 'Hover over a card to see connections.' })}</p>
    <button class="btn btn-secondary" onclick={fetchData}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.59-8.36l5.67-5.67"/></svg>
      {t('xray.refresh', { default: 'Refresh' })}
    </button>
  </div>

  {#if loading}
    <div class="center-content">
      <div class="spinner"></div>
    </div>
  {:else if error}
    <div class="error-msg">
      {error}
    </div>
  {:else}
    <div class="grid">
      <!-- Ingress Column -->
      <div class="column">
        <h4>{t('xray.ingress', { default: 'Ingress' })} ({ingresses.length})</h4>
        {#each ingresses as item}
          {@const isActive = hoveredId === `ingress:${item.metadata.name}:${item.metadata.namespace}`}
          {@const isHighlighted = isRelated("ingress", item, hoveredId)}
          <div class="node-card" 
               class:active={isActive}
               class:highlight={isHighlighted && !isActive}
               class:dimmed={hoveredId && !isActive && !isHighlighted}
               onmouseenter={() => hoveredId = `ingress:${item.metadata.name}:${item.metadata.namespace}`}
               onmouseleave={() => hoveredId = null}>
            <div class="title">{item.metadata.name}</div>
            <div class="subtitle">{item.metadata.namespace}</div>
          </div>
        {/each}
        {#if ingresses.length === 0}
          <div class="empty">{t('xray.no_ingress', { default: 'No ingresses found' })}</div>
        {/if}
      </div>

      <!-- Services Column -->
      <div class="column">
        <h4>{t('xray.services', { default: 'Services' })} ({services.length})</h4>
        {#each services as item}
          {@const isActive = hoveredId === `service:${item.metadata.name}:${item.metadata.namespace}`}
          {@const isHighlighted = isRelated("service", item, hoveredId)}
          <div class="node-card" 
               class:active={isActive}
               class:highlight={isHighlighted && !isActive}
               class:dimmed={hoveredId && !isActive && !isHighlighted}
               onmouseenter={() => hoveredId = `service:${item.metadata.name}:${item.metadata.namespace}`}
               onmouseleave={() => hoveredId = null}>
            <div class="title">{item.metadata.name}</div>
            <div class="subtitle">{item.spec?.type} • {item.spec?.clusterIP}</div>
          </div>
        {/each}
        {#if services.length === 0}
          <div class="empty">{t('xray.no_services', { default: 'No services found' })}</div>
        {/if}
      </div>

      <!-- Deployments Column -->
      <div class="column">
        <h4>{t('xray.workloads', { default: 'Workloads' })} ({deployments.length})</h4>
        {#each deployments as item}
          {@const isActive = hoveredId === `deployment:${item.metadata.name}:${item.metadata.namespace}`}
          {@const isHighlighted = isRelated("deployment", item, hoveredId)}
          <div class="node-card" 
               class:active={isActive}
               class:highlight={isHighlighted && !isActive}
               class:dimmed={hoveredId && !isActive && !isHighlighted}
               onmouseenter={() => hoveredId = `deployment:${item.metadata.name}:${item.metadata.namespace}`}
               onmouseleave={() => hoveredId = null}>
            <div class="title">{item.metadata.name}</div>
            <div class="subtitle">{item.kind}</div>
          </div>
        {/each}
        {#if deployments.length === 0}
          <div class="empty">{t('xray.no_workloads', { default: 'No workloads found' })}</div>
        {/if}
      </div>

      <!-- Pods Column -->
      <div class="column">
        <h4>{t('xray.pods', { default: 'Pods' })} ({pods.length})</h4>
        {#each pods as item}
          {@const isActive = hoveredId === `pod:${item.metadata.name}:${item.metadata.namespace}`}
          {@const isHighlighted = isRelated("pod", item, hoveredId)}
          <div class="node-card" 
               class:active={isActive}
               class:highlight={isHighlighted && !isActive}
               class:dimmed={hoveredId && !isActive && !isHighlighted}
               onmouseenter={() => hoveredId = `pod:${item.metadata.name}:${item.metadata.namespace}`}
               onmouseleave={() => hoveredId = null}>
            <div class="title">{item.metadata.name}</div>
            <div class="subtitle status" class:running={item.status?.phase === 'Running'}>
              {item.status?.phase}
            </div>
          </div>
        {/each}
        {#if pods.length === 0}
          <div class="empty">{t('xray.no_pods', { default: 'No pods found' })}</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .xray-container {
    padding: 20px;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .header {
    display: flex;
    align-items: center;
    margin-bottom: 24px;
    gap: 16px;
  }
  .header h3 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
  }
  .header p {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--text-sm);
    flex: 1;
  }
  .btn {
    padding: 6px 12px;
    border-radius: 6px;
    font-size: var(--text-sm);
    font-weight: 500;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .btn-secondary {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    color: var(--text-primary);
  }
  .center-content {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border-primary);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  .error-msg {
    padding: 16px;
    background: rgba(248, 81, 73, 0.1);
    color: var(--accent-red);
    border: 1px solid rgba(248, 81, 73, 0.2);
    border-radius: 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 24px;
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    padding-bottom: 20px;
  }
  .column {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .column h4 {
    margin: 0 0 8px 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .empty {
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-style: italic;
    padding: 12px;
    background: var(--bg-secondary);
    border-radius: 8px;
    text-align: center;
  }
  .node-card {
    background: var(--bg-content);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    padding: 12px;
    transition: all 0.2s;
    cursor: default;
    position: relative;
  }
  .node-card.active {
    border-color: var(--accent-blue);
    background: rgba(88, 166, 255, 0.1);
    box-shadow: 0 0 0 1px var(--accent-blue);
  }
  .node-card.highlight {
    border-color: var(--accent-green);
    background: rgba(63, 185, 80, 0.05);
  }
  .node-card.dimmed {
    opacity: 0.3;
    filter: grayscale(100%);
  }
  .title {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-primary);
    word-break: break-all;
    margin-bottom: 4px;
  }
  .subtitle {
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .status.running {
    color: var(--accent-green);
  }
</style>

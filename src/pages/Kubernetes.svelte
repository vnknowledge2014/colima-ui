<script lang="ts">
  import { onMount } from "svelte";
  import { k8sApi } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import ContextMenu from "../components/ContextMenu.svelte";
  import KubernetesHealth from "../components/KubernetesHealth.svelte";
  import { confirm } from "../store/confirm.svelte";
  import { k8sState, type K8sResource } from "../store/k8s.svelte";
  import { parseItems, timeAgo, statusColor, getColumns } from "../lib/k8sUtils";
  import XRay from "./XRay.svelte";
  import ClusterTopology from "./ClusterTopology.svelte";

  // Use the global state
  let connected = $derived(k8sState.connected);
  let loading = $derived(k8sState.loading);
  let dataLoading = $derived(k8sState.dataLoading);
  let namespaces = $derived(k8sState.namespaces);
  let items = $derived(k8sState.items);
  let activeResource = $derived(k8sState.activeResource);
  let contexts = $derived(k8sState.contexts);
  let currentCtx = $derived(k8sState.currentCtx);

  // Local UI state
  let selectedItem = $state<K8sResource | null>(null);
  let detailTab = $state<"describe" | "yaml" | "logs">("describe");
  let detailText = $state("");
  let yamlText = $state("");
  let yamlEdited = $state("");
  let logsText = $state("");
  let filter = $state("");
  let followLogs = $state(false);
  let containers = $state<string[]>([]);
  let selectedContainer = $state("");
  let portForwardModal = $state<K8sResource | null>(null);
  let pfLocalPort = $state("");
  let pfRemotePort = $state("");
  let activeForwards = $state<string[]>([]);
  let applying = $state(false);
  let scaleValue = $state<number | null>(null);
  let kubectlMissing = $state(false);
  let crdTypes = $state<{ id: string; label: string; resource: string; group: string }[]>([]);
  let benchModal = $state<K8sResource | null>(null);
  let benchUrl = $state("");
  let benchConc = $state(5);
  let benchReqs = $state(50);
  let benchMethod = $state("GET");
  let benchRunning = $state(false);
  let benchResult = $state<any>(null);
  let ctxMenu = $state<{ x: number; y: number; item: K8sResource } | null>(null);
  
  let eventSource: EventSource | null = null;
  let searchInput: HTMLInputElement | null = $state(null);
  let timeoutId: any = null;

  const RESOURCE_GROUPS = [
    {
      label: "Workloads",
      items: [
        { id: "topology", label: "Cluster Topology", resource: "topology" },
        { id: "xray", label: "X-Ray", resource: "xray" },
        { id: "pods", label: "Pods", resource: "pods" },
        { id: "deployments", label: "Deployments", resource: "deployments", canRestart: true },
        { id: "statefulsets", label: "StatefulSets", resource: "statefulsets", canRestart: true },
        { id: "daemonsets", label: "DaemonSets", resource: "daemonsets", canRestart: true },
        { id: "replicasets", label: "ReplicaSets", resource: "replicasets" },
        { id: "jobs", label: "Jobs", resource: "jobs" },
        { id: "cronjobs", label: "CronJobs", resource: "cronjobs" },
      ],
    },
    {
      label: "Networking",
      items: [
        { id: "services", label: "Services", resource: "services" },
        { id: "ingresses", label: "Ingresses", resource: "ingresses" },
      ],
    },
    {
      label: "Config",
      items: [
        { id: "configmaps", label: "ConfigMaps", resource: "configmaps" },
        { id: "secrets", label: "Secrets", resource: "secrets" },
      ],
    },
    {
      label: "Storage",
      items: [
        { id: "pv", label: "PV", resource: "persistentvolumes" },
        { id: "pvc", label: "PVC", resource: "persistentvolumeclaims" },
      ],
    },
    {
      label: "Cluster",
      items: [
        { id: "nodes", label: "Nodes", resource: "nodes" },
        { id: "events", label: "Events", resource: "events" },
        { id: "namespaces", label: "Namespaces", resource: "namespaces" },
        { id: "health", label: "Health", resource: "" },
      ],
    },
  ];

  const ALL_ITEMS = RESOURCE_GROUPS.flatMap(g => g.items);

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      searchInput?.focus();
    }
    if (e.key === "Escape") {
      selectedItem = null;
      ctxMenu = null;
      portForwardModal = null;
      benchModal = null;
    }
  }

  onMount(() => {
    document.addEventListener("keydown", handleKeydown);
    checkCluster();
    return () => {
      document.removeEventListener("keydown", handleKeydown);
      if (timeoutId) clearTimeout(timeoutId);
      if (eventSource) {
        eventSource.close();
        eventSource = null;
      }
    };
  });

  async function checkCluster() {
    try {
      const ctxRaw = await k8sApi.contexts();
      const ctxList = ctxRaw.trim().split("\n").filter(Boolean);
      if (ctxList.length > 0) k8sState.contexts = ctxList;
      const cur = await k8sApi.currentContext();
      k8sState.currentCtx = cur.trim();
      kubectlMissing = false;
    } catch (e) {
      if (String(e).includes("not installed")) {
        kubectlMissing = true;
        k8sState.connected = false;
        k8sState.loading = false;
        return;
      }
    }

    try {
      await k8sApi.check();
      k8sState.connected = true;
      const nsRaw = await k8sApi.namespaces();
      let nsList = [];
      if (Array.isArray(nsRaw)) {
        nsList = nsRaw.map((ns: any) => ({ name: ns.name || "" })).filter(ns => ns.name);
      } else {
        const parsed = parseItems(nsRaw);
        nsList = parsed.map(n => ({ name: n.name })).filter(ns => ns.name);
      }
      k8sState.namespaces = nsList;

      try {
        const fwds = await k8sApi.portForwardList();
        activeForwards = fwds.split("\n").filter(Boolean);
      } catch {}

      try {
        const crdRaw = await k8sApi.crds();
        const parsed = typeof crdRaw === "string" ? JSON.parse(crdRaw) : crdRaw;
        const crds = (parsed.items || []).map((crd: any) => {
          const name = crd.metadata?.name || "";
          const kind = crd.spec?.names?.kind || name;
          const group = crd.spec?.group || "";
          return { id: `crd:${name}`, label: kind, resource: name, group };
        }).slice(0, 30);
        crdTypes = crds;
      } catch {}
    } catch {
      k8sState.connected = false;
    }
    k8sState.loading = false;
  }

  async function fetchData() {
    if (!k8sState.connected) return;
    k8sState.dataLoading = true;
    try {
      if (k8sState.activeResource.startsWith("crd:")) {
        const crdName = k8sState.activeResource.slice(4);
        const raw = await k8sApi.crdResources(crdName, k8sState.namespace);
        k8sState.items = parseItems(raw);
        return;
      }
      const info = ALL_ITEMS.find(i => i.id === k8sState.activeResource);
      if (!info || k8sState.activeResource === "health") return;
      
      let raw: string;
      if (k8sState.activeResource === "nodes") raw = await k8sApi.nodesJson();
      else if (k8sState.activeResource === "events") raw = await k8sApi.eventsJson(k8sState.namespace);
      else raw = await k8sApi.resources(info.resource, k8sState.namespace);
      
      k8sState.items = parseItems(raw);
    } catch (e) {
      globalToast("error", `Failed to load ${k8sState.activeResource}: ${e}`);
      k8sState.items = [];
    } finally {
      k8sState.dataLoading = false;
    }
  }

  // React to changes
  $effect(() => {
    if (k8sState.connected && k8sState.activeResource && k8sState.namespace) {
      fetchData();
    }
  });

  async function openDetail(item: K8sResource) {
    selectedItem = item;
    detailTab = activeResource === "pods" ? "logs" : "describe";
    containers = [];
    selectedContainer = "";
    try {
      const info = ALL_ITEMS.find(i => i.id === activeResource);
      const rt = info?.resource || activeResource;
      const singularRt = rt.endsWith("ses") ? rt.slice(0, -2) : rt.endsWith("s") ? rt.slice(0, -1) : rt;
      
      const [desc, yaml] = await Promise.all([
        k8sApi.describe(item.namespace || "default", singularRt, item.name),
        k8sApi.yaml(singularRt, item.namespace || "default", item.name),
      ]);
      
      detailText = desc;
      yamlText = yaml;
      yamlEdited = yaml;
      
      if (activeResource === "pods") {
        const [logs, cont] = await Promise.all([
          k8sApi.podLogs(item.namespace || "default", item.name, 200),
          k8sApi.podContainers(item.namespace || "default", item.name).catch(() => ""),
        ]);
        logsText = logs;
        const containerList = cont.trim().split(/\s+/).filter(Boolean);
        containers = containerList;
        if (containerList.length > 0) selectedContainer = containerList[0];
      } else {
        logsText = "";
      }
    } catch (e) {
      detailText = `Error: ${e}`;
      yamlText = "";
      yamlEdited = "";
      logsText = "";
    }
  }

  async function fetchContainerLogs(container: string) {
    if (!selectedItem) return;
    selectedContainer = container;
    try {
      logsText = await k8sApi.containerLogs(selectedItem.namespace || "default", selectedItem.name, container, 200);
    } catch (e) {
      logsText = `Error: ${e}`;
    }
  }

  async function handleApply() {
    if (!selectedItem) return;
    applying = true;
    try {
      const result = await k8sApi.apply(yamlEdited, selectedItem.namespace);
      globalToast("success", result || "Applied successfully");
      yamlText = yamlEdited;
      timeoutId = setTimeout(fetchData, 1000);
    } catch (e) {
      globalToast("error", `Apply failed: ${e}`);
    } finally {
      applying = false;
    }
  }

  async function handleDelete(item: K8sResource) {
    const info = ALL_ITEMS.find(i => i.id === activeResource);
    const rt = info?.resource || activeResource;
    const singularRt = rt.replace(/s$/, "");
    
    if (await confirm({ title: `Delete ${singularRt}`, message: `Delete ${item.name} from ${item.namespace || "cluster"}?`, variant: "danger", confirmText: "Delete" })) {
      try {
        if (activeResource === "pods") await k8sApi.deletePod(item.namespace || "default", item.name);
        else await k8sApi.deleteResource(singularRt, item.namespace || "default", item.name);
        globalToast("success", `${item.name} deleted`);
        fetchData();
      } catch (e) { globalToast("error", String(e)); }
    }
  }

  async function handleRestart(item: K8sResource) {
    const info = ALL_ITEMS.find(i => i.id === activeResource);
    if (!info?.canRestart) return;
    if (await confirm({ title: `Restart ${info.label.replace(/s$/, "")}`, message: `Rollout restart ${item.name}?`, variant: "warning", confirmText: "Restart" })) {
      try {
        await k8sApi.restart(info.resource.replace(/s$/, ""), item.namespace || "default", item.name);
        globalToast("success", `${item.name} restarting`);
        timeoutId = setTimeout(fetchData, 2000);
      } catch (e) { globalToast("error", String(e)); }
    }
  }

  async function handleExec(item: K8sResource) {
    try {
      const result = await k8sApi.exec(item.namespace || "default", item.name, selectedContainer || "");
      globalToast("success", result || "Shell opened in Terminal");
    } catch (e) {
      globalToast("error", "Failed to exec: " + e);
    }
  }

  async function startPortForward() {
    if (!portForwardModal || !pfLocalPort || !pfRemotePort) return;
    const rt = activeResource === "services" ? "service" : "pod";
    try {
      const result = await k8sApi.portForwardStart(portForwardModal.namespace || "default", portForwardModal.name, parseInt(pfLocalPort), parseInt(pfRemotePort), rt);
      globalToast("success", result);
      portForwardModal = null;
      fetchPortForwards();
    } catch (e) {
      globalToast("error", "Port forward failed: " + e);
    }
  }

  async function fetchPortForwards() {
    try {
      const fwds = await k8sApi.portForwardList();
      activeForwards = fwds.split("\n").filter(Boolean);
    } catch (e) { globalToast("error", String(e)); }
  }



  async function handleScale(item: K8sResource, replicas: number) {
    if (!item) return;
    const info = ALL_ITEMS.find(t => t.id === activeResource);
    if (!info) return;
    try {
      await k8sApi.genericScale(info.resource.replace(/s$/, ""), item.namespace || "default", item.name, replicas);
      globalToast("success", `Scaled ${item.name} to ${replicas} replica(s)`);
      fetchData();
    } catch (e) { globalToast("error", String(e)); }
  }



  async function handleNodeAction(item: K8sResource, action: string) {
    const labels: Record<string, string> = { cordon: "Cordon", uncordon: "Uncordon", drain: "Drain" };
    if (await confirm({ title: `${labels[action]} Node`, message: `${labels[action]} node ${item.name}?${action === "drain" ? " This will evict all pods." : ""}`, variant: action === "drain" ? "danger" : "warning", confirmText: labels[action] })) {
      try {
        await k8sApi.nodeAction(item.name, action);
        globalToast("success", `Node ${item.name} ${action}ed`);
        timeoutId = setTimeout(fetchData, 1000);
      } catch (e) { globalToast("error", String(e)); }
    }
  }

  async function handleContextSwitch(ctx: string) {
    try {
      await k8sApi.setContext(ctx);
      k8sState.currentCtx = ctx;
      globalToast("success", `Switched to ${ctx}`);
      k8sState.loading = true;
      timeoutId = setTimeout(checkCluster, 500);
    } catch (e) { globalToast("error", `Context switch failed: ${e}`); }
  }

  function getCtxItems(item: K8sResource) {
    const result: any[] = [];
    if (activeResource === "pods") {
      result.push({ label: "View Logs", action: async () => { await openDetail(item); detailTab = "logs"; } });
      result.push({ label: "Exec Shell", action: async () => await handleExec(item) });
    }
    const activeInfo = ALL_ITEMS.find(t => t.id === activeResource);
    if (activeInfo?.canRestart) {
      result.push({ label: "Restart", action: async () => await handleRestart(item) });
    }
    if (activeResource === "services") {
      result.push({ label: "⚡ Benchmark", action: async () => {
        benchModal = item;
        const portStr = (item as any).port?.split("/")[0]?.split(",")[0] || (item as any).ports?.split("/")[0]?.split(",")[0] || "80";
        benchUrl = `http://localhost:${portStr}`;
      }});
    }
    result.push({ divider: true, label: "", action: async () => {} });
    result.push({ label: "Copy Name", action: async () => { await navigator.clipboard.writeText(item.name); globalToast("success", "Name copied"); } });
    result.push({ divider: true, label: "", action: async () => {} });
    result.push({ label: "Delete", danger: true, action: async () => await handleDelete(item) });
    return result;
  }

  let activeInfo = $derived(ALL_ITEMS.find(i => i.id === activeResource) || crdTypes.find(c => c.id === activeResource));
  let columns = $derived(getColumns(activeResource));
  let filtered = $derived(filter ? items.filter(i => i.name.toLowerCase().includes(filter.toLowerCase()) || i.namespace?.toLowerCase().includes(filter.toLowerCase())) : items);
</script>

{#if loading}
  <div class="content-header" data-tauri-drag-region><h1>Kubernetes</h1></div>
  <div class="loading-screen"><div class="spinner"></div><span>Connecting to cluster...</span></div>
{:else if !connected}
  <div class="content-header" data-tauri-drag-region>
    <h1>Kubernetes</h1>
    {#if !kubectlMissing && contexts.length > 1}
      <div class="content-header-actions" style="display: flex; gap: 8px; align-items: center;">
        <select bind:value={k8sState.currentCtx} onchange={() => handleContextSwitch(k8sState.currentCtx)} class="input select" style="color: var(--accent-purple); font-family: var(--font-mono); min-width: 150px;">
          {#each contexts as c}
            <option value={c}>{c}</option>
          {/each}
        </select>
      </div>
    {/if}
  </div>
  <div class="content-body">
    <div class="empty-state">
      <div class="empty-state-icon">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke={kubectlMissing ? "var(--accent-yellow)" : "var(--accent-red)"} stroke-width="1.5">
          {#if kubectlMissing}
            <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
          {:else}
            <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
          {/if}
        </svg>
      </div>
      <div class="empty-state-title">{kubectlMissing ? "kubectl Not Installed" : "Cluster Not Connected"}</div>
      <div class="empty-state-text">
        {#if kubectlMissing}
          <code style="display: block; margin-bottom: 12px; padding: 8px 12px; background: var(--bg-primary); border-radius: 6px; font-family: var(--font-mono); font-size: var(--text-sm);">brew install kubectl</code>
          Install kubectl to manage Kubernetes clusters.
        {:else}
          {#if currentCtx}
            <span style="display: block; margin-bottom: 8px; font-family: var(--font-mono); color: var(--accent-purple); font-size: var(--text-xs);">{t('kubernetes.context', { default: 'Context' })}: {currentCtx}</span>
          {/if}
          {t('kubernetes.enable_text', { default: 'Enable Kubernetes in the Instances tab, or switch to a different context above.' })}
        {/if}
      </div>
      <button class="btn btn-primary" onclick={() => { k8sState.loading = true; checkCluster(); }}>{t('kubernetes.retry', { default: 'Retry Connection' })}</button>
    </div>
  </div>
{:else}
  <div class="content-header" data-tauri-drag-region>
    <h1>
      {t('kubernetes.title', { default: 'Kubernetes' })}
      <span style="font-size: var(--text-sm); color: var(--accent-green); font-weight: 400; margin-left: 12px;">
        <svg width="8" height="8" viewBox="0 0 24 24" fill="var(--accent-green)" style="display: inline-block; vertical-align: middle; margin-right: 4px;"><circle cx="12" cy="12" r="10"/></svg> {t('kubernetes.connected', { default: 'Connected' })}
      </span>
    </h1>
    <div class="content-header-actions" style="display: flex; gap: 8px; align-items: center;">
      {#if activeForwards.length > 0}
        <div style="display: flex; align-items: center; gap: 4px; padding: 3px 8px; background: rgba(188,140,255,0.1); border: 1px solid rgba(188,140,255,0.3); border-radius: 6px; font-size: var(--text-xs); color: var(--accent-purple);">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M12 5l7 7-7 7"/></svg> {activeForwards.length} forward{activeForwards.length > 1 ? "s" : ""}
        </div>
      {/if}
      {#if contexts.length > 1}
        <select bind:value={k8sState.currentCtx} onchange={() => handleContextSwitch(k8sState.currentCtx)} class="input select" style="color: var(--accent-purple); font-family: var(--font-mono); min-width: 150px;">
          {#each contexts as c}
            <option value={c}>{c}</option>
          {/each}
        </select>
      {/if}
      <select bind:value={k8sState.namespace} class="input select" style="font-family: var(--font-mono); min-width: 150px;">
        <option value="all">All Namespaces</option>
        {#each namespaces as ns}
          <option value={ns.name}>{ns.name}</option>
        {/each}
      </select>
      <button class="btn btn-ghost" onclick={fetchData} aria-label="Refresh Data" title="Refresh Data">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
      </button>
    </div>
  </div>

  <div class="content-body" style="display: flex; gap: 0;">
    <!-- Resource sidebar -->
    <div style="width: 180px; min-width: 180px; border-right: 1px solid var(--border-primary); padding-right: 12px; margin-right: 16px; overflow-y: auto;">
      {#each RESOURCE_GROUPS as group}
        <div style="margin-bottom: 12px;">
          <div style="font-size: var(--text-xs); color: var(--text-muted); font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 4px; padding: 0 8px;">{group.label}</div>
          {#each group.items as item}
            <button onclick={() => { k8sState.activeResource = item.id; k8sState.items = []; filter = ""; }} style="display: block; width: 100%; text-align: left; padding: 5px 8px; background: {activeResource === item.id ? 'rgba(88,166,255,0.1)' : 'transparent'}; border: none; border-radius: 6px; cursor: pointer; font-size: var(--text-sm); color: {activeResource === item.id ? 'var(--accent-blue)' : 'var(--text-secondary)'}; font-weight: {activeResource === item.id ? 600 : 400}; transition: all 150ms;">
              {item.label}
            </button>
          {/each}
        </div>
      {/each}
      {#if crdTypes.length > 0}
        <div style="margin-bottom: 12px;">
          <div style="font-size: var(--text-xs); color: var(--text-muted); font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 4px; padding: 0 8px;">Custom Resources ({crdTypes.length})</div>
          {#each crdTypes as crd}
            <button onclick={() => { k8sState.activeResource = crd.id; k8sState.items = []; filter = ""; }} title="{crd.label} ({crd.group})" style="display: block; width: 100%; text-align: left; padding: 5px 8px; background: {activeResource === crd.id ? 'rgba(88,166,255,0.1)' : 'transparent'}; border: none; border-radius: 6px; cursor: pointer; font-size: var(--text-sm); color: {activeResource === crd.id ? 'var(--accent-blue)' : 'var(--text-secondary)'}; font-weight: {activeResource === crd.id ? 600 : 400}; transition: all 150ms; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
              {crd.label}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Main content -->
    <div style="flex: 1; min-width: 0;">
      <!-- Main view goes here -->
      {#if activeResource === "health"}
        <KubernetesHealth />
      {:else}
        <!-- Filter bar -->
        <div style="display: flex; gap: 8px; margin-bottom: 12px; align-items: center;">
          <input bind:this={searchInput} type="text" bind:value={filter} placeholder="Filter {activeInfo?.label || ''}..." class="input" style="flex: 1;" />
          <span style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono);">
            {filtered.length} {activeInfo?.label || "items"}
          </span>
        </div>

        {#if dataLoading}
          <div style="display: flex; justify-content: center; padding: 40px;">
            <div class="spinner"></div>
          </div>
        {:else if activeResource === 'xray'}
          <XRay namespace={k8sState.namespace || 'all'} />
        {:else if activeResource === 'topology'}
          <ClusterTopology />
        {:else if filtered.length > 0}
          <div class="card" style="overflow: auto;">
            <div style="display: grid; grid-template-columns: {columns.map(col => col.key === 'name' ? 'minmax(120px, 2fr)' : col.key === 'namespace' || col.key === 'node' ? 'minmax(80px, 1fr)' : 'auto').join(' ')} auto; min-width: 100%;">
              <!-- Header row -->
              {#each columns as col}
                <div style="text-align: left; padding: 10px 16px; font-size: var(--text-xs); font-weight: 600; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; background: var(--bg-content); border-bottom: 1px solid var(--border-primary); position: sticky; top: 0; z-index: 1;">
                  {col.label}
                </div>
              {/each}
              <div style="text-align: left; padding: 10px 16px; font-size: var(--text-xs); font-weight: 600; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; background: var(--bg-content); border-bottom: 1px solid var(--border-primary); position: sticky; top: 0; z-index: 1;">
                Actions
              </div>

              <!-- Data rows -->
              {#each filtered as item (item.namespace + '/' + item.name)}
                {#each columns as col}
                  {@const val = (col.key === 'age' || (col.key === 'lastSchedule' && item[col.key] !== 'Never')) ? timeAgo(item[col.key] || '') : String(item[col.key as keyof K8sResource] || '')}
                  {@const color = (col.key === 'status' || col.key === 'type' || col.key === 'restarts' || col.key === 'replicas') ? statusColor(val) : ''}
                  <div role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => openDetail(item)} oncontextmenu={(e) => { e.preventDefault(); ctxMenu = { x: e.clientX, y: e.clientY, item }; }} style="padding: 12px 16px; cursor: pointer; font-family: {col.mono ? 'var(--font-mono)' : 'inherit'}; font-size: var(--text-xs); color: {col.key === 'namespace' || col.key === 'age' ? 'var(--text-muted)' : 'var(--text-primary)'}; font-weight: {col.key === 'name' ? 500 : 'inherit'}; border-bottom: 1px solid var(--border-subtle); display: flex; align-items: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                    <!-- Svelte handles the values dynamically -->
                    
                    {#if (col.key === 'status' || col.key === 'type')}
                      <svg width="8" height="8" viewBox="0 0 24 24" fill={color} style="display: inline-block; vertical-align: middle; flex-shrink: 0; margin-right: 6px;"><circle cx="12" cy="12" r="10"/></svg>
                      <span style="color: {color}">{val}</span>
                    {:else if col.key === 'svcType'}
                      <span style="padding: 2px 6px; border-radius: 4px; font-size: var(--text-xs); font-weight: 500;
                        background: {val === 'ClusterIP' ? 'rgba(88,166,255,0.1)' : val === 'NodePort' ? 'rgba(63,185,80,0.1)' : val === 'LoadBalancer' ? 'rgba(188,140,255,0.1)' : 'rgba(255,255,255,0.05)'};
                        color: {val === 'ClusterIP' ? 'var(--accent-blue)' : val === 'NodePort' ? 'var(--accent-green)' : val === 'LoadBalancer' ? 'var(--accent-purple)' : 'var(--text-secondary)'};
                      ">{val}</span>
                    {:else if col.key === 'restarts' || col.key === 'replicas'}
                       <!-- We pass custom colors from getColumns logic manually here as we mapped it to statusColor for simplicity -->
                       <span style="color: {val.startsWith('0/') || parseInt(val) > 0 ? 'var(--accent-yellow)' : 'inherit'}">{val}</span>
                    {:else}
                      {#if col.key === 'name' && item.warnings?.length}
                        <span title={item.warnings.join('\n')} style="margin-right: 6px; cursor: help;">⚠️</span>
                      {/if}
                      <span>{val}</span>
                    {/if}
                  </div>
                {/each}

                <!-- Actions cell -->
                <div style="padding: 12px 16px; border-bottom: 1px solid var(--border-subtle); display: flex; gap: 4px; align-items: center;">
                  {#if activeResource === "pods"}
                    <button class="btn btn-ghost" title="Exec Shell" style="padding: 2px 6px; color: var(--accent-green);" onclick={() => handleExec(item)}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
                    </button>
                    <button class="btn btn-ghost" title="Port Forward" style="padding: 2px 6px; color: var(--accent-purple);" onclick={() => { portForwardModal = item; pfLocalPort = ""; pfRemotePort = ""; }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M12 5l7 7-7 7"/></svg>
                    </button>
                  {/if}
                  {#if activeResource === "services"}
                    <button class="btn btn-ghost" title="Port Forward" style="padding: 2px 6px; color: var(--accent-purple);" onclick={() => { portForwardModal = item; const p = item._ports?.[0]?.port; pfRemotePort = p ? String(p) : ""; pfLocalPort = p ? String(p) : ""; }}>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M12 5l7 7-7 7"/></svg>
                    </button>
                  {/if}
                  {#if activeResource === "nodes"}
                    <button class="btn btn-ghost" title={item.schedulable === false ? 'Uncordon' : 'Cordon'} style="padding: 2px 6px; color: var(--accent-yellow); font-size: var(--text-xs);" onclick={() => handleNodeAction(item, item.schedulable === false ? 'uncordon' : 'cordon')}>
                      {item.schedulable === false ? "⊕" : "⊘"}
                    </button>
                    <button class="btn btn-ghost" title="Drain" style="padding: 2px 6px; color: var(--accent-red); font-size: var(--text-xs);" onclick={() => handleNodeAction(item, 'drain')}>
                      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v6m0 4v10M4.93 4.93l4.24 4.24m1.66 1.66l4.24 4.24M2 12h6m4 0h10M4.93 19.07l4.24-4.24m1.66-1.66l4.24-4.24"/></svg>
                    </button>
                  {/if}
                  {#if (activeInfo as any)?.canRestart}
                    <button class="btn btn-ghost" style="font-size: var(--text-xs); color: var(--accent-yellow); padding: 2px 6px;" onclick={() => handleRestart(item)} aria-label="Restart Resource" title="Restart Resource">
                      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
                    </button>
                  {/if}
                  <button class="btn btn-ghost" style="font-size: var(--text-xs); color: var(--accent-red); padding: 2px 6px;" onclick={() => handleDelete(item)} aria-label="Delete Resource" title="Delete Resource">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {:else}
          <div class="empty-state">
            <div class="empty-state-title">No {activeInfo?.label || "Resources"}</div>
            <div class="empty-state-text">No {activeInfo?.label?.toLowerCase()} found in the selected namespace.</div>
          </div>
        {/if}
      {/if}

      {#if selectedItem}
        <div role="button" tabindex="0" style="position: fixed; inset: 0; z-index: 1000; background: rgba(0,0,0,0.5); backdrop-filter: blur(4px); display: flex; justify-content: flex-end;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => { if (e.target === e.currentTarget) selectedItem = null; }}>
          <div style="width: 800px; max-width: 90vw; background: var(--bg-primary); border-left: 1px solid var(--border-primary); display: flex; flex-direction: column; animation: slideIn 0.2s ease-out; box-shadow: -10px 0 30px rgba(0,0,0,0.5);">
            <div style="padding: 16px 20px; border-bottom: 1px solid var(--border-primary); display: flex; justify-content: space-between; align-items: center; background: var(--bg-secondary);">
              <div>
                <div style="display: flex; gap: 8px; align-items: center; margin-bottom: 4px;">
                  <span style="padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600; background: rgba(88,166,255,0.1); color: var(--accent-blue); text-transform: uppercase;">{activeInfo?.label}</span>
                  {#if selectedItem.namespace}
                    <span style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono);">{selectedItem.namespace}</span>
                  {/if}
                </div>
                <h3 style="margin: 0; font-size: var(--text-lg); font-weight: 600; color: var(--text-primary);">{selectedItem.name}</h3>
              </div>
              <div style="display: flex; gap: 8px;">
                {#if activeResource === "deployments" || activeResource === "statefulsets" || activeResource === "replicasets"}
                  <div style="display: flex; align-items: center; gap: 8px; border-right: 1px solid var(--border-subtle); padding-right: 16px; margin-right: 8px;">
                    <span style="font-size: var(--text-xs); color: var(--text-muted);">Replicas:</span>
                    <input type="number" min="0" value={scaleValue !== null ? scaleValue : parseInt(String(selectedItem.replicas || "1").split("/")[0]) || 1} onchange={(e) => handleScale(selectedItem!, parseInt(e.currentTarget.value))} style="width: 60px; padding: 4px 8px; background: var(--bg-content); border: 1px solid var(--border-primary); border-radius: 4px; color: var(--text-primary); font-size: var(--text-sm);" />
                  </div>
                {/if}
                {#if activeResource === "pods"}
                  <button class="btn btn-ghost" title="Exec Shell" onclick={() => handleExec(selectedItem!)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
                  </button>
                {/if}
                <button class="btn btn-ghost" onclick={() => selectedItem = null} aria-label="Close details" title="Close details">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                </button>
              </div>
            </div>


            <!-- Tabs -->
            <div style="display: flex; padding: 0 16px; border-bottom: 1px solid var(--border-primary); background: var(--bg-secondary); gap: 16px;">
              {#if activeResource === "pods"}
                <button class="tab-btn" class:active={detailTab === "logs"} onclick={() => detailTab = "logs"}>Logs</button>
              {/if}
              <button class="tab-btn" class:active={detailTab === "describe"} onclick={() => detailTab = "describe"}>Describe</button>
              <button class="tab-btn" class:active={detailTab === "yaml"} onclick={() => detailTab = "yaml"}>YAML</button>
            </div>

            <div style="flex: 1; overflow: hidden; display: flex; flex-direction: column; background: var(--bg-content);">
              {#if detailTab === "describe"}
                <pre style="margin: 0; padding: 16px; font-family: var(--font-mono); font-size: 13px; color: var(--text-secondary); overflow: auto; height: 100%; white-space: pre-wrap;">{detailText}</pre>
              {:else if detailTab === "yaml"}
                <div style="display: flex; flex-direction: column; height: 100%;">
                  <textarea bind:value={yamlEdited} style="flex: 1; margin: 0; padding: 16px; font-family: var(--font-mono); font-size: 13px; color: var(--text-primary); background: transparent; border: none; resize: none; outline: none; white-space: pre;" spellcheck="false"></textarea>
                  {#if yamlEdited !== yamlText}
                    <div style="padding: 12px 16px; border-top: 1px solid var(--border-primary); background: var(--bg-secondary); display: flex; justify-content: flex-end; gap: 8px;">
                      <button class="btn btn-ghost" onclick={() => yamlEdited = yamlText}>Discard</button>
                      <button class="btn btn-primary" onclick={handleApply} disabled={applying}>
                        {applying ? "Applying..." : "Apply Changes"}
                      </button>
                    </div>
                  {/if}
                </div>
              {:else if detailTab === "logs"}
                <div style="display: flex; flex-direction: column; height: 100%;">
                  <div style="padding: 8px 16px; border-bottom: 1px solid var(--border-primary); display: flex; gap: 12px; align-items: center; background: var(--bg-secondary);">
                    {#if containers.length > 1}
                      <select bind:value={selectedContainer} onchange={() => fetchContainerLogs(selectedContainer)} style="background: var(--bg-content); border: 1px solid var(--border-primary); border-radius: 4px; padding: 4px 8px; color: var(--text-primary); font-size: var(--text-xs); font-family: var(--font-mono);">
                        {#each containers as c}
                          <option value={c}>{c}</option>
                        {/each}
                      </select>
                    {/if}
                    <!-- Follow toggle is just UI for now unless we implement stream -->
                    <label style="display: flex; align-items: center; gap: 6px; font-size: var(--text-xs); color: var(--text-secondary); cursor: pointer;">
                      <input type="checkbox" class="checkbox" bind:checked={followLogs} /> Follow Logs
                    </label>
                    <div style="flex: 1;"></div>
                    <button class="btn btn-ghost" style="padding: 2px 6px; font-size: var(--text-xs);" onclick={() => fetchContainerLogs(selectedContainer)}>Refresh</button>
                  </div>
                  <pre style="margin: 0; padding: 16px; font-family: var(--font-mono); font-size: 12px; color: #a5d6ff; background: #0d1117; overflow: auto; flex: 1; white-space: pre-wrap; word-wrap: break-word;">{logsText || "No logs available."}</pre>
                </div>
              {/if}
            </div>
          </div>
        </div>
      {/if}

      {#if portForwardModal}
        <div role="button" tabindex="0" style="position: fixed; inset: 0; z-index: 2000; background: rgba(0,0,0,0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => { if (e.target === e.currentTarget) portForwardModal = null; }}>
          <div style="background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 12px; padding: 24px; width: 400px; box-shadow: 0 20px 40px rgba(0,0,0,0.5);">
            <h3 style="margin: 0 0 16px 0; font-size: var(--text-lg); color: var(--text-primary);">Port Forward: {portForwardModal.name}</h3>
            <div style="display: flex; gap: 12px; margin-bottom: 24px;">
              <div style="flex: 1;">
                <label for="pfLocalPort" style="display: block; font-size: var(--text-xs); color: var(--text-muted); margin-bottom: 6px;">Local Port</label>
                <input id="pfLocalPort" type="number" placeholder="8080" bind:value={pfLocalPort} class="input" style="font-family: var(--font-mono);" />
              </div>
              <div style="flex: 1;">
                <label for="pfRemotePort" style="display: block; font-size: var(--text-xs); color: var(--text-muted); margin-bottom: 6px;">Pod/Svc Port</label>
                <input id="pfRemotePort" type="number" placeholder="80" bind:value={pfRemotePort} class="input" style="font-family: var(--font-mono);" />
              </div>
            </div>
            <div style="display: flex; justify-content: flex-end; gap: 8px;">
              <button class="btn btn-ghost" onclick={() => portForwardModal = null}>Cancel</button>
              <button class="btn btn-primary" onclick={startPortForward}>Start Forwarding</button>
            </div>
          </div>
        </div>
      {/if}

      {#if ctxMenu}
        <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={getCtxItems(ctxMenu.item)} onClose={() => ctxMenu = null} />
      {/if}

      {#if benchModal}
        <div role="button" tabindex="0" style="position: fixed; inset: 0; z-index: 2000; background: rgba(0,0,0,0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => { if (e.target === e.currentTarget) benchModal = null; }}>
          <div style="background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 12px; padding: 24px; width: 520px; box-shadow: 0 20px 40px rgba(0,0,0,0.5);">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
              <h3 style="margin: 0; font-size: var(--text-lg); color: var(--text-primary);">⚡ HTTP Benchmark — {benchModal.name}</h3>
              <button class="btn btn-ghost" aria-label="Close Benchmark Modal" onclick={() => benchModal = null}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
            
            <div style="display: flex; flex-direction: column; gap: 12px;">
              <div>
                <label for="benchUrl" style="font-size: var(--text-xs); color: var(--text-muted);">Target URL</label>
                <input id="benchUrl" type="text" bind:value={benchUrl} class="input" style="margin-top: 4px; font-family: var(--font-mono);" placeholder="http://localhost:8080" />
              </div>
              <div style="display: flex; gap: 12px;">
                <div style="flex: 1;">
                  <label for="benchMethod" style="font-size: var(--text-xs); color: var(--text-muted);">Method</label>
                  <select id="benchMethod" bind:value={benchMethod} class="input select" style="margin-top: 4px; font-family: var(--font-mono);">
                    <option value="GET">GET</option>
                    <option value="POST">POST</option>
                    <option value="PUT">PUT</option>
                    <option value="DELETE">DELETE</option>
                  </select>
                </div>
                <div style="flex: 1;">
                  <label for="benchConc" style="font-size: var(--text-xs); color: var(--text-muted);">Concurrency</label>
                  <input id="benchConc" type="number" bind:value={benchConc} min="1" max="100" class="input" style="margin-top: 4px; font-family: var(--font-mono);" />
                </div>
                <div style="flex: 1;">
                  <label for="benchReqs" style="font-size: var(--text-xs); color: var(--text-muted);">Requests</label>
                  <input id="benchReqs" type="number" bind:value={benchReqs} min="1" max="10000" class="input" style="margin-top: 4px; font-family: var(--font-mono);" />
                </div>
              </div>
              <button class="btn btn-primary" disabled={benchRunning || !benchUrl} onclick={async () => {
                benchRunning = true; benchResult = null;
                try {
                  const raw = await k8sApi.benchmark(benchUrl, benchConc, benchReqs, benchMethod);
                  benchResult = typeof raw === "string" ? JSON.parse(raw) : raw;
                } catch (e) {
                  globalToast("error", `Benchmark failed: ${e}`);
                } finally {
                  benchRunning = false;
                }
              }}>
                {#if benchRunning}
                  <div class="spinner" style="width: 14px; height: 14px;"></div> Running...
                {:else}
                  Run Benchmark
                {/if}
              </button>
              
              {#if benchResult}
                <div class="card" style="padding: 16px;">
                  <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px; margin-bottom: 12px;">
                    <div style="text-align: center;">
                      <div style="font-size: var(--text-xl); font-weight: 700; color: var(--accent-green);">{benchResult.requests_per_sec}</div>
                      <div style="font-size: var(--text-xs); color: var(--text-muted);">req/s</div>
                    </div>
                    <div style="text-align: center;">
                      <div style="font-size: var(--text-xl); font-weight: 700; color: var(--accent-blue);">{benchResult.success}/{benchResult.total_requests}</div>
                      <div style="font-size: var(--text-xs); color: var(--text-muted);">success</div>
                    </div>
                    <div style="text-align: center;">
                      <div style="font-size: var(--text-xl); font-weight: 700; color: {benchResult.failed > 0 ? 'var(--accent-red)' : 'var(--text-secondary)'};">{benchResult.failed}</div>
                      <div style="font-size: var(--text-xs); color: var(--text-muted);">failed</div>
                    </div>
                  </div>
                  <table style="width: 100%; font-size: var(--text-xs); font-family: var(--font-mono);">
                    <thead>
                      <tr style="color: var(--text-muted); border-bottom: 1px solid var(--border-primary);">
                        <th style="text-align: left; padding: 4px;">Metric</th>
                        <th style="text-align: right; padding: 4px;">Value</th>
                      </tr>
                    </thead>
                    <tbody>
                      {#each [{ l: "Avg", v: benchResult.avg_latency_ms }, { l: "Min", v: benchResult.min_latency_ms }, { l: "Max", v: benchResult.max_latency_ms }, { l: "P50", v: benchResult.p50_ms }, { l: "P95", v: benchResult.p95_ms, c: "var(--accent-yellow)" }, { l: "P99", v: benchResult.p99_ms, c: "var(--accent-red)" }] as r}
                        <tr>
                          <td style="padding: 4px; color: var(--text-secondary);">{r.l}</td>
                          <td style="padding: 4px; text-align: right; color: {r.c || 'var(--text-primary)'};">{r.v}ms</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                  <div style="margin-top: 8px; font-size: var(--text-xs); color: var(--text-muted);">
                    Total time: {benchResult.total_time_ms}ms · {benchMethod} · Concurrency: {benchResult.concurrency}
                  </div>
                </div>
              {/if}
            </div>
          </div>
        </div>
      {/if}

    </div>
  </div>
{/if}

<script lang="ts">
  import { onMount } from "svelte";
  import { dockerApi, sysMethods, type DockerContainer } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  
  import { dockerState } from "../store.svelte";
  import { getCurrentPresetForInstance, getContainerPresetMap, PRESET_LABELS, PRESET_COLORS } from "../lib/presetStateManager";
  import { setEventCooldown } from "../store.svelte";

  let filter = $state<"all" | "running" | "stopped">("all");
  let searchTerm = $state("");
  let actionLoading = $state<string | null>(null);
  let selectedContainer = $state<DockerContainer | null>(null);
  let showRunModal = $state(false);
  let runtimeName = $state("docker");
  let selected = $state<Set<string>>(new Set());
  let batchLoading = $state(false);
  let confirm = $state<{ title: string; message: string; confirmLabel: string; danger: boolean; onConfirm: () => void; onCancel?: () => void } | null>(null);
  
  let presetMap = $state<Map<string, string>>(new Map());
  let activePreset = getCurrentPresetForInstance("default");

  async function refreshContainers() {
    try {
      const list = await dockerApi.listContainers(true);
      dockerState.containers = list;
    } catch (e) {
      dockerState.containers = [];
    }
  }

  $effect(() => {
    getContainerPresetMap("default").then((map) => presetMap = map).catch(() => {});
  });

  $effect(() => {
    // Clear selection on filter change
    if (filter || searchTerm) {
      selected = new Set();
    }
  });

  onMount(() => {
    refreshContainers();
    sysMethods.getRuntimeInfo().then(r => runtimeName = r).catch(() => {});

    const aiListener = async (e: Event) => {
      const customEvent = e as CustomEvent;
      const { payload, resolve } = customEvent.detail;
      let action = customEvent.type.replace("-container", "");
      if (action === "delete") action = "remove";
      try {
        await handleAction(payload, payload, action);
        resolve(`Successfully executed ${action} on container ${payload}`);
      } catch (err) {
        resolve(`Failed to ${action} container: ${err}`);
      }
    };

    window.addEventListener("start-container", aiListener);
    window.addEventListener("stop-container", aiListener);
    window.addEventListener("restart-container", aiListener);
    window.addEventListener("delete-container", aiListener);

    return () => {
      window.removeEventListener("start-container", aiListener);
      window.removeEventListener("stop-container", aiListener);
      window.removeEventListener("restart-container", aiListener);
      window.removeEventListener("delete-container", aiListener);
    };
  });

  async function handleAction(id: string, name: string, action: string) {
    actionLoading = `${id}-${action}`;
    setEventCooldown();
    try {
      if (action === "start") await dockerApi.startContainer(id);
      else if (action === "stop") await dockerApi.stopContainer(id);
      else if (action === "restart") await dockerApi.restartContainer(id);
      else if (action === "remove") {
        return new Promise<void>((resolve, reject) => {
          confirm = {
            title: "Remove Container", danger: true, confirmLabel: "Remove",
            message: `Remove container "${name}"?\n\nThis will permanently delete the container and its data.`,
            onConfirm: async () => {
              confirm = null;
              try {
                await dockerApi.removeContainer(id, true);
                selected = new Set();
                globalToast("success", `Container '${name}' removed`);
                await refreshContainers();
                resolve();
              } catch (e) {
                globalToast("error", String(e));
                reject(e);
              } finally {
                actionLoading = null;
              }
            },
            onCancel: () => { confirm = null; actionLoading = null; reject("cancelled"); }
          };
        });
      }
      else if (action === "pause") await dockerApi.pauseContainer(id);
      else if (action === "unpause") await dockerApi.unpauseContainer(id);

      const past: any = { start: "started", stop: "stopped", restart: "restarted", remove: "removed", pause: "paused", unpause: "unpaused" };
      globalToast("success", `Container '${name}' ${past[action] || action}`);
      await refreshContainers();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      if (action !== "remove") actionLoading = null;
    }
  }

  async function handleBatchStop() {
    const running = filtered.filter(c => selected.has(c.Id) && c.State === "running");
    if (running.length === 0) return;
    confirm = {
      title: "Stop Selected", danger: false, confirmLabel: "Stop All",
      message: `Stop ${running.length} container(s)?\n\n${running.map(c => c.Names).join(", ")}`,
      onConfirm: async () => {
        confirm = null; batchLoading = true; setEventCooldown();
        let ok = 0;
        for (const c of running) {
          try { await dockerApi.stopContainer(c.Id); ok++; } catch {}
        }
        globalToast("success", `Stopped ${ok} container(s)`);
        selected = new Set();
        batchLoading = false;
        refreshContainers();
      },
      onCancel: () => { confirm = null; }
    };
  }

  async function handleBatchRemove() {
    const targets = filtered.filter(c => selected.has(c.Id));
    if (targets.length === 0) return;
    confirm = {
      title: "Remove Selected", danger: true, confirmLabel: `Remove ${targets.length}`,
      message: `Remove ${targets.length} container(s)?\n\n${targets.map(c => c.Names).join(", ")}\n\nThis cannot be undone.`,
      onConfirm: async () => {
        confirm = null; batchLoading = true; setEventCooldown();
        let ok = 0;
        for (const c of targets) {
          try { await dockerApi.removeContainer(c.Id, true); ok++; } catch {}
        }
        globalToast("success", `Removed ${ok} container(s)`);
        selected = new Set();
        batchLoading = false;
        refreshContainers();
      },
      onCancel: () => { confirm = null; }
    };
  }

  let filtered = $derived(dockerState.containers.filter(c => {
    if (filter === "running") return c.State === "running";
    if (filter === "stopped") return c.State !== "running";
    return true;
  }).filter(c => {
    if (!searchTerm) return true;
    const term = searchTerm.toLowerCase();
    return c.Names.toLowerCase().includes(term) || c.Image.toLowerCase().includes(term) || c.Id.toLowerCase().includes(term);
  }));

  let runningCount = $derived(dockerState.containers.filter(c => c.State === "running").length);

  function toggleSelect(id: string) {
    const next = new Set(selected);
    next.has(id) ? next.delete(id) : next.add(id);
    selected = next;
  }

  function toggleAll() {
    if (selected.size === filtered.length) selected = new Set();
    else selected = new Set(filtered.map(c => c.Id));
  }

  // --- Run Modal State ---
  let runImage = $state("");
  let runName = $state("");
  let runPorts = $state("");
  let runEnv = $state("");
  let runVols = $state("");
  let runDetach = $state(true);
  let runRemove = $state(false);
  let runLoading = $state(false);

  async function handleRun() {
    if (!runImage.trim()) return;
    runLoading = true;
    try {
      const p = runPorts.split("\n").map(s => s.trim()).filter(Boolean);
      const e = runEnv.split("\n").map(s => s.trim()).filter(Boolean);
      const v = runVols.split("\n").map(s => s.trim()).filter(Boolean);
      await dockerApi.runContainer(runImage.trim(), runName.trim(), p, e, v, runDetach, runRemove);
      globalToast("success", "Container started!");
      showRunModal = false;
      refreshContainers();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      runLoading = false;
    }
  }

  // --- Detail View State ---
  let detailTab = $state<"overview" | "logs" | "stats" | "exec" | "inspect">("overview");
  let detailLogs = $state("");
  let detailStats = $state("");
  let detailTop = $state("");
  let detailInspect = $state("");
  let detailLogLines = $state(200);
  let detailLogLoading = $state(false);
  let detailAutoScroll = $state(true);
  let detailExecCmd = $state("");
  let detailExecOutput = $state<string[]>([]);
  let detailExecLoading = $state(false);
  
  let logRef: HTMLDivElement | undefined = $state();
  let execRef: HTMLDivElement | undefined = $state();

  async function fetchLogs() {
    if (!selectedContainer) return;
    detailLogLoading = true;
    try {
      detailLogs = await dockerApi.containerLogs(selectedContainer.Id, detailLogLines);
      if (detailAutoScroll && logRef) {
        setTimeout(() => logRef?.scrollTo(0, logRef.scrollHeight), 50);
      }
    } catch (e) { detailLogs = `Error: ${e}`; }
    finally { detailLogLoading = false; }
  }

  async function fetchStats() {
    if (!selectedContainer) return;
    try {
      detailStats = await dockerApi.containerStats(selectedContainer.Id);
      detailTop = await dockerApi.containerTop(selectedContainer.Id);
    } catch (e) { detailStats = `Error: ${e}`; }
  }

  async function fetchInspect() {
    if (!selectedContainer) return;
    try {
      detailInspect = JSON.stringify(JSON.parse(await dockerApi.inspectContainer(selectedContainer.Id)), null, 2);
    } catch (e) { detailInspect = `Error: ${e}`; }
  }

  $effect(() => {
    if (selectedContainer) {
      if (detailTab === "logs") fetchLogs();
      if (detailTab === "stats") fetchStats();
      if (detailTab === "inspect") fetchInspect();
    }
  });

  // Polling for detail view
  $effect(() => {
    if (!selectedContainer) return;
    let int: any;
    if (detailTab === "logs") int = setInterval(fetchLogs, 3000);
    if (detailTab === "stats") int = setInterval(fetchStats, 5000);
    return () => clearInterval(int);
  });

  async function handleExec() {
    if (!detailExecCmd.trim() || !selectedContainer) return;
    detailExecLoading = true;
    const cmd = detailExecCmd;
    detailExecOutput = [...detailExecOutput, `${selectedContainer.Names}$ ${cmd}`];
    detailExecCmd = "";
    try {
      const res = await dockerApi.containerExec(selectedContainer.Id, cmd);
      detailExecOutput = [...detailExecOutput, res];
    } catch(e) { detailExecOutput = [...detailExecOutput, `Error: ${e}`]; }
    finally { 
      detailExecLoading = false; 
      if (execRef) setTimeout(() => execRef?.scrollTo(0, execRef.scrollHeight), 50);
    }
  }

</script>

<div class="content-header">
  <h1>
    {t('containers.title', { default: 'Containers' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {runningCount} {t('containers.running', { default: 'running' })} · {dockerState.containers.length} {t('containers.total', { default: 'total' })}
    </span>
    {#if runtimeName}
      <span style="font-size: var(--text-xs); background: var(--bg-secondary); border: 1px solid var(--border-primary); padding: 2px 8px; border-radius: 12px; margin-left: 12px; color: var(--text-muted);">
        {runtimeName === "podman" ? "🦭 Podman" : runtimeName === "containerd" ? "📦 Containerd" : "🐳 Docker"}
      </span>
    {/if}
  </h1>
  <div class="content-header-actions">
    <input class="input" placeholder={t('containers.search', { default: 'Search containers...' })} bind:value={searchTerm} style="width: 180px;" />
    <div style="display: flex; gap: 2px; background: var(--bg-card); border-radius: var(--radius-md); padding: 2px;">
      {#each ["all", "running", "stopped"] as f}
        <button class="btn" style="background: {filter === f ? 'var(--bg-card-hover)' : 'transparent'}; color: {filter === f ? 'var(--text-primary)' : 'var(--text-muted)'}; border: none; font-size: var(--text-xs); padding: 4px 10px; text-transform: capitalize;" onclick={() => filter = f as any}>
          {t(`containers.filter_${f}`, { default: f })}
        </button>
      {/each}
    </div>
    <button class="btn btn-primary" style="display: flex; align-items: center; gap: 4px;" onclick={() => showRunModal = true}>
      {t('containers.run', { default: 'Run' })}
    </button>
  </div>
</div>

<div class="content-body">
  {#if activePreset && activePreset !== "custom" && presetMap.size > 0}
    <div style="display: flex; align-items: center; gap: 10px; padding: 8px 14px; margin-bottom: 12px; border-radius: var(--radius-md); background: {PRESET_COLORS[activePreset] || PRESET_COLORS.custom}08; border: 1px solid {PRESET_COLORS[activePreset] || PRESET_COLORS.custom}20;">
      <span style="font-size: var(--text-xs); color: var(--text-secondary);">{t('containers.active_workspace', { default: 'Active Workspace' })}: {PRESET_LABELS[activePreset] || activePreset}</span>
    </div>
  {/if}

  {#if selected.size > 0}
    <div style="display: flex; align-items: center; gap: 12px; padding: 10px 16px; margin-bottom: 12px; background: rgba(88,166,255,0.08); border: 1px solid rgba(88,166,255,0.25); border-radius: var(--radius-md);">
      <span style="font-size: var(--text-sm); color: var(--accent-blue); font-weight: 600;">{selected.size} {t('containers.selected', { default: 'selected' })}</span>
      <div style="flex: 1;"></div>
      <button class="btn btn-ghost" style="font-size: var(--text-xs); color: var(--accent-yellow);" onclick={handleBatchStop} disabled={batchLoading}>{t('containers.stop_selected', { default: 'Stop Selected' })}</button>
      <button class="btn btn-ghost" style="font-size: var(--text-xs); color: var(--accent-red);" onclick={handleBatchRemove} disabled={batchLoading}>{batchLoading ? t('containers.removing', { default: 'Removing...' }) : t('containers.remove_selected', { default: 'Remove Selected' })}</button>
      <button class="btn btn-ghost" style="font-size: var(--text-xs);" onclick={() => selected = new Set()}>{t('containers.clear', { default: 'Clear' })}</button>
    </div>
  {/if}

  {#if filtered.length > 0}
    <div class="vtable">
      <div class="vtable-header" style="display: grid; grid-template-columns: 44px minmax(160px,1.5fr) minmax(140px,1fr) minmax(160px,200px) minmax(120px,1fr) 180px;">
        <div class="vtable-header-cell" style="text-align: center;">
          <input type="checkbox" class="checkbox" checked={filtered.length > 0 && selected.size === filtered.length} onchange={toggleAll} />
        </div>
        <div class="vtable-header-cell">{t('containers.name', { default: 'Name' })}</div>
        <div class="vtable-header-cell">{t('containers.image', { default: 'Image' })}</div>
        <div class="vtable-header-cell">{t('containers.status', { default: 'Status' })}</div>
        <div class="vtable-header-cell">{t('containers.ports', { default: 'Ports' })}</div>
        <div class="vtable-header-cell" style="text-align: right;">{t('containers.actions', { default: 'Actions' })}</div>
      </div>
      
      <div class="vtable-scroll">
        {#each filtered as c (c.Id)}
          {@const isRunning = c.State === "running"}
          {@const isPaused = c.Status.toLowerCase().includes("paused")}
          {@const isLoading = actionLoading?.startsWith(c.Id)}
          <div role="button" tabindex="0" class="vtable-row {selected.has(c.Id) ? 'selected' : ''}" style="display: grid; grid-template-columns: 44px minmax(160px,1.5fr) minmax(140px,1fr) minmax(160px,200px) minmax(120px,1fr) 180px; opacity: {isLoading ? 0.6 : 1};" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => selectedContainer = c}>
            <div role="button" tabindex="0" class="vtable-cell" style="text-align: center;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()}>
              <input type="checkbox" class="checkbox" checked={selected.has(c.Id)} onchange={() => toggleSelect(c.Id)} />
            </div>
            <div class="vtable-cell">
              <div style="display: flex; align-items: center; gap: 8px;">
                <div style="width: 8px; height: 8px; border-radius: 50%; background: {isPaused ? 'var(--accent-yellow)' : isRunning ? 'var(--status-running)' : 'var(--status-stopped)'}; box-shadow: {isRunning && !isPaused ? '0 0 6px var(--status-running)' : 'none'}; flex-shrink: 0;"></div>
                <span style="font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title={c.Names}>{c.Names}</span>
              </div>
            </div>
            <div class="vtable-cell" style="color: var(--text-secondary); font-family: var(--font-mono); font-size: var(--text-xs);">{c.Image}</div>
            <div class="vtable-cell">
              <span class="badge badge-{isPaused ? 'stopped' : isRunning ? 'running' : 'stopped'}" title={c.Status}>
                <span class="badge-dot"></span>
                <span class="badge-label">{isPaused ? t('containers.paused', { default: 'Paused' }) : c.State}</span>
                <span class="badge-detail">{c.Status.replace(/^\S+\s*/, "")}</span>
              </span>
            </div>
            <div class="vtable-cell" style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-muted);" title={c.Ports || ""}>{c.Ports || "—"}</div>
            <div role="button" tabindex="0" class="vtable-cell" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()}>
              <div class="table-actions" style="justify-content: flex-end;">
                {#if isPaused}
                  <button class="btn btn-ghost btn-icon" data-tooltip={t('containers.unpause', { default: 'Unpause' })} disabled={!!isLoading} onclick={() => handleAction(c.Id, c.Names, 'unpause')}>U</button>
                {:else if isRunning}
                  <button class="btn btn-ghost btn-icon" data-tooltip={t('containers.stop', { default: 'Stop' })} disabled={!!isLoading} onclick={() => handleAction(c.Id, c.Names, 'stop')}>S</button>
                {:else}
                  <button class="btn btn-ghost btn-icon" data-tooltip={t('containers.start', { default: 'Start' })} disabled={!!isLoading} onclick={() => handleAction(c.Id, c.Names, 'start')}>P</button>
                {/if}
                <button class="btn btn-ghost btn-icon" data-tooltip={t('containers.restart', { default: 'Restart' })} disabled={!!isLoading} onclick={() => handleAction(c.Id, c.Names, 'restart')}>R</button>
                <button class="btn btn-ghost btn-icon" data-tooltip={t('containers.remove', { default: 'Remove' })} disabled={!!isLoading} onclick={() => handleAction(c.Id, c.Names, 'remove')} style="color: var(--accent-red);">X</button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {:else}
    <div class="empty-state">
      <div class="empty-state-title">{t('containers.no_containers', { default: 'No containers' })}</div>
      <div class="empty-state-text">{t('containers.empty_text', { default: 'Click "Run" to start a container from an image.' })}</div>
    </div>
  {/if}
</div>

{#if showRunModal}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => showRunModal = false}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()} style="width: min(600px, 95vw);">
      <div class="modal-header"><h2 class="modal-title">Run Container</h2></div>
      <div style="display: flex; flex-direction: column; gap: 12px;">
        <input class="input" bind:value={runImage} placeholder="Image (e.g. nginx:latest) *" />
        <input class="input" bind:value={runName} placeholder="Container Name (optional)" />
        <textarea class="input" bind:value={runPorts} placeholder="Ports (host:container, one per line)" style="min-height: 50px;"></textarea>
        <textarea class="input" bind:value={runEnv} placeholder="Environment Variables (KEY=VALUE, one per line)" style="min-height: 50px;"></textarea>
        <textarea class="input" bind:value={runVols} placeholder="Volumes (host:container, one per line)" style="min-height: 50px;"></textarea>
        <div style="display: flex; gap: 16px;">
          <label><input type="checkbox" class="checkbox" bind:checked={runDetach} /> Detached mode</label>
          <label><input type="checkbox" class="checkbox" bind:checked={runRemove} /> Remove on exit</label>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-ghost" onclick={() => showRunModal = false}>Cancel</button>
        <button class="btn btn-primary" onclick={handleRun} disabled={runLoading || !runImage.trim()}>{runLoading ? "Starting..." : "Run Container"}</button>
      </div>
    </div>
  </div>
{/if}

{#if selectedContainer}
  {@const isRunning = selectedContainer.State === "running"}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => selectedContainer = null}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()} style="width: min(960px, 95vw); max-height: 85vh;">
      <div class="modal-header">
        <div>
          <h2 class="modal-title" style="display: flex; align-items: center; gap: 8px;">
            <span style="width: 8px; height: 8px; border-radius: 50%; background: {isRunning ? 'var(--status-running)' : 'var(--status-stopped)'}; display: inline-block;"></span>
            {selectedContainer.Names}
          </h2>
          <p style="font-size: var(--text-xs); color: var(--text-muted); font-family: var(--font-mono); margin-top: 4px;">{selectedContainer.Image} · {selectedContainer.Id.substring(0, 12)}</p>
        </div>
        <div style="display: flex; gap: 6px; align-items: center;">
          {#if isRunning}
            <button class="btn btn-ghost" style="font-size: var(--text-xs);" onclick={() => handleAction(selectedContainer!.Id, selectedContainer!.Names, "stop")}>Stop</button>
            <button class="btn btn-ghost" style="font-size: var(--text-xs);" onclick={() => handleAction(selectedContainer!.Id, selectedContainer!.Names, "restart")}>Restart</button>
          {:else}
            <button class="btn btn-primary" style="font-size: var(--text-xs);" onclick={() => handleAction(selectedContainer!.Id, selectedContainer!.Names, "start")}>Start</button>
          {/if}
          <button class="btn btn-icon btn-ghost" onclick={() => selectedContainer = null}>X</button>
        </div>
      </div>

      <div style="display: flex; gap: 2px; border-bottom: 1px solid var(--border-primary); margin-bottom: 16px;">
        {#each ["overview", "logs", "stats", "exec", "inspect"] as t}
          <button class="btn" style="background: transparent; border: none; border-bottom: {detailTab === t ? '2px solid var(--accent-blue)' : '2px solid transparent'}; color: {detailTab === t ? 'var(--text-primary)' : 'var(--text-secondary)'}; border-radius: 0; padding: 8px 16px; font-weight: {detailTab === t ? 600 : 400}; text-transform: capitalize;" onclick={() => detailTab = t as any}>{t}</button>
        {/each}
      </div>

      {#if detailTab === "overview"}
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 16px;">
          <div><span style="font-size: var(--text-xs); color: var(--text-muted);">Status</span><div style="font-size: var(--text-sm);">{selectedContainer.Status}</div></div>
          <div><span style="font-size: var(--text-xs); color: var(--text-muted);">Created</span><div style="font-size: var(--text-sm);">{selectedContainer.CreatedAt}</div></div>
          <div><span style="font-size: var(--text-xs); color: var(--text-muted);">Image</span><div style="font-size: var(--text-sm); font-family: var(--font-mono); word-break: break-all;">{selectedContainer.Image}</div></div>
          <div><span style="font-size: var(--text-xs); color: var(--text-muted);">Command</span><div style="font-size: var(--text-sm); font-family: var(--font-mono); word-break: break-all;">{selectedContainer.Command}</div></div>
          <div><span style="font-size: var(--text-xs); color: var(--text-muted);">Ports</span><div style="font-size: var(--text-sm); font-family: var(--font-mono); word-break: break-all;">{selectedContainer.Ports || "None"}</div></div>
          <div><span style="font-size: var(--text-xs); color: var(--text-muted);">Size</span><div style="font-size: var(--text-sm);">{selectedContainer.Size || "N/A"}</div></div>
        </div>
      {:else if detailTab === "logs"}
        <div style="display: flex; gap: 8px; margin-bottom: 12px; align-items: center;">
          <select class="input select" style="width: 100px;" bind:value={detailLogLines}>
            <option value={50}>50 lines</option>
            <option value={200}>200 lines</option>
            <option value={500}>500 lines</option>
            <option value={1000}>1000 lines</option>
          </select>
          <label style="display: flex; align-items: center; gap: 4px; font-size: var(--text-xs); cursor: pointer;">
            <input type="checkbox" class="checkbox" bind:checked={detailAutoScroll} /> Auto-scroll
          </label>
          <button class="btn btn-ghost" style="padding: 4px 8px; font-size: var(--text-xs);" onclick={fetchLogs}>{detailLogLoading ? "..." : "↻ Refresh"}</button>
        </div>
        <div class="log-viewer" bind:this={logRef} style="max-height: 50vh;">
          <pre style="margin: 0;">{detailLogs || "No logs available"}</pre>
        </div>
      {:else if detailTab === "stats"}
        {#if isRunning}
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
            <h3 style="margin: 0; font-size: var(--text-sm); font-weight: 600;">Processes</h3>
            <button class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px;" onclick={fetchStats}>↻</button>
          </div>
          <pre style="padding: 12px; background: var(--bg-primary); border-radius: 8px; font-size: var(--text-xs); overflow: auto; max-height: 30vh; color: var(--text-secondary); margin: 0;">{detailTop || "No processes running"}</pre>
          <div style="margin-top: 12px;">
            <pre style="padding: 12px; background: var(--bg-primary); border-radius: 8px; font-size: var(--text-xs); overflow: auto; max-height: 20vh; color: var(--text-secondary); margin: 0;">{detailStats}</pre>
          </div>
        {:else}
          <div style="text-align: center; padding: 40px; color: var(--text-muted);">Container is not running</div>
        {/if}
      {:else if detailTab === "exec"}
        {#if isRunning}
          <div bind:this={execRef} style="background: var(--bg-primary); border-radius: 8px; padding: 12px; max-height: 40vh; overflow: auto; margin-bottom: 12px; min-height: 120px;">
            {#if detailExecOutput.length === 0}
              <div style="color: var(--text-muted); font-size: var(--text-sm);">Run commands inside '{selectedContainer.Names}'. Output will appear here.</div>
            {:else}
              {#each detailExecOutput as line}
                <div style="font-family: var(--font-mono); font-size: var(--text-xs); color: {line.startsWith(selectedContainer.Names + '$') ? 'var(--accent-green)' : line.startsWith('Error') ? 'var(--accent-red)' : 'var(--text-secondary)'}; white-space: pre-wrap; padding: 1px 0;">{line}</div>
              {/each}
            {/if}
          </div>
          <div style="display: flex; gap: 8px;">
            <input class="input" bind:value={detailExecCmd} placeholder="{selectedContainer.Names}$ Enter command..." style="flex: 1; font-family: var(--font-mono);" onkeydown={(e) => e.key === 'Enter' && handleExec()} autofocus />
            <button class="btn btn-primary" onclick={handleExec} disabled={detailExecLoading || !detailExecCmd.trim()}>{detailExecLoading ? "Running..." : "Run"}</button>
            <button class="btn btn-ghost" onclick={() => detailExecOutput = []}>Clear</button>
          </div>
        {:else}
          <div style="text-align: center; padding: 40px; color: var(--text-muted);">Container must be running to execute commands</div>
        {/if}
      {:else if detailTab === "inspect"}
        <div class="log-viewer" style="max-height: 55vh;">
          <pre style="margin: 0; color: var(--text-secondary);">{detailInspect}</pre>
        </div>
      {/if}

      <div class="modal-footer">
        <button class="btn btn-primary" onclick={() => selectedContainer = null}>Close</button>
      </div>
    </div>
  </div>
{/if}

{#if confirm}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => confirm?.onCancel?.()}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()} style="width: min(400px, 90vw);">
      <div class="modal-header"><h2 class="modal-title">{confirm.title}</h2></div>
      <p style="color: var(--text-secondary); font-size: var(--text-sm); line-height: 1.6;">{confirm.message}</p>
      <div class="modal-footer">
        <button class="btn btn-ghost" onclick={() => confirm?.onCancel?.()}>Cancel</button>
        <button class="btn {confirm.danger ? 'btn-danger' : 'btn-primary'}" onclick={() => confirm?.onConfirm()}>{confirm.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

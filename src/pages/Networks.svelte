<script lang="ts">
  import { onMount } from "svelte";
  import { networksApi, sysMethods } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { resourceState } from "../store.svelte";
  import * as Icons from "../components/Icons.svelte";
  import { t } from "../lib/i18n.svelte";

  let searchTerm = $state("");
  let showCreate = $state(false);
  let newName = $state("");
  let newDriver = $state("bridge");
  let newSubnet = $state("");
  let actionLoading = $state<string | null>(null);
  let inspecting = $state<string | null>(null);
  let inspectData = $state<string>("");
  let selected = $state<Set<string>>(new Set());
  let batchLoading = $state(false);
  let confirm = $state<{ title: string; message: string; confirmLabel: string; danger: boolean; onConfirm: () => void; onCancel?: () => void } | null>(null);
  let runtimeName = $state("docker");

  async function refreshNetworks() {
    try {
      const list = await networksApi.listNetworks();
      resourceState.networks = list;
    } catch (e) {
      resourceState.networks = [];
    } finally {
      resourceState.networksLoading = false;
    }
  }

  onMount(() => {
    refreshNetworks();
    sysMethods.getRuntimeInfo().then(r => runtimeName = r).catch(() => {});

    const aiListener = async (e: Event) => {
      const customEvent = e as CustomEvent;
      const { payload, resolve } = customEvent.detail;
      const action = customEvent.type.replace("-network", "");
      try {
        if (action === "delete") {
          await executeRemove(payload);
          resolve(`Successfully removed network ${payload}`);
        }
      } catch (err) {
        resolve(`Failed to ${action} network: ${err}`);
      }
    };

    window.addEventListener("delete-network", aiListener);
    return () => window.removeEventListener("delete-network", aiListener);
  });

  $effect(() => {
    if (searchTerm !== undefined) {
      selected = new Set();
    }
  });

  async function handleCreate() {
    if (!newName.trim()) return;
    actionLoading = "create";
    try {
      await networksApi.createNetwork(newName.trim(), newDriver, newSubnet.trim());
      globalToast("success", `Network "${newName}" created`);
      newName = "";
      newSubnet = "";
      showCreate = false;
      refreshNetworks();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  async function handleRemove(name: string) {
    confirm = {
      title: "Remove Network", danger: true, confirmLabel: "Remove",
      message: `Remove network "${name}"?`,
      onConfirm: async () => {
        confirm = null;
        try {
          await executeRemove(name);
        } catch (e) {}
      },
      onCancel: () => { confirm = null; }
    };
  }

  async function executeRemove(name: string) {
    actionLoading = name;
    try {
      await networksApi.removeNetwork(name);
      selected = new Set();
      globalToast("success", `Network "${name}" removed`);
      refreshNetworks();
    } catch (e) {
      globalToast("error", String(e));
      throw e;
    } finally {
      actionLoading = null;
    }
  }

  async function handlePrune() {
    confirm = {
      title: "Prune Networks", danger: false, confirmLabel: "Prune All",
      message: "Remove all unused networks?",
      onConfirm: async () => {
        confirm = null;
        actionLoading = "prune";
        try {
          await networksApi.pruneNetworks();
          selected = new Set();
          globalToast("success", "Unused networks pruned");
          refreshNetworks();
        } catch (e) {
          globalToast("error", String(e));
        } finally {
          actionLoading = null;
        }
      },
      onCancel: () => { confirm = null; }
    };
  }

  async function handleInspect(name: string) {
    if (inspecting === name) { inspecting = null; return; }
    try {
      const data = await networksApi.inspectNetwork(name);
      inspectData = data;
      inspecting = name;
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  function isSystemNetwork(name: string) {
    return ["bridge", "host", "none"].includes(name);
  }

  function driverColor(driver: string) {
    switch (driver) {
      case "bridge": return "var(--accent-blue)";
      case "host": return "var(--accent-orange)";
      case "overlay": return "var(--accent-purple)";
      case "null": return "var(--text-muted)";
      default: return "var(--accent-green)";
    }
  }

  async function handleBatchRemove() {
    const targets = filteredNetworks.filter(n => selected.has(n.Id) && !isSystemNetwork(n.Name));
    if (targets.length === 0) return;
    const names = targets.map(n => n.Name);
    confirm = {
      title: "Remove Selected Networks", danger: true, confirmLabel: `Remove ${names.length}`,
      message: `Remove ${names.length} network(s)?\n\n${names.join(", ")}\n\nThis cannot be undone.`,
      onConfirm: async () => {
        confirm = null;
        batchLoading = true;
        let ok_count = 0;
        for (const name of names) {
          try { await networksApi.removeNetwork(name); ok_count++; } catch {}
        }
        selected = new Set();
        batchLoading = false;
        refreshNetworks();
        if (ok_count > 0) {
          globalToast("success", `Removed ${ok_count} network(s)`);
        } else {
          globalToast("error", "Failed to remove selected networks");
        }
      },
      onCancel: () => { confirm = null; }
    };
  }

  function toggleSelect(id: string) {
    const next = new Set(selected);
    next.has(id) ? next.delete(id) : next.add(id);
    selected = next;
  }

  let filteredNetworks = $derived(resourceState.networks.filter(n => 
    n.Name.toLowerCase().includes(searchTerm.toLowerCase()) || 
    n.Driver.toLowerCase().includes(searchTerm.toLowerCase())
  ));

</script>

<div class="content-header" data-tauri-drag-region>
  <h1>
    {t('networks.title', { default: 'Networks' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {resourceState.networks.length} {t('networks.count', { default: 'networks' })}
    </span>
    {#if runtimeName}
      <span style="font-size: var(--text-xs); background: var(--bg-secondary); border: 1px solid var(--border-primary); padding: 2px 8px; border-radius: 12px; margin-left: 12px; color: var(--text-muted);">
        {#if runtimeName === "podman"}
          🦭 Podman
        {:else if runtimeName === "containerd"}
          📦 Containerd
        {:else}
          {@html Icons.Docker} Docker
        {/if}
      </span>
    {/if}
  </h1>
  <div class="content-header-actions">
    {#if selected.size > 0}
      <button class="btn btn-ghost" style="color: var(--accent-red); font-size: var(--text-sm);" onclick={handleBatchRemove} disabled={batchLoading}>
        {batchLoading ? t('networks.removing', { default: 'Removing...' }) : t('networks.remove_selected', { default: `Remove ${selected.size} Selected` })}
      </button>
    {/if}
    <button class="btn btn-ghost" onclick={handlePrune} disabled={actionLoading === "prune"}>
      {actionLoading === "prune" ? t('networks.pruning', { default: 'Pruning...' }) : t('networks.prune', { default: 'Prune' })}
    </button>
    <button class="btn btn-primary" onclick={() => showCreate = !showCreate}>
      + {t('networks.create_button', { default: 'Create Network' })}
    </button>
    <button class="btn btn-ghost" onclick={refreshNetworks}>{t('networks.refresh', { default: '↻ Refresh' })}</button>
  </div>
</div>

<div class="content-body">
  {#if showCreate}
    <div style="padding: 16px; background: var(--bg-secondary); border-radius: 12px; margin-bottom: 16px; border: 1px solid var(--border-primary);">
      <h3 style="margin: 0 0 12px; font-size: var(--text-base);">{t('networks.create_title', { default: 'Create Network' })}</h3>
      <div style="display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap;">
        <div style="flex: 1; min-width: 180px;">
          <label for="netName" style="display: block; font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 4px;">{t('networks.name', { default: 'Name' })}</label>
          <input id="netName" type="text" bind:value={newName} placeholder="my-network" style="width: 100%; padding: 8px 12px; background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-primary); font-size: var(--text-sm);" onkeydown={(e) => e.key === 'Enter' && handleCreate()} />
        </div>
        <div>
          <label for="netDriver" style="display: block; font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 4px;">{t('networks.driver', { default: 'Driver' })}</label>
          <select id="netDriver" bind:value={newDriver} class="input select">
            <option value="bridge">bridge</option>
            <option value="host">host</option>
            <option value="overlay">overlay</option>
            <option value="macvlan">macvlan</option>
            <option value="ipvlan">ipvlan</option>
          </select>
        </div>
        <div style="flex: 1; min-width: 180px;">
          <label for="netSubnet" style="display: block; font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 4px;">{t('networks.subnet', { default: 'Subnet (optional)' })}</label>
          <input id="netSubnet" type="text" bind:value={newSubnet} placeholder="172.28.0.0/16" style="width: 100%; padding: 8px 12px; background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-primary); font-size: var(--text-sm);" onkeydown={(e) => e.key === 'Enter' && handleCreate()} />
        </div>
        <button class="btn btn-primary" onclick={handleCreate} disabled={actionLoading === "create" || !newName.trim()}>
          {actionLoading === "create" ? t('networks.creating', { default: 'Creating...' }) : t('networks.create', { default: 'Create' })}
        </button>
        <button class="btn btn-ghost" onclick={() => showCreate = false}>{t('networks.cancel', { default: 'Cancel' })}</button>
      </div>
    </div>
  {/if}

  <div style="margin-bottom: 16px;">
    <input type="text" bind:value={searchTerm} placeholder={t('networks.search', { default: 'Search networks...' })} style="width: 100%; padding: 8px 12px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-primary); font-size: var(--text-sm);" />
  </div>

  {#if resourceState.networksLoading}
    <div style="text-align: center; padding: 40px; color: var(--text-muted);">{t('networks.loading', { default: 'Loading networks...' })}</div>
  {:else if filteredNetworks.length === 0}
    <div style="text-align: center; padding: 40px; color: var(--text-muted);">
      {searchTerm ? t('networks.no_match', { default: 'No networks match your search' }) : t('networks.no_networks', { default: 'No networks found' })}
    </div>
  {:else}
    <div style="display: flex; flex-direction: column; gap: 8px;">
      {#each filteredNetworks as net (net.Id)}
        <div style="padding: 16px; background: {selected.has(net.Id) ? 'rgba(88,166,255,0.06)' : 'var(--bg-secondary)'}; border-radius: 12px; border: {selected.has(net.Id) ? '1px solid rgba(88,166,255,0.25)' : '1px solid var(--border-primary)'}; transition: all 150ms;">
          <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px;">
            <div style="display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0;">
              {#if isSystemNetwork(net.Name)}
                <input type="checkbox" class="checkbox" disabled checked={false} title="System network — cannot be removed" />
              {:else}
                <input type="checkbox" class="checkbox" checked={selected.has(net.Id)} onchange={() => toggleSelect(net.Id)} />
              {/if}
              <div style="min-width: 0;">
                <div style="display: flex; align-items: center; gap: 8px;">
                  <span style="font-weight: 600; font-size: var(--text-base);">{net.Name}</span>
                  {#if isSystemNetwork(net.Name)}
                    <span style="font-size: var(--text-xs); padding: 2px 6px; border-radius: 4px; background: rgba(139,148,158,0.2); color: var(--text-muted); flex-shrink: 0;">system</span>
                  {/if}
                </div>
                <div style="color: var(--text-muted); font-size: var(--text-sm); margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                  Driver: <span style="color: {driverColor(net.Driver)};">{net.Driver}</span>
                  {#if net.Scope} · Scope: {net.Scope}{/if}
                  · ID: {net.Id.substring(0, 12)}
                </div>
              </div>
            </div>
            <div style="display: flex; gap: 6px; flex-shrink: 0;">
              <button class="btn btn-ghost" onclick={() => handleInspect(net.Name)} style="font-size: var(--text-xs); padding: 4px 10px;">
                {inspecting === net.Name ? "Hide" : "Inspect"}
              </button>
              {#if !isSystemNetwork(net.Name)}
                <button class="btn btn-ghost" onclick={() => handleRemove(net.Name)} disabled={actionLoading === net.Name} style="font-size: var(--text-xs); padding: 4px 10px; color: var(--accent-red);">
                  {actionLoading === net.Name ? "..." : "Remove"}
                </button>
              {/if}
            </div>
          </div>
          {#if inspecting === net.Name}
            <pre style="margin-top: 12px; padding: 12px; background: var(--bg-primary); border-radius: 8px; font-size: var(--text-xs); overflow: auto; max-height: 300px; color: var(--text-secondary);">{inspectData}</pre>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

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

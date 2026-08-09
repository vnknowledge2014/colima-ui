<script lang="ts">
  import { onMount } from "svelte";
  import { volumesApi, sysMethods } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import { resourceState } from "../store.svelte";
  import * as Icons from "../components/Icons.svelte";
  import { formatVolumeName } from "../lib/formatters";

  let searchTerm = $state("");
  let showCreate = $state(false);
  let newName = $state("");
  let newDriver = $state("local");
  let actionLoading = $state<string | null>(null);
  let inspecting = $state<string | null>(null);
  let inspectData = $state<string>("");
  let selected = $state<Set<string>>(new Set());
  let batchLoading = $state(false);
  let confirm = $state<{ title: string; message: string; confirmLabel: string; danger: boolean; onConfirm: () => void; onCancel?: () => void } | null>(null);
  let runtimeName = $state("docker");

  async function refreshVolumes() {
    try {
      const list = await volumesApi.listVolumes();
      resourceState.volumes = list;
    } catch (e) {
      resourceState.volumes = [];
    } finally {
      resourceState.volumesLoading = false;
    }
  }

  onMount(() => {
    refreshVolumes();
    sysMethods.getRuntimeInfo().then(r => runtimeName = r).catch(() => {});

    const aiListener = async (e: Event) => {
      const customEvent = e as CustomEvent;
      const { payload, resolve } = customEvent.detail;
      const action = customEvent.type.replace("-volume", "");
      try {
        if (action === "delete") {
          await executeRemove(payload);
          resolve(`Successfully removed volume ${payload}`);
        }
      } catch (err) {
        resolve(`Failed to ${action} volume: ${err}`);
      }
    };

    window.addEventListener("delete-volume", aiListener);
    return () => window.removeEventListener("delete-volume", aiListener);
  });

  $effect(() => {
    // Clear selection on search change
    if (searchTerm !== undefined) {
      selected = new Set();
    }
  });

  async function handleCreate() {
    if (!newName.trim()) return;
    actionLoading = "create";
    try {
      await volumesApi.createVolume(newName.trim(), newDriver);
      globalToast("success", `Volume "${newName}" created`);
      newName = "";
      showCreate = false;
      refreshVolumes();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  async function handleRemove(name: string) {
    confirm = {
      title: t("volumes.remove_title", { default: "Remove Volume" }), danger: true, confirmLabel: t("volumes.remove_action", { default: "Remove" }),
      message: t("volumes.remove_confirm", { default: `Remove volume "${name}"?` }),
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
      await volumesApi.removeVolume(name, true);
      selected = new Set();
      globalToast("success", `Volume "${name}" removed`);
      refreshVolumes();
    } catch (e) {
      globalToast("error", String(e));
      throw e;
    } finally {
      actionLoading = null;
    }
  }

  async function handlePrune() {
    confirm = {
      title: t("volumes.prune_title", { default: "Prune Volumes" }), danger: false, confirmLabel: t("volumes.prune_action", { default: "Prune All" }),
      message: t("volumes.prune_confirm", { default: "Remove all unused volumes? This cannot be undone." }),
      onConfirm: async () => {
        confirm = null;
        actionLoading = "prune";
        try {
          await volumesApi.pruneVolumes();
          selected = new Set();
          globalToast("success", "Unused volumes pruned");
          refreshVolumes();
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
      const data = await volumesApi.inspectVolume(name);
      inspectData = data;
      inspecting = name;
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function handleBatchRemove() {
    const targets = filteredVolumes.filter(v => selected.has(v.Name));
    if (targets.length === 0) return;
    const names = targets.map(v => v.Name);
    confirm = {
      title: t("volumes.remove_selected_title", { default: "Remove Selected Volumes" }), danger: true, confirmLabel: t("volumes.remove_count", { default: `Remove ${names.length}`, count: names.length }),
      message: t("volumes.remove_batch_confirm", { default: `Remove ${names.length} volume(s)?\n\n${names.join(", ")}\n\nThis cannot be undone.` }),
      onConfirm: async () => {
        confirm = null;
        batchLoading = true;
        let ok_count = 0;
        for (const name of names) {
          try { await volumesApi.removeVolume(name, true); ok_count++; } catch {}
        }
        globalToast("success", `Removed ${ok_count} volume(s)`);
        selected = new Set();
        batchLoading = false;
        refreshVolumes();
      },
      onCancel: () => { confirm = null; }
    };
  }

  function toggleSelect(name: string) {
    const next = new Set(selected);
    next.has(name) ? next.delete(name) : next.add(name);
    selected = next;
  }

  let filteredVolumes = $derived(resourceState.volumes.filter(v => 
    v.Name.toLowerCase().includes(searchTerm.toLowerCase()) || 
    v.Driver.toLowerCase().includes(searchTerm.toLowerCase())
  ));

</script>

<div class="content-header">
  <h1>
    {t('volumes.title', { default: 'Volumes' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {resourceState.volumes.length} {t('volumes.count_label', { default: 'volume(s)' })}
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
        {batchLoading ? t('volumes.removing', { default: 'Removing...' }) : t('volumes.remove_selected', { default: `Remove ${selected.size} Selected` })}
      </button>
    {/if}
    <button class="btn btn-ghost" onclick={handlePrune} disabled={actionLoading === "prune"}>
      {actionLoading === "prune" ? t('volumes.pruning', { default: 'Pruning...' }) : t('volumes.prune', { default: 'Prune' })}
    </button>
    <button class="btn btn-primary" onclick={() => showCreate = !showCreate}>
      + {t('volumes.create_button', { default: 'Create Volume' })}
    </button>
    <button class="btn btn-ghost" onclick={refreshVolumes}>↻ {t('volumes.refresh', { default: 'Refresh' })}</button>
  </div>
</div>

<div class="content-body">
  {#if showCreate}
    <div style="padding: 16px; background: var(--bg-secondary); border-radius: 12px; margin-bottom: 16px; border: 1px solid var(--border-primary);">
      <h3 style="margin: 0 0 12px; font-size: var(--text-base);">{t('volumes.create_title', { default: 'Create Volume' })}</h3>
      <div style="display: flex; gap: 12px; align-items: flex-end;">
        <div style="flex: 1;">
          <label for="volName" style="display: block; font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 4px;">{t('volumes.name', { default: 'Name' })}</label>
          <input id="volName" type="text" bind:value={newName} placeholder="my-volume" style="width: 100%; padding: 8px 12px; background: var(--bg-primary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-primary); font-size: var(--text-sm);" onkeydown={(e) => e.key === 'Enter' && handleCreate()} />
        </div>
        <div>
          <label for="volDriver" style="display: block; font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 4px;">{t('volumes.driver', { default: 'Driver' })}</label>
          <select id="volDriver" bind:value={newDriver} class="input select">
            <option value="local">local</option>
          </select>
        </div>
        <button class="btn btn-primary" onclick={handleCreate} disabled={actionLoading === "create" || !newName.trim()}>
          {actionLoading === "create" ? t('volumes.creating', { default: 'Creating...' }) : t('volumes.create', { default: 'Create' })}
        </button>
        <button class="btn btn-ghost" onclick={() => showCreate = false}>{t('volumes.cancel', { default: 'Cancel' })}</button>
      </div>
    </div>
  {/if}

  <div style="margin-bottom: 16px;">
    <input type="text" bind:value={searchTerm} placeholder={t('volumes.search', { default: 'Search volumes...' })} style="width: 100%; padding: 8px 12px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-primary); font-size: var(--text-sm);" />
  </div>

  {#if resourceState.volumesLoading}
    <div style="text-align: center; padding: 40px; color: var(--text-muted);">{t('volumes.loading', { default: 'Loading volumes...' })}</div>
  {:else if filteredVolumes.length === 0}
    <div style="text-align: center; padding: 40px; color: var(--text-muted);">
      {searchTerm ? t('volumes.no_search', { default: 'No volumes match your search' }) : t('volumes.no_found', { default: 'No volumes found' })}
    </div>
  {:else}
    <div style="display: flex; flex-direction: column; gap: 8px;">
      {#each filteredVolumes as vol (vol.Name)}
        {@const formatted = formatVolumeName(vol.Name)}
        <div style="padding: 16px; background: {selected.has(vol.Name) ? 'rgba(88,166,255,0.06)' : 'var(--bg-secondary)'}; border-radius: 12px; border: {selected.has(vol.Name) ? '1px solid rgba(88,166,255,0.25)' : '1px solid var(--border-primary)'}; transition: all 150ms;">
          <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px;">
            <div style="display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0;">
              <input type="checkbox" class="checkbox" checked={selected.has(vol.Name)} onchange={() => toggleSelect(vol.Name)} />
              <div style="min-width: 0;">
                <div style="font-weight: 600; font-size: var(--text-base); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title={vol.Name}>
                  {#if formatted.isHash}
                    <span style="font-family: var(--font-mono); font-size: var(--text-sm); color: var(--text-secondary);">{formatted.display}</span>
                  {:else}
                    {formatted.display}
                  {/if}
                </div>
                <div style="color: var(--text-muted); font-size: var(--text-sm); margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title={`Driver: ${vol.Driver}${vol.Scope ? ` · Scope: ${vol.Scope}` : ''}${vol.Mountpoint ? ` · ${vol.Mountpoint}` : ''}`}>
                  {t('volumes.driver', { default: 'Driver' })}: <span style="color: var(--accent-blue);">{vol.Driver}</span>
                  {#if vol.Scope} · {t('volumes.scope', { default: 'Scope' })}: {vol.Scope}{/if}
                  {#if vol.Mountpoint} · {vol.Mountpoint}{/if}
                </div>
              </div>
            </div>
            <div style="display: flex; gap: 6px; flex-shrink: 0;">
              <button class="btn btn-ghost" onclick={() => handleInspect(vol.Name)} style="font-size: var(--text-xs); padding: 4px 10px;">
                {inspecting === vol.Name ? "Hide" : "Inspect"}
              </button>
              <button class="btn btn-ghost" onclick={() => handleRemove(vol.Name)} disabled={actionLoading === vol.Name} style="font-size: var(--text-xs); padding: 4px 10px; color: var(--accent-red);">
                {actionLoading === vol.Name ? "..." : "Remove"}
              </button>
            </div>
          </div>
          {#if inspecting === vol.Name}
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

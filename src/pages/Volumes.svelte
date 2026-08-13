<script lang="ts">
  import { onMount } from "svelte";
  import { volumesApi, sysMethods } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import { resourceState } from "../store.svelte";
  import * as Icons from "../components/Icons.svelte";
  import RowActions from "../components/RowActions.svelte";
  import { formatVolumeName } from "../lib/formatters";
  import { viewInTopology } from "../lib/topology-link";
  import { blockingCapability, capabilityNotice } from "../store/capabilities.svelte";

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
    } catch {
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
      message: t("volumes.remove_confirm", { default: 'Remove volume "{name}"?', name }),
      onConfirm: async () => {
        confirm = null;
        try {
          await executeRemove(name);
        } catch {
          // A failed single removal surfaces through the toast in executeRemove.
        }
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
      title: t("volumes.remove_selected_title", { default: "Remove Selected Volumes" }), danger: true, confirmLabel: t("volumes.remove_count", { default: "Remove {count}", count: names.length }),
      message: t("volumes.remove_batch_confirm", {
        default: "Remove {count} volume(s)?\n\n{names}\n\nThis cannot be undone.",
        count: names.length,
        names: names.join(", "),
      }),
      onConfirm: async () => {
        confirm = null;
        batchLoading = true;
        let ok_count = 0;
        for (const name of names) {
          try { await volumesApi.removeVolume(name, true); ok_count++; } catch { /* continue with the rest */ }
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
    if (next.has(name)) next.delete(name);
    else next.add(name);
    selected = next;
  }

  let filteredVolumes = $derived(resourceState.volumes.filter(v => 
    v.Name.toLowerCase().includes(searchTerm.toLowerCase()) || 
    v.Driver.toLowerCase().includes(searchTerm.toLowerCase())
  ));

</script>

<div class="content-header" data-tauri-drag-region>
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
    {@const blocked = searchTerm ? undefined : blockingCapability("colima", "docker")}
    <div style="text-align: center; padding: 40px; color: var(--text-muted);">
      {#if blocked}
        <div>{capabilityNotice(blocked).title}</div>
        <div style="margin-top: 6px; font-size: 0.9em;">{capabilityNotice(blocked).text}</div>
      {:else}
        {searchTerm ? t('volumes.no_search', { default: 'No volumes match your search' }) : t('volumes.no_found', { default: 'No volumes found' })}
      {/if}
    </div>
  {:else}
    <div class="resource-card-list">
      {#each filteredVolumes as vol (vol.Name)}
        {@const formatted = formatVolumeName(vol.Name)}
        <div class="resource-card {selected.has(vol.Name) ? 'selected' : ''}">
          <!-- Only the header row toggles inspect, so the output it reveals stays
               selectable without collapsing under the click. -->
          <div role="button" tabindex="0" class="resource-card-body" style="cursor: pointer;" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => handleInspect(vol.Name)}>
            <div class="resource-card-main">
              <input type="checkbox" class="checkbox" checked={selected.has(vol.Name)} onchange={() => toggleSelect(vol.Name)} onclick={(e) => e.stopPropagation()} />
              <div style="min-width: 0;">
                <div class="resource-card-title" title={vol.Name}>
                  {#if formatted.isHash}
                    <span style="font-family: var(--font-mono); font-size: var(--text-sm); color: var(--text-secondary);">{formatted.display}</span>
                  {:else}
                    {formatted.display}
                  {/if}
                </div>
                <div class="resource-card-meta" title={`Driver: ${vol.Driver}${vol.Scope ? ` · Scope: ${vol.Scope}` : ''}${vol.Mountpoint ? ` · ${vol.Mountpoint}` : ''}`}>
                  {t('volumes.driver', { default: 'Driver' })}: <span style="color: var(--accent-blue);">{vol.Driver}</span>
                  {#if vol.Scope} · {t('volumes.scope', { default: 'Scope' })}: {vol.Scope}{/if}
                  {#if vol.Mountpoint} · {vol.Mountpoint}{/if}
                </div>
              </div>
            </div>
            <RowActions
              inline={[{
                icon: Icons.Topology,
                label: t('common.view_in_topology', { default: 'View in topology' }),
                onclick: () => viewInTopology("volume", vol.Name),
              }]}
              menu={[
                {
                  label: inspecting === vol.Name
                    ? t('common.hide_details', { default: 'Hide details' })
                    : t('common.inspect', { default: 'Inspect' }),
                  icon: Icons.Search,
                  action: () => handleInspect(vol.Name),
                },
                { divider: true, label: '', action: () => {} },
                {
                  label: t('volumes.remove', { default: 'Remove' }),
                  icon: Icons.Trash,
                  danger: true,
                  disabled: actionLoading === vol.Name,
                  action: () => handleRemove(vol.Name),
                },
              ]}
            />
          </div>
          {#if inspecting === vol.Name}
            <pre class="resource-card-inspect">{inspectData}</pre>
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

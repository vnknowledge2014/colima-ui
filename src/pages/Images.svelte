<script lang="ts">
  import { onMount } from "svelte";
  import { dockerApi, sysMethods } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { dockerState } from "../store.svelte";
  import * as Icons from "../components/Icons.svelte";
  import { t } from "../lib/i18n.svelte";
  import { formatTimestamp, formatSize, truncateId } from "../lib/formatters";

  let searchTerm = $state("");
  let showPull = $state(false);
  let pullName = $state("");
  let actionLoading = $state<string | null>(null);
  let inspecting = $state<string | null>(null);
  let inspectData = $state<string>("");
  let showTag = $state<string | null>(null);
  let tagTarget = $state("");
  let selected = $state<Set<string>>(new Set());
  let batchLoading = $state(false);
  let confirm = $state<{ title: string; message: string; confirmLabel: string; danger: boolean; onConfirm: () => void; onCancel?: () => void } | null>(null);
  let runtimeName = $state("docker");

  

  async function refreshImages() {
    try {
      const list = await dockerApi.listImages();
      dockerState.images = list;
    } catch (e) {
      dockerState.images = [];
    }
  }

  onMount(() => {
    refreshImages();
    sysMethods.getRuntimeInfo().then(r => runtimeName = r).catch(() => {});

    const aiListener = async (e: Event) => {
      const customEvent = e as CustomEvent;
      const { payload, resolve } = customEvent.detail;
      const action = customEvent.type.replace("-image", "");
      try {
        if (action === "pull") {
          await handlePull(payload);
          resolve(`Successfully pulled image ${payload}`);
        } else if (action === "delete") {
          await handleRemove(payload, payload, true);
          resolve(`Successfully removed image ${payload}`);
        }
      } catch (err) {
        resolve(`Failed to ${action} image: ${err}`);
      }
    };

    window.addEventListener("pull-image", aiListener);
    window.addEventListener("delete-image", aiListener);

    return () => {
      window.removeEventListener("pull-image", aiListener);
      window.removeEventListener("delete-image", aiListener);
    };
  });

  $effect(() => {
    // Clear selection on search change
    if (searchTerm !== undefined) {
      selected = new Set();
    }
  });

  async function handlePull(overrideName?: string) {
    const name = (typeof overrideName === 'string' ? overrideName : pullName).trim();
    if (!name) return;
    globalToast("success", `Pulling image "${name}"... This may take a moment.`);
    pullName = "";
    showPull = false;
    try {
      await dockerApi.pullImage(name);
      globalToast("success", `Image "${name}" pulled successfully`);
      refreshImages();
    } catch (e) {
      globalToast("error", `Pull failed: ${e}`);
      throw e;
    }
  }

  async function handleRemove(imageId: string, name: string, forceDelete: boolean = false) {
    if (!forceDelete) {
      return new Promise<void>((resolve, reject) => {
        confirm = {
          title: "Remove Image", danger: true, confirmLabel: "Remove",
          message: `Remove image "${name}"?\n\nIf the image is used by running containers, they will be stopped and removed automatically.`,
          onConfirm: async () => {
            confirm = null;
            try {
              await executeRemove(imageId, name);
              resolve();
            } catch (e) { reject(e); }
          },
          onCancel: () => { confirm = null; reject("User cancelled"); }
        };
      });
    } else {
      await executeRemove(imageId, name);
    }
  }

  async function executeRemove(imageId: string, name: string) {
    actionLoading = imageId;
    try {
      await dockerApi.removeImage(imageId, true);
      selected = new Set();
      globalToast("success", `Image "${name}" removed`);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("being used by running container")) {
        globalToast("error", `Cannot remove "${name}" — it is used by a running container. Stop the container first.`);
      } else if (msg.includes("cannot be forced")) {
        globalToast("error", `Cannot force-remove "${name}" — stop related containers first.`);
      } else {
        globalToast("error", msg);
      }
      throw e;
    } finally {
      actionLoading = null;
      refreshImages();
    }
  }

  async function handlePrune() {
    confirm = {
      title: "Prune Images", danger: false, confirmLabel: "Prune All",
      message: "Remove all unused images? This cannot be undone.",
      onConfirm: async () => {
        confirm = null;
        actionLoading = "prune";
        try {
          await dockerApi.pruneImages();
          selected = new Set();
          globalToast("success", "Unused images pruned");
        } catch (e) {
          globalToast("error", String(e));
        } finally {
          actionLoading = null;
          refreshImages();
        }
      },
      onCancel: () => { confirm = null; }
    };
  }

  async function handleInspect(imageId: string) {
    if (inspecting === imageId) { inspecting = null; return; }
    try {
      const data = await dockerApi.inspectImage(imageId);
      inspectData = data;
      inspecting = imageId;
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  async function handleTag(source: string) {
    if (!tagTarget.trim()) return;
    actionLoading = "tag";
    try {
      await dockerApi.tagImage(source, tagTarget.trim());
      globalToast("success", `Image tagged as "${tagTarget}"`);
      showTag = null;
      tagTarget = "";
      refreshImages();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      actionLoading = null;
    }
  }

  async function handleBatchRemove() {
    const targets = filteredImages.filter(i => selected.has(i.Id));
    if (targets.length === 0) return;
    const names = targets.map(i => `${i.Repository}:${i.Tag}`);
    confirm = {
      title: "Remove Selected Images", danger: true, confirmLabel: `Remove ${names.length}`,
      message: `Remove ${names.length} image(s)?\n\n${names.join("\n")}\n\nThis cannot be undone.`,
      onConfirm: async () => {
        confirm = null;
        batchLoading = true;
        let ok_count = 0;
        for (const img of targets) {
          try { await dockerApi.removeImage(img.Id, true); ok_count++; } catch {}
        }
        globalToast("success", `Removed ${ok_count} image(s)`);
        selected = new Set();
        batchLoading = false;
        refreshImages();
      },
      onCancel: () => { confirm = null; }
    };
  }

  function toggleSelect(id: string) {
    const next = new Set(selected);
    next.has(id) ? next.delete(id) : next.add(id);
    selected = next;
  }
  
  function toggleAll() {
    if (selected.size === filteredImages.length) selected = new Set();
    else selected = new Set(filteredImages.map(i => i.Id));
  }

  let filteredImages = $derived(dockerState.images.filter(img => {
    if (!searchTerm) return true;
    const term = searchTerm.toLowerCase();
    return img.Repository.toLowerCase().includes(term) || img.Tag.toLowerCase().includes(term) || img.Id.toLowerCase().includes(term);
  }));

  let totalSize = $derived(dockerState.images.reduce((sum, img) => {
    const sizeStr = String(img.Size || '0');
    const match = sizeStr.match(/([\d.]+)\s*(GB|MB|KB|B)/i);
    if (match) {
      const val = parseFloat(match[1]);
      const unit = match[2].toUpperCase();
      if (unit === "GB") return sum + val * 1024;
      if (unit === "MB") return sum + val;
      if (unit === "KB") return sum + val / 1024;
      if (unit === "B") return sum + val / (1024 * 1024);
    }
    const num = parseFloat(sizeStr);
    if (!isNaN(num) && num > 0) return sum + num / (1024 * 1024);
    return sum;
  }, 0));

</script>

<div class="content-header" data-tauri-drag-region>
  <h1>
    {t('images.title', { default: 'Images' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {dockerState.images.length} {t('images.count', { default: 'images' })} · {totalSize > 1024 ? `${(totalSize / 1024).toFixed(1)} GB` : `${totalSize.toFixed(0)} MB`}
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
    <input class="input" placeholder={t('images.search', { default: 'Search images...' })} bind:value={searchTerm} style="width: 200px;" />
    <button class="btn btn-ghost" onclick={handlePrune} disabled={actionLoading === "prune"}>{actionLoading === "prune" ? t('images.pruning', { default: 'Pruning...' }) : t('images.prune', { default: 'Prune' })}</button>
    <button class="btn btn-primary" onclick={() => showPull = !showPull}>{t('images.pull', { default: 'Pull Image' })}</button>
  </div>
</div>

<div class="content-body">
  {#if showPull}
    <div class="card" style="margin-bottom: 16px; padding: 16px;">
      <h3 style="margin: 0 0 12px; font-size: var(--text-base);">{t('images.pull_title', { default: 'Pull Docker Image' })}</h3>
      <div style="display: flex; gap: 12px; align-items: flex-end;">
        <div style="flex: 1;">
          <label for="pullImageName" style="display: block; font-size: var(--text-sm); color: var(--text-secondary); margin-bottom: 4px;">{t('images.name', { default: 'Image name' })}</label>
          <input id="pullImageName" class="input" bind:value={pullName} placeholder="nginx:latest, ubuntu:22.04, docker.io/library/redis..." style="width: 100%;" onkeydown={(e) => e.key === 'Enter' && handlePull()} autofocus />
        </div>
        <button class="btn btn-primary" onclick={() => handlePull()} disabled={!pullName.trim()}>{t('images.pull_action', { default: 'Pull' })}</button>
        <button class="btn btn-ghost" onclick={() => showPull = false}>{t('images.cancel', { default: 'Cancel' })}</button>
      </div>
    </div>
  {/if}

  {#if selected.size > 0}
    <div style="display: flex; align-items: center; gap: 12px; padding: 10px 16px; margin-bottom: 12px; background: rgba(88,166,255,0.08); border: 1px solid rgba(88,166,255,0.25); border-radius: var(--radius-md);">
      <span style="font-size: var(--text-sm); color: var(--accent-blue); font-weight: 600;">{selected.size} {t('images.selected', { default: 'selected' })}</span>
      <div style="flex: 1;"></div>
      <button class="btn btn-ghost" style="font-size: var(--text-xs); color: var(--accent-red);" onclick={handleBatchRemove} disabled={batchLoading}>{batchLoading ? t('images.removing', { default: 'Removing...' }) : t('images.remove_selected', { default: 'Remove Selected' })}</button>
      <button class="btn btn-ghost" style="font-size: var(--text-xs);" onclick={() => selected = new Set()}>{t('images.clear', { default: 'Clear' })}</button>
    </div>
  {/if}

  {#if filteredImages.length > 0}
    <div class="vtable">
      <div class="vtable-header" style="display: grid; grid-template-columns: 44px minmax(160px,1.5fr) 100px 120px minmax(100px,0.8fr) 120px 160px;">
        <div class="vtable-header-cell" style="text-align: center;">
          <input type="checkbox" class="checkbox" checked={filteredImages.length > 0 && selected.size === filteredImages.length} onchange={toggleAll} />
        </div>
        <div class="vtable-header-cell">{t('images.repository', { default: 'Repository' })}</div>
        <div class="vtable-header-cell">{t('images.tag', { default: 'Tag' })}</div>
        <div class="vtable-header-cell">{t('images.id', { default: 'Image ID' })}</div>
        <div class="vtable-header-cell">{t('images.created', { default: 'Created' })}</div>
        <div class="vtable-header-cell">{t('images.size', { default: 'Size' })}</div>
        <div class="vtable-header-cell">{t('images.actions', { default: 'Actions' })}</div>
      </div>
      
      <div class="vtable-scroll">
        {#each filteredImages as img (img.Id)}
          <div class="vtable-row {selected.has(img.Id) ? 'selected' : ''}" style="display: grid; grid-template-columns: 44px minmax(160px,1.5fr) 100px 120px minmax(100px,0.8fr) 120px 160px;">
            <div class="vtable-cell" style="text-align: center;">
              <input type="checkbox" class="checkbox" checked={selected.has(img.Id)} onchange={() => toggleSelect(img.Id)} />
            </div>
            <div class="vtable-cell" style="font-weight: 500;">{img.Repository}</div>
            <div class="vtable-cell">
              <span style="padding: 2px 8px; border-radius: var(--radius-sm); background: {img.Tag === 'latest' ? 'rgba(63,185,80,0.1)' : 'rgba(88,166,255,0.1)'}; color: {img.Tag === 'latest' ? 'var(--accent-green)' : 'var(--accent-blue)'}; font-size: var(--text-xs); font-weight: 500;">{img.Tag}</span>
            </div>
            <div class="vtable-cell" style="font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-muted);" title={img.Id}>{truncateId(img.Id)}</div>
            <div class="vtable-cell" style="color: var(--text-secondary); font-size: var(--text-sm);" title={img.CreatedAt}>{formatTimestamp(img.CreatedAt)}</div>
            <div class="vtable-cell" style="font-family: var(--font-mono); font-size: var(--text-sm); font-variant-numeric: tabular-nums;" title={img.Size}>{formatSize(img.Size)}</div>
            <div class="vtable-cell">
              <div style="display: flex; gap: 4px;">
                <button class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px;" onclick={() => handleInspect(img.Id)}>{inspecting === img.Id ? 'Hide' : 'Inspect'}</button>
                <button class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px;" onclick={() => { showTag = showTag === img.Id ? null : img.Id; tagTarget = ''; }}>Tag</button>
                <button class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px; color: var(--accent-red);" onclick={() => handleRemove(img.Id, `${img.Repository}:${img.Tag}`)} disabled={actionLoading === img.Id}>{actionLoading === img.Id ? '...' : 'Remove'}</button>
              </div>
            </div>
          </div>
          
          {#if showTag === img.Id}
            <div style="padding: 8px 16px; background: var(--bg-secondary); border-bottom: 1px solid var(--border-subtle);">
              <div style="display: flex; gap: 8px; align-items: center;">
                <span style="font-size: var(--text-sm); color: var(--text-secondary);">Tag {img.Repository}:{img.Tag} as:</span>
                <input class="input" bind:value={tagTarget} placeholder="myrepo/myimage:v1.0" style="flex: 1;" onkeydown={(e) => e.key === 'Enter' && handleTag(`${img.Repository}:${img.Tag}`)} autofocus />
                <button class="btn btn-primary" onclick={() => handleTag(`${img.Repository}:${img.Tag}`)} disabled={actionLoading === 'tag' || !tagTarget.trim()} style="font-size: var(--text-sm);">{actionLoading === 'tag' ? '...' : 'Tag'}</button>
                <button class="btn btn-ghost" onclick={() => showTag = null} style="font-size: var(--text-sm);">Cancel</button>
              </div>
            </div>
          {/if}
          
          {#if inspecting === img.Id}
            <pre style="margin: 0; padding: 12px 16px; background: var(--bg-secondary); font-size: var(--text-xs); overflow: auto; max-height: 300px; color: var(--text-secondary); border-bottom: 1px solid var(--border-subtle);">{inspectData}</pre>
          {/if}
        {/each}
      </div>
    </div>
  {:else}
    <div class="empty-state">
      <div class="empty-state-title">{searchTerm ? "No matching images" : "No Docker images"}</div>
      <div class="empty-state-text">{searchTerm ? "Try a different search term." : "Click \"Pull Image\" to download your first image."}</div>
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

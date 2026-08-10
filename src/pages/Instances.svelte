<script lang="ts">
  import { onMount } from "svelte";
  import { colimaApi, kindApi, sysMethods, aiApi, type ColimaInstance, type StartConfig, type HostSpecs } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import { dashboardState } from "../store.svelte";
  import { handleInstanceStarted, handleInstanceStopping, getCurrentPresetForInstance, BUILT_IN_PRESETS } from "../lib/presetStateManager";
  import { getAppSetting } from "../lib/settingsStore.svelte";

  interface InstancePreset {
    id: string; label: string; cpus: number; memory: number; disk: number;
    description?: string; color?: string; runtime?: string;
  }

  let { onRefresh } = $props<{ onRefresh: () => void }>();

  // --- State ---
  let showCreate = $state(false);
  let showCreateKind = $state(false);
  let actionLoading = $state<string | null>(null);
  let qemuChecking = $state(false);
  let qemuMissing = $state(false);

  $effect(() => {
    if (newConfig.vm_type === "qemu") {
      qemuChecking = true;
      sysMethods.checkTool("qemu-img").then(res => {
        qemuMissing = !res.installed;
      }).finally(() => {
        qemuChecking = false;
      });
    } else {
      qemuMissing = false;
      qemuChecking = false;
    }
  });
  let confirm = $state<{ title: string; message: string; confirmLabel: string; danger: boolean; onConfirm: () => void; onCancel?: () => void } | null>(null);
  let selected = $state<{ type: "colima"; data: ColimaInstance } | { type: "kind"; name: string } | null>(null);
  let kindClusters = $state<string[]>([]);
  let kindLoading = $state(true);

  // New Instance State
  let newConfig = $state<StartConfig>({
    profile: "default", runtime: "docker", cpus: 2, memory: 2, disk: 60, vm_type: "vz",
    kubernetes: false, kubernetes_version: "", arch: "", mount_type: "", mounts: [], dns: [], network_address: false
  });
  let instanceRole = $state<"standalone" | "worker">("standalone");
  let selectedMaster = $state("");
  let detecting = $state(false);
  let aiOptimizing = $state(false);
  let aiOptimized = $state(false);
  let presets = $state<InstancePreset[]>(BUILT_IN_PRESETS as InstancePreset[]);

  // --- Helpers ---
  function formatBytes(bytes: number) {
    if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
    if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
    return `${bytes} B`;
  }

  function generateOptimalPresets(specs: HostSpecs): InstancePreset[] {
    const { cpu_cores: cpus, memory_gib: mem, disk_free_gib: diskFree } = specs;
    const diskBudget = Math.max(diskFree > 0 ? Math.floor(diskFree * 0.8) : 200, 20);
    const clamp = (v: number, min: number, max: number) => Math.max(min, Math.min(max, v));
    const roundMem = (v: number) => Math.max(1, Math.round(v));

    return [
      { id: "minimal", label: "Minimal", cpus: clamp(Math.floor(cpus * 0.15), 1, 4), memory: roundMem(mem * 0.1), disk: clamp(Math.floor(diskBudget * 0.1), 10, 30), color: "var(--accent-green)" },
      { id: "development", label: "Dev", cpus: clamp(Math.floor(cpus * 0.25), 1, 8), memory: roundMem(mem * 0.25), disk: clamp(Math.floor(diskBudget * 0.25), 20, 100), color: "var(--accent-blue)" },
      { id: "standard", label: "Standard", cpus: clamp(Math.floor(cpus * 0.5), 2, 16), memory: roundMem(mem * 0.5), disk: clamp(Math.floor(diskBudget * 0.4), 40, 200), color: "var(--accent-purple)" },
      { id: "power", label: "Power", cpus: clamp(Math.floor(cpus * 0.75), 4, 32), memory: roundMem(mem * 0.75), disk: clamp(Math.floor(diskBudget * 0.6), 60, 500), color: "var(--accent-orange)" },
      { id: "kubernetes", label: "K8s", cpus: clamp(Math.floor(cpus * 0.5), 2, 16), memory: roundMem(mem * 0.5), disk: clamp(Math.floor(diskBudget * 0.4), 40, 200), color: "#a78bfa" }
    ];
  }

  async function handleAutoDetect() {
    detecting = true;
    aiOptimized = false;
    try {
      const specs = await sysMethods.hostSpecs();
      presets = generateOptimalPresets(specs);
      globalToast("success", `Detected: ${specs.cpu_cores} CPUs · ${specs.memory_gib} GiB RAM`);

      const apiKey = getAppSetting("ai_api_key");
      const provider = getAppSetting("ai_provider", "anthropic");
      const model = getAppSetting("ai_model");
      const endpoint = getAppSetting("ai_endpoint");

      if ((apiKey || provider === "ollama-local") && model) {
        aiOptimizing = true;
        try {
          const prompt = `You are a macOS VM resource allocation expert. Given these host specs:
- CPU cores: ${specs.cpu_cores}
- Total RAM: ${specs.memory_gib} GiB
- Free disk: ${specs.disk_free_gib} GiB
Return ONLY a valid JSON object with optimized Colima VM settings for 5 profiles: minimal, development, standard, power, kubernetes.
Rules:
- Never exceed 80% of host CPU/RAM
- All values must be integers
Format: {"minimal": {"cpus": N, "memory": N, "disk": N}, ...}`;

          const raw = await aiApi.chat(provider, model, apiKey, [{ role: "user", content: prompt }], endpoint);
          const text = typeof raw === "string" ? raw : String(raw);
          const jsonMatch = text.match(/\{[\s\S]*\}/);
          if (jsonMatch) {
            const data = JSON.parse(jsonMatch[0]);
            presets = presets.map(p => {
              const override = data[p.id];
              return override ? { ...p, cpus: override.cpus ?? p.cpus, memory: override.memory ?? p.memory, disk: override.disk ?? p.disk } : p;
            });
            aiOptimized = true;
            globalToast("success", "AI optimally adjusted presets for your hardware.");
          }
        } catch (e) {
          console.error("AI optimization failed", e);
        } finally {
          aiOptimizing = false;
        }
      }
    } catch (e) {
      globalToast("error", "Hardware detection failed");
    } finally {
      detecting = false;
    }
  }

  function applyPreset(p: InstancePreset) {
    newConfig.cpus = p.cpus;
    newConfig.memory = p.memory;
    newConfig.disk = p.disk;
  }

  async function fetchKind() {
    try {
      const raw = await kindApi.list();
      kindClusters = raw.trim().split("\n").filter(Boolean).filter(c => c !== "No kind clusters found.");
    } catch {
      kindClusters = [];
    }
    kindLoading = false;
  }

  onMount(() => {
    fetchKind();
  });

  $effect(() => {
    if (!selected && dashboardState.colimaInstances.length > 0) {
      selected = { type: "colima", data: dashboardState.colimaInstances[0] };
    }
  });

  $effect(() => {
    if (selected?.type === "colima") {
      const fresh = dashboardState.colimaInstances.find(i => i.name === (selected as any).data.name);
      if (fresh && JSON.stringify(fresh) !== JSON.stringify(selected.data)) {
        selected = { type: "colima", data: fresh };
      }
    }
  });

  async function handleAction(profile: string, action: "start" | "stop" | "restart" | "delete", config?: StartConfig) {
    if (action === "delete") {
      confirm = {
        title: "Delete Instance", danger: true, confirmLabel: "Delete",
        message: `Are you sure you want to delete instance "${profile}"? This action cannot be undone.`,
        onConfirm: async () => {
          confirm = null; actionLoading = `${profile}-delete`;
          try { 
            await colimaApi.deleteInstance(profile, true); 
            globalToast("success", `Instance '${profile}' deleted`); 
            selected = null; 
            onRefresh(); 
          } catch (e) { globalToast("error", String(e)); }
          finally { actionLoading = null; }
        },
        onCancel: () => { confirm = null; }
      };
      return;
    }

    const labels: any = { start: "Starting", stop: "Stopping", restart: "Restarting" };
    globalToast("success", `${labels[action]} instance '${profile}'...`);
    actionLoading = `${profile}-${action}`;

    let finalConfig = config;
    if (!finalConfig) {
      const inst = dashboardState.colimaInstances.find(i => i.name === profile);
      finalConfig = {
        profile,
        runtime: inst?.runtime || "docker",
        cpus: inst?.cpus || 2,
        memory: inst?.memory || 2,
        disk: inst?.disk || 60,
        vm_type: "",
        kubernetes: inst?.kubernetes || false,
        kubernetes_version: "",
        arch: inst?.arch || "",
        mount_type: "",
        mounts: [],
        dns: [],
        network_address: false,
      };
    }

    try {
      if (action === "start") {
        await colimaApi.startInstance(finalConfig);
        await handleInstanceStarted(profile, getCurrentPresetForInstance(profile));
      } else if (action === "stop") {
        await handleInstanceStopping(profile);
        await colimaApi.stopInstance(profile);
      } else if (action === "restart") {
        await handleInstanceStopping(profile);
        await colimaApi.stopInstance(profile);
        await colimaApi.startInstance(finalConfig);
        await handleInstanceStarted(profile, getCurrentPresetForInstance(profile));
      }
      const past: any = { start: "started", stop: "stopped", restart: "restarted" };
      globalToast("success", `Instance '${profile}' ${past[action]}`);
      onRefresh();
    } catch (e) {
      globalToast("error", `${action} failed: ${e}`);
    } finally {
      actionLoading = null;
    }
  }

  async function handleDeleteKind(name: string) {
    confirm = {
      title: "Delete Kind Cluster", danger: true, confirmLabel: "Delete",
      message: `Delete Kind cluster "${name}"? This cannot be undone.`,
      onConfirm: async () => {
        confirm = null; actionLoading = `kind-${name}-delete`;
        try { 
          await kindApi.delete(name); 
          globalToast("success", `Kind cluster "${name}" deleted`); 
          selected = null; 
          fetchKind(); 
        } catch (e) { globalToast("error", String(e)); }
        finally { actionLoading = null; }
      },
    };
  }

  let runningColima = $derived(dashboardState.colimaInstances.filter(i => i.status === "Running").length);
  let totalItems = $derived(dashboardState.colimaInstances.length + kindClusters.length);

  let customPresets = $state<any[]>([]);
  onMount(() => {
    const saved = getAppSetting("ColimaCustomProfiles");
    if (saved) {
      try {
        customPresets = JSON.parse(saved);
      } catch {}
    }
  });

  let allPresets = $derived([...BUILT_IN_PRESETS, ...customPresets]);
</script>

<div class="content-header" data-tauri-drag-region>
  <h1>
    {t('instances.title', { default: 'Instances' })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {runningColima} {t('instances.running', { default: 'running' })} · {totalItems} {t('instances.total', { default: 'total' })}
    </span>
  </h1>
  <div class="content-header-actions">
    <button class="btn btn-ghost" onclick={() => { onRefresh(); fetchKind(); }} style="display: flex; align-items: center; gap: 6px;">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
      {t('instances.refresh', { default: 'Refresh' })}
    </button>
    <button class="btn btn-primary" onclick={() => showCreate = true} style="display: flex; align-items: center; gap: 6px;">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      {t('instances.new', { default: 'New Instance' })}
    </button>
    <button class="btn btn-ghost" onclick={() => showCreateKind = true} style="display: flex; align-items: center; gap: 6px;">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" stroke="url(#kindGrad)" stroke-width="1.5" stroke-linejoin="round"/><circle cx="12" cy="12" r="3" stroke="url(#kindGrad)" stroke-width="1.5"/><defs><linearGradient id="kindGrad" x1="3" y1="2" x2="21" y2="22"><stop stop-color="#a78bfa"/><stop offset="1" stop-color="#7c3aed"/></linearGradient></defs></svg>
      {t('instances.kind', { default: 'Kind Cluster' })}
    </button>
  </div>
</div>

<div class="content-body">
  {#if totalItems === 0 && !kindLoading}
    <div class="empty-state">
      <div class="empty-state-title">{t('instances.empty_title', { default: 'No instances' })}</div>
      <div class="empty-state-text">{t('instances.empty_text', { default: 'Create a Colima VM or Kind cluster to get started.' })}</div>
      <button class="btn btn-primary" onclick={() => showCreate = true} style="display: flex; align-items: center; gap: 6px;"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>{t('instances.new', { default: 'New Instance' })}</button>
    </div>
  {:else}
    <div style="display: grid; grid-template-columns: 320px 1fr; gap: 0; min-height: calc(100vh - 140px);">
      <!-- Left: Item List -->
      <div style="border-right: 1px solid var(--border-primary); overflow-y: auto; background: var(--bg-primary); border-radius: 12px 0 0 12px;">
        <div style="padding: 10px 14px 6px; font-size: 10px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-muted);">
          Colima Instances ({dashboardState.colimaInstances.length})
        </div>
        {#each dashboardState.colimaInstances as inst}
          {@const isRunning = inst.status === "Running"}
          {@const isSelected = selected?.type === "colima" && selected.data.name === inst.name}
          <div role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => selected = { type: "colima", data: inst }} style="padding: 10px 14px; cursor: pointer; display: flex; align-items: center; gap: 10px; background: {isSelected ? 'var(--bg-card-hover)' : 'transparent'}; border-left: 3px solid {isSelected ? 'var(--accent-blue)' : 'transparent'};">
            <div style="width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: {isRunning ? 'var(--accent-green)' : 'var(--text-muted)'}; box-shadow: {isRunning ? '0 0 6px var(--accent-green)' : 'none'};"></div>
            <div style="flex: 1; min-width: 0;">
              <div style="font-weight: 600; font-size: var(--text-sm); color: var(--text-primary);">{inst.name}</div>
              <div style="font-size: 11px; color: var(--text-muted); margin-top: 1px;">{inst.runtime} · {inst.cpus} CPU · {formatBytes(inst.memory)}</div>
            </div>
            <div style="display: flex; flex-direction: column; align-items: flex-end; gap: 4px;">
              <span style="padding: 2px 6px; border-radius: 10px; font-size: 10px; font-weight: 600; background: {isRunning ? 'rgba(63,185,80,0.1)' : 'rgba(139,148,158,0.1)'}; color: {isRunning ? 'var(--accent-green)' : 'var(--text-muted)'};">{inst.status}</span>
              {#if inst.kubernetes}
                <span style="padding: 2px 6px; border-radius: 10px; font-size: 10px; font-weight: 600; background: rgba(56, 189, 248, 0.1); color: var(--accent-blue);">☸️ K3s</span>
              {/if}
            </div>
          </div>
        {/each}

        <div style="padding: 14px 14px 6px; font-size: 10px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-muted); border-top: 1px solid var(--border-primary); margin-top: 4px;">
          Kind Clusters ({kindLoading ? "..." : kindClusters.length})
        </div>
        {#if kindLoading}
          <div style="display: flex; justify-content: center; padding: 16px;"><div class="spinner" style="width: 16px; height: 16px;"></div></div>
        {:else}
          {#each kindClusters as name}
            {@const isSelected = selected?.type === "kind" && selected.name === name}
            <div role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => selected = { type: "kind", name }} style="padding: 10px 14px; cursor: pointer; display: flex; align-items: center; gap: 10px; background: {isSelected ? 'rgba(167,139,250,0.08)' : 'transparent'}; border-left: 3px solid {isSelected ? 'var(--accent-purple)' : 'transparent'};">
              <div style="flex: 1; min-width: 0;">
                <div style="font-weight: 600; font-size: var(--text-sm); color: var(--text-primary); font-family: var(--font-mono);">{name}</div>
                <div style="font-size: 11px; color: var(--text-muted); margin-top: 1px;">kind-{name}</div>
              </div>
              <span style="padding: 2px 6px; border-radius: 10px; font-size: 10px; font-weight: 600; background: rgba(63,185,80,0.1); color: var(--accent-green);">Running</span>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Right: Detail Panel -->
      <div style="padding: 24px; overflow-y: auto; background: var(--bg-secondary); border-radius: 0 12px 12px 0;">
        {#if !selected}
          <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; color: var(--text-muted);">
            <div style="margin-top: 12px; font-size: var(--text-sm);">Select an instance to view details</div>
          </div>
        {:else if selected.type === "colima"}
          <!-- Inline ColimaDetail equivalent for brevity in this rewrite -->
          {@const inst = selected.data}
          {@const profileId = inst.name === "colima" ? "default" : inst.name.replace("colima-", "")}
          {@const isRunning = inst.status === "Running"}
          {@const isLoading = actionLoading?.startsWith(profileId)}
          
          <div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px;">
            <div>
              <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 6px;">
                <div style="width: 12px; height: 12px; border-radius: 50%; background: {isRunning ? 'var(--accent-green)' : 'var(--text-muted)'}; box-shadow: {isRunning ? '0 0 8px var(--accent-green)' : 'none'};"></div>
                <h2 style="margin: 0; font-size: var(--text-xl); font-weight: 700;">{inst.name}</h2>
                <span style="padding: 2px 6px; border-radius: 10px; font-size: 10px; font-weight: 600; background: {allPresets.find(p => p.cpus === inst.cpus && p.memory === inst.memory) ? 'rgba(63,185,80,0.1)' : 'rgba(139,148,158,0.1)'}; color: {allPresets.find(p => p.cpus === inst.cpus && p.memory === inst.memory) ? 'var(--accent-green)' : 'var(--text-muted)'}; margin-left: 8px;">
                  {allPresets.find(p => p.cpus === inst.cpus && p.memory === inst.memory) ? allPresets.find(p => p.cpus === inst.cpus && p.memory === inst.memory)?.label : "Custom"}
                </span>
              </div>
              <div style="font-size: var(--text-xs); color: var(--text-muted); margin-left: 22px;">
                Profile: <span style="font-family: var(--font-mono); color: var(--accent-blue);">{profileId}</span>
              </div>
            </div>
            <div style="display: flex; gap: 6px;">
              {#if isRunning}
                <button class="btn btn-ghost" disabled={!!isLoading} onclick={() => handleAction(profileId, "stop")} style="font-size: var(--text-xs);">Stop</button>
                <button class="btn btn-ghost" disabled={!!isLoading} onclick={() => handleAction(profileId, "restart")} style="font-size: var(--text-xs);">Restart</button>
              {:else}
                <button class="btn btn-primary" disabled={!!isLoading} onclick={() => handleAction(profileId, "start")} style="font-size: var(--text-xs);">Start</button>
              {/if}
              <button class="btn btn-ghost" disabled={!!isLoading} onclick={() => handleAction(profileId, "delete")} style="font-size: var(--text-xs); color: var(--accent-red);">Delete</button>
            </div>
          </div>

          <!-- Resource Stats -->
          <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px; margin-bottom: 24px;">
            <div style="padding: 14px 16px; border-radius: 10px; background: var(--bg-primary); border-left: 3px solid var(--accent-blue);">
              <div style="font-size: 10px; color: var(--text-muted); font-weight: 600; margin-bottom: 4px; text-transform: uppercase;">CPUs</div>
              <div style="font-size: var(--text-lg); font-weight: 700; font-family: var(--font-mono); color: var(--accent-blue);">{inst.cpus}</div>
            </div>
            <div style="padding: 14px 16px; border-radius: 10px; background: var(--bg-primary); border-left: 3px solid var(--accent-green);">
              <div style="font-size: 10px; color: var(--text-muted); font-weight: 600; margin-bottom: 4px; text-transform: uppercase;">Memory</div>
              <div style="font-size: var(--text-lg); font-weight: 700; font-family: var(--font-mono); color: var(--accent-green);">{formatBytes(inst.memory)}</div>
            </div>
            <div style="padding: 14px 16px; border-radius: 10px; background: var(--bg-primary); border-left: 3px solid var(--accent-orange);">
              <div style="font-size: 10px; color: var(--text-muted); font-weight: 600; margin-bottom: 4px; text-transform: uppercase;">Disk</div>
              <div style="font-size: var(--text-lg); font-weight: 700; font-family: var(--font-mono); color: var(--accent-orange);">{formatBytes(inst.disk)}</div>
            </div>
          </div>
        {:else if selected.type === "kind"}
          {@const name = selected.name}
          <div style="display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px;">
            <div>
              <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 6px;">
                <h2 style="margin: 0; font-size: var(--text-xl); font-weight: 700; font-family: var(--font-mono);">{name}</h2>
              </div>
            </div>
            <button class="btn btn-ghost" onclick={() => handleDeleteKind(name)} disabled={actionLoading === `kind-${name}-delete`} style="font-size: var(--text-xs); color: var(--accent-red);">Delete</button>
          </div>
        {/if}
      </div>
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

{#if showCreateKind}
  <!-- Simplified Create Kind Modal -->
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => showCreateKind = false}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()} style="width: 460px;">
      <div class="modal-header"><h2 class="modal-title">Create Kind Cluster</h2></div>
      <div class="modal-footer">
        <button class="btn btn-ghost" onclick={() => showCreateKind = false}>Cancel</button>
        <button class="btn btn-primary" onclick={() => {
          showCreateKind = false;
          globalToast("success", "Creating Kind cluster...");
          kindApi.create("my-cluster", "").then(() => { fetchKind(); onRefresh(); });
        }}>Create</button>
      </div>
    </div>
  </div>
{/if}

{#if showCreate}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => { if (e.target === e.currentTarget) showCreate = false; }}>
    <div class="modal" style="width: min(560px, 95vw); box-shadow: 0 20px 40px rgba(0,0,0,0.5);">
      <div class="modal-header"><h2 class="modal-title">Create Colima Instance</h2></div>
      
      <div style="padding: 20px; display: flex; flex-direction: column; gap: 16px; overflow-y: auto; max-height: 70vh;">
        <div>
          <label for="profileName" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">Profile Name</label>
          <input id="profileName" type="text" bind:value={newConfig.profile} placeholder="default" style="width: 100%; padding: 8px 12px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-primary); font-size: var(--text-sm);" />
        </div>

        <div style="background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; padding: 16px;">
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
            <h3 style="margin: 0; font-size: var(--text-sm); font-weight: 600; color: var(--text-primary); display: flex; align-items: center; gap: 6px;">
              Configuration Presets
              {#if aiOptimized}
                <span style="font-size: 10px; background: rgba(56, 189, 248, 0.2); color: #38bdf8; padding: 2px 6px; border-radius: 4px; font-weight: 700;">AI OPTIMIZED</span>
              {/if}
            </h3>
            <button class="btn btn-ghost" onclick={handleAutoDetect} disabled={detecting || aiOptimizing} style="font-size: var(--text-xs); padding: 4px 10px; color: var(--accent-blue); display: flex; align-items: center; gap: 6px;">
              {#if detecting || aiOptimizing}
                <div class="spinner" style="width: 12px; height: 12px; border-color: rgba(56,189,248,0.3); border-top-color: #38bdf8;"></div>
                {aiOptimizing ? "AI Optimizing..." : "Detecting..."}
              {:else}
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/></svg>
                Auto-Detect Hardware
              {/if}
            </button>
          </div>
          
          <div style="display: grid; grid-template-columns: repeat(5, 1fr); gap: 8px;">
            {#each presets as p}
              <button 
                onclick={() => applyPreset(p)}
                style="background: var(--bg-primary); border: 1px solid {newConfig.cpus === p.cpus && newConfig.memory === p.memory && newConfig.disk === p.disk ? p.color || 'var(--accent-blue)' : 'var(--border-primary)'}; border-radius: 6px; padding: 8px; text-align: center; cursor: pointer; transition: all 0.2s; box-shadow: {newConfig.cpus === p.cpus && newConfig.memory === p.memory && newConfig.disk === p.disk ? '0 0 0 1px ' + (p.color || 'var(--accent-blue)') : 'none'};">
                <div style="font-size: 11px; font-weight: 600; color: var(--text-secondary); margin-bottom: 4px;">{p.label}</div>
                <div style="font-size: var(--text-sm); font-weight: 700; color: var(--text-primary);">{p.cpus}C / {p.memory}G</div>
                <div style="font-size: 10px; color: var(--text-muted); margin-top: 2px;">{p.disk} GiB</div>
              </button>
            {/each}
          </div>
        </div>

        <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px;">
          <div>
            <label for="colimaCpus" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">CPUs</label>
            <input id="colimaCpus" type="number" bind:value={newConfig.cpus} min="1" max="16" class="input" />
          </div>
          <div>
            <label for="colimaMem" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">Memory (GiB)</label>
            <input id="colimaMem" type="number" bind:value={newConfig.memory} min="1" max="64" class="input" />
          </div>
          <div>
            <label for="colimaDisk" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">Disk (GiB)</label>
            <input id="colimaDisk" type="number" bind:value={newConfig.disk} min="10" max="500" class="input" />
          </div>
        </div>

        <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px;">
          <div>
            <label for="colimaRuntime" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">Runtime</label>
            <select id="colimaRuntime" bind:value={newConfig.runtime} class="input select">
              <option value="docker">Docker</option><option value="containerd">Containerd</option><option value="incus">Incus</option>
            </select>
          </div>
          <div>
            <label for="colimaVmType" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">VM Type</label>
            <select id="colimaVmType" bind:value={newConfig.vm_type} class="input select">
              <option value="vz">VZ (macOS 13+)</option><option value="qemu">QEMU</option>
            </select>
            {#if qemuMissing}
              <div style="font-size: 11px; color: var(--accent-red); margin-top: 6px; display: flex; align-items: flex-start; gap: 4px;">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="flex-shrink: 0; margin-top: 1px;"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
                <span>qemu is missing. Run <code>brew install qemu</code></span>
              </div>
            {/if}
          </div>
        </div>

        <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px;">
          <div>
            <label for="colimaArch" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">Architecture</label>
            <select id="colimaArch" bind:value={newConfig.arch} class="input select">
              <option value="">Default (host)</option><option value="aarch64">aarch64 (ARM64)</option><option value="x86_64">x86_64 (Intel)</option>
            </select>
          </div>
          <div>
            <label for="colimaMountType" style="display: block; font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); margin-bottom: 6px;">Mount Type</label>
            <select id="colimaMountType" bind:value={newConfig.mount_type} class="input select">
              <option value="">Default</option><option value="virtiofs">VirtioFS (macOS)</option><option value="sshfs">SSHFS</option><option value="9p">9P</option>
            </select>
          </div>
        </div>

        <div style="border-top: 1px solid var(--border-primary); padding-top: 16px; margin-top: 4px;">
          <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 12px;">
            <input type="checkbox" id="k8s-check" bind:checked={newConfig.kubernetes} class="checkbox" />
            <label for="k8s-check" style="font-size: var(--text-sm); color: var(--text-primary); margin: 0; cursor: pointer;">Enable Kubernetes (K3s)</label>
          </div>
          {#if newConfig.kubernetes}
            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-left: 24px; margin-bottom: 12px;">
              <div>
                <label for="roleSelect" style="display: block; font-size: 11px; font-weight: 500; color: var(--text-secondary); margin-bottom: 4px;">Cluster Role</label>
                <select id="roleSelect" bind:value={instanceRole} class="input select" style="padding: 4px 8px; font-size: 12px;" onchange={() => {
                  if (instanceRole === 'worker') {
                    newConfig.memory = Math.min(newConfig.memory, 2); // Default 2GB for worker
                    if (!selectedMaster && dashboardState.colimaInstances.some(i => i.kubernetes)) {
                      selectedMaster = dashboardState.colimaInstances.find(i => i.kubernetes)?.name || "";
                    }
                  } else {
                    newConfig.memory = Math.max(newConfig.memory, 4); // Default 4GB for master
                  }
                }}>
                  <option value="standalone">Standalone / Master</option>
                  <option value="worker">Worker Node</option>
                </select>
              </div>
              {#if instanceRole === 'worker'}
                <div>
                  <label for="masterSelect" style="display: block; font-size: 11px; font-weight: 500; color: var(--text-secondary); margin-bottom: 4px;">Join Master Node</label>
                  <select id="masterSelect" bind:value={selectedMaster} class="input select" style="padding: 4px 8px; font-size: 12px;">
                    {#each dashboardState.colimaInstances.filter(i => i.kubernetes) as m}
                      <option value={m.name}>{m.name}</option>
                    {/each}
                  </select>
                </div>
              {/if}
            </div>
          {/if}
          <div style="display: flex; align-items: center; gap: 10px;">
            <input type="checkbox" id="net-addr" bind:checked={newConfig.network_address} class="checkbox" />
            <label for="net-addr" style="font-size: var(--text-sm); color: var(--text-primary); margin: 0; cursor: pointer;">Reachable Network Address (Required for cluster)</label>
          </div>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-ghost" onclick={() => showCreate = false}>Cancel</button>
        <button class="btn btn-primary" disabled={qemuMissing || qemuChecking || (instanceRole === 'worker' && !selectedMaster)} onclick={() => {
          showCreate = false;
          
          if (instanceRole === 'worker') {
            globalToast("success", `Starting worker node joined to ${selectedMaster}...`);
            colimaApi.createWorkerNode(selectedMaster, newConfig.profile, newConfig.cpus, newConfig.memory)
              .then(onRefresh)
              .catch(err => {
                globalToast("error", `Failed to create worker: ${err}`);
                onRefresh();
              });
          } else {
            globalToast("success", "Starting new instance...");
            // Automatically enable network_address for standalone if we plan to add workers later
            colimaApi.startInstance(newConfig)
              .then(onRefresh)
              .catch(err => {
                globalToast("error", `Failed to create instance: ${err}`);
                onRefresh();
              });
          }
        }} style="display: flex; align-items: center; gap: 6px;">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          Create Instance
        </button>
      </div>
    </div>
  </div>
{/if}

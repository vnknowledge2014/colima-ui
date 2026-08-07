<script lang="ts">
  import { dashboardState } from "../store.svelte";
  import Icon from "../components/Icon.svelte";
  import { getApiToken } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import TerminalInstance from "./TerminalInstance.svelte";

  const API_BASE = "http://127.0.0.1:11420";

  async function authHeaders(): Promise<Record<string, string>> {
    const token = await getApiToken();
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (token) {
      headers["Authorization"] = `Bearer ${token}`;
    }
    return headers;
  }

  interface TerminalTab {
    id: string;
    label: string;
    profile: string;
    vmType: "colima" | "lima";
  }

  const termTheme = {
    background: "#0D1117",
    foreground: "#E6EDF3",
    cursor: "#58A6FF",
    selectionBackground: "rgba(88, 166, 255, 0.3)",
    black: "#0D1117",
    red: "#F85149",
    green: "#3FB950",
    yellow: "#D29922",
    blue: "#58A6FF",
    magenta: "#BC8CFF",
    cyan: "#39D2C0",
    white: "#E6EDF3",
    brightBlack: "#6E7681",
    brightRed: "#F85149",
    brightGreen: "#3FB950",
    brightYellow: "#D29922",
    brightBlue: "#58A6FF",
    brightMagenta: "#BC8CFF",
    brightCyan: "#39D2C0",
    brightWhite: "#FFFFFF",
  };

  let tabs = $state<TerminalTab[]>([]);
  let activeTab = $state<string | null>(null);
  let showPicker = $state(false);

  let instances = $derived(dashboardState.colimaInstances.filter(i => i.status === "Running"));
  let limaVMs = $derived(dashboardState.linuxVMs.filter(v => v.status === "Running"));

  let hasRunning = $derived(instances.length > 0 || limaVMs.length > 0);

  function addTab(profile: string, vmType: "colima" | "lima" = "colima") {
    const id = `term-${Date.now()}`;
    const label = vmType === "lima" ? `🐧 ${profile}` : (profile === "default" ? "colima" : profile);
    tabs = [...tabs, { id, label, profile, vmType }];
    activeTab = id;
    showPicker = false;
  }

  function removeTab(id: string) {
    const next = tabs.filter(t => t.id !== id);
    if (activeTab === id) {
      activeTab = next.length > 0 ? next[next.length - 1].id : null;
    }
    tabs = next;
  }
</script>

<div class="content-header">
  <h1>{t('terminal.title', { default: 'Terminal' })}</h1>
  <div class="content-header-actions">
    <button class="btn btn-primary" onclick={() => {
      if (instances.length + limaVMs.length === 1) {
        if (instances.length === 1) {
          const name = instances[0].name;
          addTab(name === "colima" ? "default" : name.replace("colima-", ""), "colima");
        } else {
          addTab(limaVMs[0].name, "lima");
        }
      } else {
        showPicker = true;
      }
    }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
      </svg>
      {t('terminal.new_session', { default: 'New Session' })}
    </button>
  </div>
</div>

<!-- Tab Bar -->
{#if tabs.length > 0}
  <div style="display: flex; align-items: center; gap: 0; border-bottom: 1px solid var(--border-primary); background: var(--bg-content); padding-left: 12px; flex-shrink: 0; overflow: auto;">
    {#each tabs as tab (tab.id)}
      <div
        style="display: flex; align-items: center; gap: 6px; padding: 8px 12px; border-bottom: {activeTab === tab.id ? '2px solid var(--accent-blue)' : '2px solid transparent'}; color: {activeTab === tab.id ? 'var(--text-primary)' : 'var(--text-secondary)'}; cursor: pointer; font-size: var(--text-sm); font-weight: {activeTab === tab.id ? 600 : 400}; white-space: nowrap; transition: all 150ms;"
        onclick={() => activeTab = tab.id}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
        </svg>
        {tab.label}
        <span
          style="margin-left: 4px; opacity: 0.5; cursor: pointer; font-size: var(--text-xs);"
          onclick={(e) => { e.stopPropagation(); removeTab(tab.id); }}
        >
          <Icon name="Close" size={10} />
        </span>
      </div>
    {/each}
  </div>
{/if}

<!-- Terminal Content -->
<div style="flex: 1; overflow: hidden; background: #0D1117; display: flex; flex-direction: column;">
  {#if tabs.length === 0}
    <div class="empty-state" style="height: 100%;">
      <div class="empty-state-icon">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="color: var(--text-muted);">
          <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
        </svg>
      </div>
      <div class="empty-state-title">{t('terminal.no_sessions', { default: 'No terminal sessions' })}</div>
      <div class="empty-state-text">
        {t('terminal.instructions', { default: 'Open a new SSH session to a running instance.' })}
      </div>
      {#if hasRunning}
        <button class="btn btn-primary" onclick={() => {
          if (instances.length + limaVMs.length === 1) {
            if (instances.length === 1) {
              const name = instances[0].name;
              addTab(name === "colima" ? "default" : name.replace("colima-", ""), "colima");
            } else {
              addTab(limaVMs[0].name, "lima");
            }
          } else {
            showPicker = true;
          }
        }}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          New Session
        </button>
      {:else}
        <p style="font-size: var(--text-xs); color: var(--text-muted);">
          No running instances. Start an instance first.
        </p>
      {/if}
    </div>
  {:else}
    {#each tabs as tab (tab.id)}
      <div style="display: {activeTab === tab.id ? 'block' : 'none'}; flex: 1; position: relative;">
        <!-- BrowserTerminalInstance component logic embedded -->
        {#if tab.id}
          {@const sessionId = tab.id}
          {@const profile = tab.profile}
          {@const vmType = tab.vmType}
          <!-- Create a nested wrapper component for each terminal instance -->
          <TerminalInstance {sessionId} {profile} {vmType} active={activeTab === tab.id} {termTheme} {authHeaders} {API_BASE} />
        {/if}
      </div>
    {/each}
  {/if}
</div>

<!-- Instance Picker Modal -->
{#if showPicker}
  <div role="button" tabindex="0" class="modal-overlay" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={() => showPicker = false}>
    <div role="button" tabindex="0" class="modal" onkeydown={(e) => e.key === 'Enter' && e.currentTarget.click()} onclick={(e) => e.stopPropagation()} style="width: min(400px, 90vw);">
      <div class="modal-header">
        <h2 class="modal-title">Select Instance</h2>
        <button class="btn btn-icon btn-ghost" onclick={() => showPicker = false}><Icon name="Close" size={16} /></button>
      </div>
      <div style="display: flex; flex-direction: column; gap: 4px;">
        {#if instances.length > 0}
          <div style="font-size: var(--text-xs); color: var(--text-muted); padding: 8px 16px 4px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;">
            Colima Instances
          </div>
        {/if}
        {#each instances as inst}
          <div
            class="nav-item"
            style="padding: 12px 16px; border-radius: var(--radius-md);"
            onclick={() => addTab(inst.name === "colima" ? "default" : inst.name.replace("colima-", ""), "colima")}
          >
            <div style="width: 8px; height: 8px; border-radius: 50%; background: var(--status-running); box-shadow: 0 0 6px var(--status-running);"></div>
            <div>
              <div style="font-weight: 500;">{inst.name}</div>
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                {inst.runtime} · {inst.arch} · {inst.cpus} CPU
              </div>
            </div>
          </div>
        {/each}
        {#if limaVMs.length > 0}
          <div style="font-size: var(--text-xs); color: var(--text-muted); padding: 8px 16px 4px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; border-top: {instances.length > 0 ? '1px solid var(--border-primary)' : 'none'}; margin-top: {instances.length > 0 ? 4 : 0}px;">
            Linux VMs (Lima)
          </div>
        {/if}
        {#each limaVMs as vm}
          <div
            class="nav-item"
            style="padding: 12px 16px; border-radius: var(--radius-md);"
            onclick={() => addTab(vm.name, "lima")}
          >
            <div style="width: 8px; height: 8px; border-radius: 50%; background: var(--status-running); box-shadow: 0 0 6px var(--status-running);"></div>
            <div>
              <div style="font-weight: 500;">🐧 {vm.name}</div>
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                {vm.arch} · {vm.cpus} CPU · {vm.memory}
              </div>
            </div>
          </div>
        {/each}
        {#if !hasRunning}
          <p style="text-align: center; color: var(--text-muted); padding: 20px; font-size: var(--text-sm);">
            No running instances available.
          </p>
        {/if}
      </div>
    </div>
  </div>
{/if}

<script lang="ts">
  import { onMount } from "svelte";
  import { dashboardState, uiState } from "../store.svelte";
  import Icon from "../components/Icon.svelte";
  import { t } from "../lib/i18n.svelte";
  import { k8sApi } from "../lib/api";
  import { parseItems } from "../lib/k8sUtils";
  import { sessionLabel, type SessionKind } from "../lib/terminal-transport";
  import TerminalInstance from "./TerminalInstance.svelte";

  // The API origin and bearer token used to be threaded down to every tab,
  // because each keystroke was an authenticated HTTP request. Sessions run over
  // Tauri IPC now, so there is no origin to resolve and no token to pass.

  interface TerminalTab {
    id: string;
    label: string;
    kind: SessionKind;
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

  /**
   * Container targets from the connected cluster.
   *
   * The picker used to list only Colima/Lima VMs, so on a machine whose
   * Kubernetes comes from somewhere else — OrbStack, Docker Desktop, a remote
   * context — the Terminal page offered nothing to open, even though the
   * terminal itself can attach to any container. Loaded lazily: the page should
   * not pay for a cluster query nobody asked for.
   */
  interface PodTarget {
    namespace: string;
    pod: string;
    container: string;
    label: string;
  }

  let podTargets = $state<PodTarget[]>([]);
  let podsLoading = $state(false);
  let podsError = $state("");

  // On mount, not on picker-open: `hasRunning` gates whether the New Session
  // button renders at all, so the cluster has to be counted before the user
  // reaches for it.
  onMount(loadPodTargets);

  async function loadPodTargets() {
    if (podsLoading) return;
    podsLoading = true;
    podsError = "";
    try {
      const pods = parseItems(await k8sApi.pods("")).filter(
        (p) => p.status === "Running",
      );

      podTargets = pods.flatMap((p) => {
        const containers: string[] =
          (p._raw as any)?.spec?.containers?.map((c: any) => c.name) ?? [];
        const ns = p.namespace || "default";

        // One row per container only when there is a choice to make; a
        // single-container pod does not need the name spelled out.
        if (containers.length <= 1) {
          return [{ namespace: ns, pod: p.name, container: "", label: `${ns}/${p.name}` }];
        }
        return containers.map((c) => ({
          namespace: ns,
          pod: p.name,
          container: c,
          label: `${ns}/${p.name} · ${c}`,
        }));
      });
    } catch (e) {
      // No cluster is a normal state, not an error worth a toast.
      podsError = String(e);
      podTargets = [];
    } finally {
      podsLoading = false;
    }
  }

  /** Anything at all that a session could attach to. */
  let hasRunning = $derived(
    instances.length > 0 || limaVMs.length > 0 || podTargets.length > 0,
  );

  function addTab(profile: string, vmType: "colima" | "lima" = "colima") {
    openSession(
      vmType === "lima"
        ? { kind: "lima", instance: profile }
        : { kind: "colima", profile },
    );
  }

  /**
   * Skip the picker when there is only one thing to pick.
   *
   * Counts every source, not just VMs — otherwise a machine with one pod and no
   * VM would open a dialog containing a single row.
   */
  function openOrPick() {
    const total = instances.length + limaVMs.length + podTargets.length;
    if (total !== 1) {
      showPicker = true;
      return;
    }

    if (instances.length === 1) {
      const name = instances[0].name;
      addTab(name === "colima" ? "default" : name.replace("colima-", ""), "colima");
    } else if (limaVMs.length === 1) {
      addTab(limaVMs[0].name, "lima");
    } else {
      openPod(podTargets[0]);
    }
  }

  function openPod(p: PodTarget) {
    openSession({
      kind: "k8sExec",
      namespace: p.namespace,
      pod: p.pod,
      container: p.container,
    });
  }

  /**
   * Open a tab for any session kind.
   *
   * The id is minted once here and never re-derived, so remounting a tab
   * reattaches to its existing pty instead of spawning a second one.
   */
  function openSession(kind: SessionKind) {
    const id = `term-${Date.now()}`;
    tabs = [...tabs, { id, label: sessionLabel(kind), kind }];
    activeTab = id;
    showPicker = false;
  }

  // Consume the one-shot deep link set by `openTerminalSession` — today the
  // Kubernetes pod drawer's shell button. Cleared immediately so returning to
  // this page later does not reopen a stale session.
  $effect(() => {
    const pending = uiState.pendingTerminalSession as SessionKind | null;
    if (!pending) return;
    uiState.pendingTerminalSession = null;
    openSession(pending);
  });

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
          openOrPick();
        }}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          New Session
        </button>
      {:else}
        <p style="font-size: var(--text-xs); color: var(--text-muted); line-height: 1.7;">
          {#if podsLoading}
            Looking for targets…
          {:else}
            No running VM and no cluster container.<br />
            Run <code>colima start</code>, or connect a Kubernetes cluster.
          {/if}
        </p>
      {/if}
    </div>
  {:else}
    {#each tabs as tab (tab.id)}
      <div style="display: {activeTab === tab.id ? 'block' : 'none'}; flex: 1; position: relative;">
        <TerminalInstance
          sessionId={tab.id}
          kind={tab.kind}
          active={activeTab === tab.id}
          {termTheme}
        />
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
        {#if podTargets.length > 0}
          <div style="font-size: var(--text-xs); color: var(--text-muted); padding: 8px 16px 4px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; border-top: {instances.length + limaVMs.length > 0 ? '1px solid var(--border-primary)' : 'none'}; margin-top: {instances.length + limaVMs.length > 0 ? 4 : 0}px;">
            Kubernetes containers
          </div>
        {/if}
        {#each podTargets as p (p.label)}
          <div
            class="nav-item"
            style="padding: 12px 16px; border-radius: var(--radius-md);"
            onclick={() => openPod(p)}
          >
            <div style="width: 8px; height: 8px; border-radius: 50%; background: var(--status-running); box-shadow: 0 0 6px var(--status-running);"></div>
            <div style="min-width: 0;">
              <div style="font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{p.pod}</div>
              <div style="font-size: var(--text-xs); color: var(--text-muted);">
                {p.namespace}{p.container ? ` · ${p.container}` : ""}
              </div>
            </div>
          </div>
        {/each}

        {#if podsLoading && !hasRunning}
          <p style="text-align: center; color: var(--text-muted); padding: 20px; font-size: var(--text-sm);">
            Looking for targets…
          </p>
        {:else if !hasRunning}
          <!-- Says which sources were checked and what to do, rather than
               implying the only possible target is a Colima VM. -->
          <p style="text-align: center; color: var(--text-muted); padding: 20px 16px; font-size: var(--text-sm); line-height: 1.6;">
            Nothing to connect to.<br />
            Start a VM with <code>colima start</code>, or connect a Kubernetes
            cluster to open a shell in a container.
          </p>
        {/if}

        {#if podsError}
          <!-- "The cluster query failed" and "the cluster has no pods" look
               identical from the empty list, so the failure has to say so. -->
          <p style="padding: 0 16px 16px; font-size: var(--text-xs); color: var(--accent-yellow); line-height: 1.6;">
            Could not list cluster containers: {podsError}
          </p>
        {/if}
      </div>
    </div>
  </div>
{/if}

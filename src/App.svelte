<script lang="ts">
  import { onMount } from "svelte";
  import { colimaApi, dockerApi, volumesApi, networksApi, type ColimaInstance, type SystemInfo, sysMethods, getApiToken } from "./lib/api";
  import { normalizeContainer, normalizeImage } from "./lib/normalizers";
  import { onToast, type ToastMessage } from "./lib/globalToast";
  import * as Icons from "./components/Icons.svelte";
  
  import { dockerState, resourceState, dashboardState, uiState, isEventCooldownActive } from "./store.svelte";
  import { appState, loadAllSettings, setAppSetting, getAppSetting } from "./lib/settingsStore.svelte";
  import { t } from "./lib/i18n.svelte";

  import Dashboard from "./pages/Dashboard.svelte";

  function handleNavigateEvent(e: Event) {
    const customEvent = e as CustomEvent;
    if (customEvent.detail) {
      uiState.currentPage = customEvent.detail;
    }
  }

  import Instances from "./pages/Instances.svelte";
  import Containers from "./pages/Containers.svelte";
  // import TerminalPage from "./pages/Terminal.svelte";
  import Models from "./pages/Models.svelte";
  import Images from "./pages/Images.svelte";
  import Volumes from "./pages/Volumes.svelte";
  import Networks from "./pages/Networks.svelte";
  import Compose from "./pages/Compose.svelte";
  import Kubernetes from "./pages/Kubernetes.svelte";
  import LinuxVMs from "./pages/LinuxVMs.svelte";
  import Settings from "./pages/Settings.svelte";
  import SetupWizard from "./components/SetupWizard.svelte";
  import GettingStartedTour from "./components/GettingStartedTour.svelte";
  import ErrorBoundary from "./components/ErrorBoundary.svelte";
  import AiChatPanel from "./components/AiChatPanel.svelte";
  import Terminal from "./pages/Terminal.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";

  type Page = "dashboard" | "instances" | "containers" | "images" | "volumes" | "networks" | "compose" | "kubernetes" | "linux-vms" | "terminal" | "models" | "settings" | "ai-chat";

  const isTauri = !!(window as any).__TAURI_INTERNALS__;

  let systemInfo = $state<SystemInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let currentTime = $state(new Date());
  let toasts = $state<ToastMessage[]>([]);

  let showWizard = $state(false);
  let showTour = $state(false);

  $effect(() => {
    if (appState.isSettingsLoaded) {
      if (getAppSetting("colimaui_setup_complete") !== "true" && !showWizard && !showTour) {
        showWizard = true;
      }
    }
  });

  const formatTime = () => currentTime.toLocaleTimeString();

  // Dynamic labels for sidebar
  const navGroups = $derived([
    {
      label: t("sidebar.overview", { default: "Overview" }),
      items: [
        { id: "dashboard", label: t("sidebar.dashboard", { default: "Dashboard" }), icon: Icons.Dashboard },
        { id: "instances", label: t("sidebar.instances", { default: "Instances" }), icon: Icons.Server },
      ],
    },
    {
      label: t("sidebar.docker", { default: "Docker" }),
      items: [
        { id: "containers", label: t("sidebar.containers", { default: "Containers" }), icon: Icons.Container },
        { id: "images", label: t("sidebar.images", { default: "Images" }), icon: Icons.Container },
        { id: "volumes", label: t("sidebar.volumes", { default: "Volumes" }), icon: Icons.Volume },
        { id: "networks", label: t("sidebar.networks", { default: "Networks" }), icon: Icons.Network },
        { id: "compose", label: t("sidebar.compose", { default: "Compose" }), icon: Icons.Compose },
      ],
    },
    {
      label: t("sidebar.clusters_vms", { default: "Clusters & VMs" }),
      items: [
        { id: "kubernetes", label: t("sidebar.kubernetes", { default: "Kubernetes" }), icon: Icons.Kubernetes },
        { id: "linux-vms", label: t("sidebar.linux_vms", { default: "Linux VMs" }), icon: Icons.LinuxVM },
      ],
    },
    {
      label: t("sidebar.ai_tools", { default: "AI & Tools" }),
      items: [
        { id: "models", label: t("sidebar.local_ai_models", { default: "Local AI Models" }), icon: Icons.Models },
        { id: "ai-chat" as Page, label: t("sidebar.ai_chat", { default: "AI Assistant" }), icon: Icons.AiCenter },
        { id: "terminal", label: t("sidebar.terminal", { default: "Terminal" }), icon: Icons.Terminal },
        { id: "settings", label: t("sidebar.settings", { default: "Settings" }), icon: Icons.Settings },
      ],
    },
  ]);

  async function refreshManual() {
    try {
      error = null;
      const [instanceList, sysInfo] = await Promise.all([
        colimaApi.listInstances().catch(() => [] as ColimaInstance[]),
        sysMethods.checkSystem().catch(() => null),
      ]);
      dashboardState.colimaInstances = instanceList;
      if (sysInfo) systemInfo = sysInfo;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Refetch all docker resources
  let _refetchRetryCount = 0;
  const MAX_REFETCH_RETRIES = 10;

  async function refetchAllResources() {
    let fetchFailed = false;
    try {
      const [c, i] = await Promise.all([
        dockerApi.listContainers(true),
        dockerApi.listImages(),
      ]);
      dockerState.containers = c;
      dockerState.images = i;
      dockerState.loading = false;
    } catch {
      fetchFailed = true;
    }
    
    try {
      resourceState.volumes = await volumesApi.listVolumes();
      resourceState.volumesLoading = false;
    } catch {
      fetchFailed = true;
    }
    
    try {
      resourceState.networks = await networksApi.listNetworks();
      resourceState.networksLoading = false;
    } catch {
      fetchFailed = true;
    }

    if (fetchFailed) {
      _refetchRetryCount++;
      if (_refetchRetryCount < MAX_REFETCH_RETRIES) {
        setTimeout(refetchAllResources, 5000);
      } else {
        console.warn(`[ColimaUI] refetchAllResources gave up after ${MAX_REFETCH_RETRIES} attempts`);
      }
    } else {
      _refetchRetryCount = 0; // Reset on success
    }
  }

  function handleDockerStateUpdated(data: any) {
    if (isEventCooldownActive()) return;

    dockerState.containers = (data.containers || []).map(normalizeContainer);
    dockerState.images = (data.images || []).map(normalizeImage);
    dockerState.loading = false;
    
    // Quick refetch for secondary
    setTimeout(() => {
      volumesApi.listVolumes().then((v) => { resourceState.volumes = v; resourceState.volumesLoading = false; }).catch(() => {});
      networksApi.listNetworks().then((n) => { resourceState.networks = n; resourceState.networksLoading = false; }).catch(() => {});
    }, 1000);
  }

  function handleConnectionLost() {
    dockerState.containers = [];
    dockerState.images = [];
    resourceState.volumes = [];
    resourceState.networks = [];
    dockerState.loading = true;
    resourceState.volumesLoading = true;
    resourceState.networksLoading = true;
  }

  onMount(() => {
    window.addEventListener('colima-navigate', handleNavigateEvent);

    // Run async setup in an IIFE so onMount returns the cleanup function synchronously
    (async () => {
      await loadAllSettings();
      refreshManual();
      refetchAllResources();
    })();

    // Init Resource Saver Daemon
    const rsEnabled = getAppSetting("colimaui_auto_pause") === "true";
    const rsMins = parseInt(getAppSetting("colimaui_auto_pause_mins") || "15", 10);
    sysMethods.setResourceSaver(rsEnabled, rsMins).catch(() => {});

    // Toasts
    const unToast = onToast((toast) => {
      toasts = [...toasts, toast];
      setTimeout(() => {
        toasts = toasts.filter((t) => t.id !== toast.id);
      }, 5000);
    });

    // Nav listener removed

    // Clocks
    const clockInterval = setInterval(() => { currentTime = new Date(); }, 1000);
    const sysInterval = setInterval(() => { sysMethods.checkSystem().then(s => systemInfo = s).catch(()=>{}); }, 30000);

    // Hoisted for cleanup access across both branches
    let pollInterval: ReturnType<typeof setInterval> | null = null;
    let sseRetryTimeout: ReturnType<typeof setTimeout> | null = null;

    // Platform specific polling / push
    if (isTauri) {
      import("@tauri-apps/api/event").then((mod) => {
        mod.listen("instances-update", (event: any) => {
          dashboardState.colimaInstances = event.payload.instances;
          loading = false;
        });
        mod.listen("docker-state-updated", (event: any) => handleDockerStateUpdated(event.payload));
        mod.listen("docker-connection-lost", handleConnectionLost);
        mod.listen("docker-reconnected", refetchAllResources);
      });
    } else {
      let sseRetryDelay = 2000; // Start at 2s, backoff to max 30s

      function connectSSE(token: string) {
        const sseUrl = token ? `http://127.0.0.1:11420/api/events?token=${token}` : "http://127.0.0.1:11420/api/events";
        const es = new EventSource(sseUrl);
        es.addEventListener("instances-update", (e: any) => {
          try {
            const data = JSON.parse(e.data);
            dashboardState.colimaInstances = data.instances;
            loading = false;
          } catch {}
        });
        es.addEventListener("docker-state-updated", (e: any) => {
          try {
            handleDockerStateUpdated(JSON.parse(e.data));
          } catch {}
        });
        es.addEventListener("docker-connection-lost", handleConnectionLost);
        es.addEventListener("docker-reconnected", refetchAllResources);
        es.onopen = () => {
          // Connection succeeded — reset backoff and stop polling fallback
          sseRetryDelay = 2000;
          if (pollInterval) {
            clearInterval(pollInterval);
            pollInterval = null;
          }
        };
        es.onerror = () => {
          es.close();
          // Start polling as fallback while we wait to reconnect
          if (!pollInterval) {
            pollInterval = setInterval(() => {
              refreshManual();
              refetchAllResources();
            }, 5000);
          }
          // Schedule SSE reconnect with exponential backoff (max 30s)
          sseRetryTimeout = setTimeout(() => {
            connectSSE(token);
          }, sseRetryDelay);
          sseRetryDelay = Math.min(sseRetryDelay * 2, 30000);
        };
      }

      getApiToken().then((token) => {
        connectSSE(token);
      });
    }

    return () => {
      window.removeEventListener('colima-navigate', handleNavigateEvent);
      unToast();
      clearInterval(clockInterval);
      clearInterval(sysInterval);
      if (pollInterval) clearInterval(pollInterval);
      if (sseRetryTimeout) clearTimeout(sseRetryTimeout);
    };
  });
</script>

<ErrorBoundary>
<div class="app-layout {isTauri ? 'tauri-app' : ''}">
  <aside class="sidebar">
    <div class="sidebar-header">
      <img src="/colima_icon.png" alt="ColimaUI" class="sidebar-logo" />
      <h1 class="sidebar-title">ColimaUI</h1>
    </div>

    <nav class="sidebar-nav" data-tour-id="sidebar-nav">
      {#each navGroups as group}
        <div class="nav-section">
          <div class="nav-section-label">{group.label}</div>
          {#each group.items as item}
            <button
              class="nav-item {uiState.currentPage === item.id || (item.id === 'ai-chat' && uiState.aiPanelOpen) ? 'active' : ''}"
              onclick={() => {
                if (item.id === "ai-chat") {
                  uiState.aiPanelOpen = !uiState.aiPanelOpen;
                } else {
                  uiState.currentPage = item.id;
                }
              }}
              data-tour-id={`nav-${item.id}`}
            >
              {@html item.icon}
              <span>{item.label}</span>
            </button>
          {/each}
        </div>
      {/each}
    </nav>

    <div class="sidebar-footer">
      <button
        class="nav-item"
        style="font-size: var(--text-xs); color: var(--text-muted);"
        onclick={() => {
          showTour = true;
          setAppSetting("colimaui_tour_complete", "false");
        }}
        data-tooltip="Restart tour"
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M2 3h6a4 4 0 0 1 4 4v14"/><path d="M22 3h-6a4 4 0 0 0-4 4v14"/><polyline points="6 7 2 3 6 -1"/>
        </svg>
        <span>Tour Guide</span>
      </button>
      <div class="nav-item" style="font-size: var(--text-xs); color: var(--text-muted); cursor: default;">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
        </svg>
        <span>{formatTime()}</span>
      </div>
      <div class="nav-item" style="font-size: var(--text-xs); color: var(--text-muted); cursor: default;">
        {systemInfo?.colima_version
          ? `Colima v${systemInfo.colima_version.split("\n")[0].replace(/.*version\s*/i, "")}`
          : "Colima not detected"}
      </div>
    </div>
  </aside>

  <main class="main-content">
    {#if error}
      <div style="padding: 8px 24px; background: rgba(248, 81, 73, 0.1); color: var(--accent-red); font-size: var(--text-sm); border-bottom: 1px solid var(--border-primary); display: flex; align-items: center; justify-content: space-between;">
        <span style="display: flex; align-items: center; gap: 6px;">{@html Icons.Warning} {error}</span>
        <button class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px;" onclick={() => error = null}>
          Dismiss
        </button>
      </div>
    {/if}

    {#if uiState.currentPage === "dashboard"}
      <Dashboard {systemInfo} {loading} onNavigate={(p) => uiState.currentPage = p} />
    {:else if uiState.currentPage === "instances"}
      <Instances onRefresh={refreshManual} />
    {:else if uiState.currentPage === "containers"}
      <Containers />
    {:else if uiState.currentPage === "images"}
      <Images />
    {:else if uiState.currentPage === "volumes"}
      <Volumes />
    {:else if uiState.currentPage === "networks"}
      <Networks />
    {:else if uiState.currentPage === "compose"}
      <Compose />
    {:else if uiState.currentPage === "kubernetes"}
      <Kubernetes />
    {:else if uiState.currentPage === "linux-vms"}
      <LinuxVMs />
    {:else if uiState.currentPage === "settings"}
      <Settings {systemInfo} />
    {:else if uiState.currentPage === "terminal"}
      <Terminal />
    {:else if uiState.currentPage === "models"}
      <Models />
    {:else}
      <div style="display: flex; justify-content: center; align-items: center; height: 50vh;">
        <div>Component Migration in Progress... ({uiState.currentPage})</div>
      </div>
    {/if}
  </main>

  {#if toasts.length > 0}
    <div class="toast-container">
      {#each toasts as toast (toast.id)}
        <button
          class="toast-item toast-{toast.type}"
          onclick={() => toasts = toasts.filter(t => t.id !== toast.id)}
        >
          {toast.type === 'success' ? '✓' : toast.type === 'error' ? '✕' : 'ℹ'} {toast.text}
        </button>
      {/each}
    </div>
  {/if}

  {#if showWizard}
    <SetupWizard
      {systemInfo}
      onComplete={() => {
        showWizard = false;
        setAppSetting("colimaui_setup_complete", "true");
        if (getAppSetting("colimaui_tour_complete") !== "true") {
          setTimeout(() => showTour = true, 300);
        }
      }}
      onSkip={() => {
        showWizard = false;
        setAppSetting("colimaui_setup_complete", "true");
        if (getAppSetting("colimaui_tour_complete") !== "true") {
          setTimeout(() => showTour = true, 300);
        }
      }}
    />
  {/if}

  {#if showTour}
    <GettingStartedTour
      onComplete={() => {
        showTour = false;
        setAppSetting("colimaui_tour_complete", "true");
      }}
    />
  {/if}

  <AiChatPanel />
  <ConfirmDialog />
</div>
</ErrorBoundary>

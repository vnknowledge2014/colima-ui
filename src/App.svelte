<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { uiState, dashboardState } from "./store.svelte";
  import { appState, loadAllSettings, getAppSetting, setAppSetting } from "./lib/settingsStore.svelte";
  import { startDataPoller, refreshManual } from "./lib/dataPoller";
  import * as Icons from "./components/Icons.svelte";

  import Dashboard from "./pages/Dashboard.svelte";
  import Instances from "./pages/Instances.svelte";
  import Containers from "./pages/Containers.svelte";
  import Models from "./pages/Models.svelte";
  import Images from "./pages/Images.svelte";
  import Volumes from "./pages/Volumes.svelte";
  import Networks from "./pages/Networks.svelte";
  import Compose from "./pages/Compose.svelte";
  import Kubernetes from "./pages/Kubernetes.svelte";
  import LinuxVMs from "./pages/LinuxVMs.svelte";
  import Settings from "./pages/Settings.svelte";
  import Terminal from "./pages/Terminal.svelte";

  import SetupWizard from "./components/SetupWizard.svelte";
  import GettingStartedTour from "./components/GettingStartedTour.svelte";
  import ErrorBoundary from "./components/ErrorBoundary.svelte";
  import AiChatPanel from "./components/AiChatPanel.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  
  import Sidebar from "./components/Sidebar.svelte";
  import ToastContainer from "./components/ToastContainer.svelte";

  const isTauri = !!(window as any).__TAURI_INTERNALS__;

  let showWizard = $state(false);
  let showTour = $state(false);
  let cleanupPoller: (() => void) | null = null;

  $effect(() => {
    if (appState.isSettingsLoaded) {
      if (getAppSetting("colimaui_setup_complete") !== "true" && !showWizard && !showTour) {
        showWizard = true;
      }
    }
  });

  onMount(() => {
    (async () => {
      await loadAllSettings();
      cleanupPoller = startDataPoller();
    })();
  });

  onDestroy(() => {
    if (cleanupPoller) cleanupPoller();
  });
</script>

<ErrorBoundary>
<div class="app-layout {isTauri ? 'tauri-app' : ''}">
  <Sidebar 
    systemInfo={dashboardState.systemInfo} 
    onStartTour={() => { showTour = true; setAppSetting("colimaui_tour_complete", "false"); }} 
  />

  <main class="main-content">
    {#if uiState.globalError}
      <div style="padding: 8px 24px; background: rgba(248, 81, 73, 0.1); color: var(--accent-red); font-size: var(--text-sm); border-bottom: 1px solid var(--border-primary); display: flex; align-items: center; justify-content: space-between;">
        <span style="display: flex; align-items: center; gap: 6px;">{@html Icons.Warning} {uiState.globalError}</span>
        <button class="btn btn-ghost" style="font-size: var(--text-xs); padding: 2px 8px;" onclick={() => uiState.globalError = null}>
          Dismiss
        </button>
      </div>
    {/if}

    {#if uiState.currentPage === "dashboard"}
      <Dashboard systemInfo={dashboardState.systemInfo} loading={false} onNavigate={(p) => uiState.currentPage = p} />
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
      <Settings systemInfo={dashboardState.systemInfo} />
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

  <ToastContainer />

  {#if showWizard}
    <SetupWizard
      systemInfo={dashboardState.systemInfo}
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

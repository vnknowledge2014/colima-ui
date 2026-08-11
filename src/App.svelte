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
  import Help from "./pages/Help.svelte";

  import SetupWizard from "./components/SetupWizard.svelte";
  import GettingStartedTour from "./components/GettingStartedTour.svelte";
  import ErrorBoundary from "./components/ErrorBoundary.svelte";
  import AiChatPanel from "./components/AiChatPanel.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  
  import Sidebar from "./components/Sidebar.svelte";
  import ToastContainer from "./components/ToastContainer.svelte";
  import ErrorDetailPanel from "./components/ErrorDetailPanel.svelte";
  import { isRunningInTauri } from "./lib/env";

  let isTauri = $state(isRunningInTauri());

  // Tauri 2's reliable window drag is the data-tauri-drag-region attribute;
  // plain -webkit-app-region CSS is unreliable in macOS WKWebView with an
  // overlay titlebar. Pages swap headers on navigation, so re-tag on DOM
  // changes instead of tagging each page's markup.
  function windowDragRegion(node: HTMLElement) {
    if (!isRunningInTauri()) return {};
    const tag = () => {
      node.querySelectorAll<HTMLElement>(".content-header, .sidebar-header").forEach((el) => {
        if (!el.hasAttribute("data-tauri-drag-region")) el.setAttribute("data-tauri-drag-region", "");
      });
    };
    tag();
    const observer = new MutationObserver(tag);
    observer.observe(node, { childList: true, subtree: true });
    return { destroy: () => observer.disconnect() };
  }

  let showWizard = $state(false);
  let showTour = $state(false);
  let cleanupPoller: (() => void) | null = null;

  // Sync current tab to localStorage on every navigation so the store can
  // restore it on next load. The initial value is already set from localStorage
  // in store.svelte.ts (module-level, before any reactivity).
  $effect(() => {
    if (uiState.currentPage) {
      localStorage.setItem("colima_active_page", uiState.currentPage);
    }
  });

  $effect(() => {
    if (appState.isSettingsLoaded) {
      if (getAppSetting("colimaui_setup_complete") !== "true" && !showWizard && !showTour) {
        showWizard = true;
      }
    }
  });

  onMount(() => {
    setTimeout(() => {
      if (!isTauri) {
        isTauri = isRunningInTauri();
      }
    }, 250);

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
<div class="app-layout {isTauri ? 'tauri-app' : ''}" use:windowDragRegion>
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
    {:else if uiState.currentPage === "help"}
      <Help />
    {:else}
      <div style="display: flex; justify-content: center; align-items: center; height: 50vh;">
        <div>Component Migration in Progress... ({uiState.currentPage})</div>
      </div>
    {/if}
  </main>

  <!-- A sibling of <main>, not an overlay: the layout is a flex row, so the
       panel claims its own column and the content shrinks instead of being
       covered. -->
  <AiChatPanel />

  <ToastContainer />
  <ErrorDetailPanel />

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

  <ConfirmDialog />
</div>

<!-- ToastContainer is rendered OUTSIDE .app-layout to avoid stacking context
     issues from backdrop-filter on modal-overlay. This guarantees
     z-index: 99999 is always above modals and overlays. -->
<ToastContainer />
</ErrorBoundary>

<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { uiState, dashboardState } from "./store.svelte";
  import { appState, loadAllSettings, getAppSetting, setAppSetting } from "./lib/settingsStore.svelte";
  import { startDataPoller, refreshManual } from "./lib/dataPoller";
  import { startTransferNotifications } from "./lib/transferNotifications";
  import { startAnnouncements } from "./lib/announcements";
  import * as Icons from "./components/Icons.svelte";

  import Dashboard from "./pages/Dashboard.svelte";
  import Instances from "./pages/Instances.svelte";
  import Containers from "./pages/Containers.svelte";
  import Models from "./pages/Models.svelte";
  import Images from "./pages/Images.svelte";
  import Volumes from "./pages/Volumes.svelte";
  import Networks from "./pages/Networks.svelte";
  import Topology from "./pages/Topology.svelte";
  import Activity from "./pages/Activity.svelte";
  import Compose from "./pages/Compose.svelte";
  import Kubernetes from "./pages/Kubernetes.svelte";
  import LinuxVMs from "./pages/LinuxVMs.svelte";
  import Settings from "./pages/Settings.svelte";
  import Terminal from "./pages/Terminal.svelte";
  import Security from "./pages/Security.svelte";
  import Help from "./pages/Help.svelte";

  import SetupWizard from "./components/SetupWizard.svelte";
  import GettingStartedTour from "./components/GettingStartedTour.svelte";
  import ErrorBoundary from "./components/ErrorBoundary.svelte";
  import AiChatPanel from "./components/AiChatPanel.svelte";
  import NotificationPanel from "./components/notifications/NotificationPanel.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import { installCrashReporter } from "./lib/crashReporter";
  
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
  let cleanupTransfers: (() => void) | null = null;
  /**
   * Starts out as a "do not start" switch and becomes the real teardown once
   * polling begins — the start happens after several awaits, so a teardown that
   * arrives first must still have something to find. Without this the interval
   * would be created after the app was gone, with nothing left to clear it.
   */
  let announcementsTornDown = false;
  let cleanupAnnouncements: (() => void) | null = () => {
    announcementsTornDown = true;
  };

  /**
   * Latches true the first time the Terminal page is opened, and never resets.
   *
   * Deferring the first mount keeps xterm out of startup for users who never
   * open a shell; never resetting is the point — see the render block.
   */
  let terminalMounted = $state(false);
  $effect(() => {
    if (uiState.currentPage === "terminal") terminalMounted = true;
  });

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
    // Redact secrets from any uncaught frontend error before it hits the console.
    installCrashReporter();

    setTimeout(() => {
      if (!isTauri) {
        isTauri = isRunningInTauri();
      }
    }, 250);

    // Subscribed synchronously, before any await: a teardown that arrives while
    // settings are still loading must be able to find something to tear down.
    // Transfers outlive the dialog that started them, so this cannot live in a
    // component that unmounts — and in browser mode a per-dialog subscription
    // opened a new EventSource every time one was opened.
    cleanupTransfers = startTransferNotifications();
    (async () => {
      await loadAllSettings();
      cleanupPoller = startDataPoller();
      // Last, and inside the same chain: the first poll needs settings loaded
      // — the off switch and the set of ids already shown live there.
      if (!announcementsTornDown) cleanupAnnouncements = startAnnouncements();
    })();
  });

  onDestroy(() => {
    if (cleanupPoller) cleanupPoller();
    if (cleanupTransfers) cleanupTransfers();
    if (cleanupAnnouncements) cleanupAnnouncements();
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
    {:else if uiState.currentPage === "topology"}
      <Topology />
    {:else if uiState.currentPage === "activity"}
      <Activity />
    {:else if uiState.currentPage === "security"}
      <Security />
    {:else if uiState.currentPage === "compose"}
      <Compose />
    {:else if uiState.currentPage === "kubernetes"}
      <Kubernetes />
    {:else if uiState.currentPage === "linux-vms"}
      <LinuxVMs />
    {:else if uiState.currentPage === "settings"}
      <Settings systemInfo={dashboardState.systemInfo} />
    {:else if uiState.currentPage === "terminal"}
      <!-- Deliberately empty: Terminal is rendered after this chain so that
           leaving the page hides it instead of destroying it. See below. -->
    {:else if uiState.currentPage === "models"}
      <Models />
    {:else if uiState.currentPage === "help"}
      <Help />
    {:else}
      <div style="display: flex; justify-content: center; align-items: center; height: 50vh;">
        <div>Component Migration in Progress... ({uiState.currentPage})</div>
      </div>
    {/if}

    <!-- Terminal sits outside the page chain because every other page can be
         destroyed on navigation and this one cannot. Unmounting it runs each
         TerminalInstance's cleanup, which calls `terminal_close` and kills the
         pty — so opening a shell on pod A, walking back to Kubernetes, and
         opening pod B killed A's session on the way out. The backend has no
         re-attach path (`SessionManager::create` closes and respawns any
         existing id), so the session only survives if the component does.

         `display: contents` keeps the wrapper out of the box tree entirely, so
         Terminal's two root elements lay out exactly as they did when they were
         direct children of <main>. Mounted lazily, then kept forever. -->
    {#if terminalMounted}
      <div style="display: {uiState.currentPage === 'terminal' ? 'contents' : 'none'};">
        <Terminal />
      </div>
    {/if}
  </main>

  <!-- A sibling of <main>, not an overlay: the layout is a flex row, so the
       panel claims its own column and the content shrinks instead of being
       covered. -->
  <AiChatPanel />
  <NotificationPanel />

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

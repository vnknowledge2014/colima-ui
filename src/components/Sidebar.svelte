<script lang="ts">
  import { onMount } from "svelte";
  import * as Icons from "./Icons.svelte";
  import { uiState } from "../store.svelte";
  import { setAppSetting } from "../lib/settingsStore.svelte";
  import { t } from "../lib/i18n.svelte";
  import type { SystemInfo } from "../lib/api";

  let { systemInfo, onStartTour } = $props<{
    systemInfo: SystemInfo | null;
    onStartTour: () => void;
  }>();

  let currentTime = $state(new Date());
  const formatTime = () => currentTime.toLocaleTimeString();

  onMount(() => {
    const clockInterval = setInterval(() => { currentTime = new Date(); }, 1000);
    return () => clearInterval(clockInterval);
  });

  const navGroups = $derived([
    {
      label: t("sidebar.overview", { default: "Overview" }),
      items: [
        { id: "dashboard", label: t("sidebar.dashboard", { default: "Dashboard" }), icon: Icons.Dashboard },
        { id: "instances", label: t("sidebar.colima_instances", { default: "Instances" }), icon: Icons.Instance },
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
        { id: "ai-chat", label: t("sidebar.ai_chat", { default: "AI Assistant" }), icon: Icons.AiCenter },
        { id: "terminal", label: t("sidebar.terminal", { default: "Terminal" }), icon: Icons.Terminal },
        { id: "settings", label: t("sidebar.settings", { default: "Settings" }), icon: Icons.Settings },
      ],
    },
  ]);
</script>

<style>
  .sidebar-nav {
    flex: 1;
    padding: 8px;
    overflow-y: auto;
    /* Hide scrollbar for Firefox and IE */
    scrollbar-width: none;
    -ms-overflow-style: none;
  }
  
  /* Hide scrollbar for Webkit natively using global */
  :global(.sidebar-nav::-webkit-scrollbar) {
    display: none !important;
    width: 0 !important;
    height: 0 !important;
  }
</style>

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
        setAppSetting("colimaui_tour_complete", "false");
        onStartTour();
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

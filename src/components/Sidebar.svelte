<script lang="ts">
  import { onMount } from "svelte";
  import * as Icons from "./Icons.svelte";
  import { uiState } from "../store.svelte";
  import { setAppSetting, getAppSetting } from "../lib/settingsStore.svelte";
  import { t, getLanguage } from "../lib/i18n.svelte";
  import { dashboardState } from "../store.svelte";
  import type { SystemInfo } from "../lib/api";
  import { Bell } from "./Icons.svelte";
  import {
    unreadCount,
    openNotificationPanel,
    closeNotificationPanel,
    notificationState,
  } from "../store/notifications.svelte";
  import { closeAiPanel } from "../store.svelte";

  let { systemInfo, onStartTour } = $props<{
    systemInfo: SystemInfo | null;
    onStartTour: () => void;
  }>();

  const instances = $derived(dashboardState.colimaInstances);
  const unread = $derived(unreadCount());

  /**
   * Only one side panel at a time.
   *
   * Both are fixed to the same edge, so opening the second draws it over the first
   * and leaves two Escape handlers listening. Deciding it here keeps the two panels
   * from having to know about each other.
   */
  function openNotifications() {
    if (notificationState.panelOpen) {
      closeNotificationPanel();
      return;
    }
    closeAiPanel();
    openNotificationPanel();
  }

  let currentTime = $state(new Date());
  // Hours and minutes only. The seconds were the sole reason this had to tick
  // once a second, and nobody reads a sidebar clock to the second.
  const formatTime = () =>
    currentTime.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

  onMount(() => {
    uiState.sidebarCollapsed = getAppSetting("colimaui_sidebar_collapsed") === "true";
    // Aligned to the top of the minute rather than ticking every 60s from
    // mount: a fixed interval started at :47 would flip the displayed minute
    // 13 seconds late, every minute.
    let clockInterval: ReturnType<typeof setInterval> | undefined;
    const align = setTimeout(() => {
      currentTime = new Date();
      clockInterval = setInterval(() => { currentTime = new Date(); }, 60000);
    }, 60000 - (Date.now() % 60000));
    return () => { clearTimeout(align); if (clockInterval) clearInterval(clockInterval); };
  });

  const navGroups = $derived([
    {
      label: t("sidebar.overview", { default: "Overview" }),
      items: [
        { id: "dashboard", label: t("sidebar.dashboard", { default: "Dashboard" }), icon: Icons.Dashboard },
        { id: "instances", label: t("sidebar.colima_instances", { default: "Instances" }), icon: Icons.Server },
      ],
    },
    {
      label: t("sidebar.docker", { default: "Docker" }),
      items: [
        { id: "containers", label: t("sidebar.containers", { default: "Containers" }), icon: Icons.Container },
        { id: "images", label: t("sidebar.images", { default: "Images" }), icon: Icons.Image },
        { id: "volumes", label: t("sidebar.volumes", { default: "Volumes" }), icon: Icons.Volume },
        { id: "networks", label: t("sidebar.networks", { default: "Networks" }), icon: Icons.Network },
        { id: "compose", label: t("sidebar.compose", { default: "Compose" }), icon: Icons.Compose },
        { id: "topology", label: t("sidebar.topology", { default: "Topology" }), icon: Icons.Topology },
        { id: "activity", label: t("sidebar.activity", { default: "Activity" }), icon: Icons.Activity },
        { id: "security", label: t("sidebar.security", { default: "Security" }), icon: Icons.Shield },
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
        // Help deliberately lives in the footer, not here: it is a reference
        // you consult, not a place you work, and a 15th nav item pushed the
        // list past the viewport so Dashboard scrolled under the header.
      ],
    },
  ]);

  /**
   * Colima status for the footer.
   *
   * The old footer printed "Colima not detected" as dead text with no way to
   * act on it. Version alone also cannot distinguish "installed but stopped"
   * from "running", which is the difference the user actually cares about —
   * so this reads the polled instance list too.
   */
  const colimaStatus = $derived.by(() => {
    const version = systemInfo?.colima_version
      ? `v${systemInfo.colima_version.split("\n")[0].replace(/.*version\s*/i, "")}`
      : null;

    if (!version) {
      return {
        tone: "off" as const,
        label: t("sidebar.colima_missing", { default: "Colima not detected" }),
        detail: t("sidebar.colima_missing_hint", { default: "Open Help to install it" }),
        target: "help",
      };
    }

    const running = instances.filter((i) => i.status?.toLowerCase() === "running").length;
    if (running > 0) {
      return {
        tone: "on" as const,
        label: `Colima ${version}`,
        detail: t("sidebar.colima_running", { default: "{count} running", count: running }),
        target: "instances",
      };
    }
    return {
      tone: "idle" as const,
      label: `Colima ${version}`,
      detail: t("sidebar.colima_stopped", { default: "Stopped" }),
      target: "instances",
    };
  });

  /**
   * Date first, then time. The clock previously showed time only, which is
   * ambiguous on a machine left running overnight — and the seconds are what
   * make a stale window obvious at a glance, so they stay.
   */
  const formatDate = () =>
    currentTime.toLocaleDateString(getLanguage(), { day: "numeric", month: "short" });
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

  /* Sits in the header rather than the nav list: it opens a panel over the current
     page instead of going anywhere, and the nav list has no room left. */
  .notif-button {
    position: relative;
    margin-left: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .notif-button:hover {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }
  :global(.sidebar.collapsed) .notif-button {
    margin-left: 0;
  }
  .notif-badge {
    position: absolute;
    top: -2px;
    right: -2px;
    min-width: 15px;
    padding: 0 3px;
    border-radius: 8px;
    background: var(--accent-red, #f85149);
    color: #fff;
    font-size: 10px;
    line-height: 15px;
    text-align: center;
    /* The button's aria-label carries the count too: a badge is a visual cue and
       a screen reader would otherwise read a bare number with no subject. */
  }

  .nav-panel-hint {
    margin-left: auto;
    font-size: 11px;
    line-height: 1;
    color: var(--text-muted);
  }
  :global(.sidebar.collapsed) .nav-panel-hint {
    display: none;
  }

  /* ===== Footer =====
     Two rows instead of four. The old layout gave a full-width nav-item row
     each to Tour, the clock, the version string and Collapse — about 140px for
     content nobody clicks, which is what pushed the nav list past the viewport
     and left Dashboard scrolled under the header. */

  .footer-status {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
    color: var(--text-secondary);
    cursor: pointer;
    font: inherit;
    text-align: left;
  }
  .footer-status:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-subtle);
  }
  .footer-status:hover .footer-status-chevron {
    opacity: 1;
  }

  .footer-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  /* Colour carries the state, but the label always spells it out too — colour
     alone would be invisible to a colour-blind user. */
  .footer-status-on .footer-dot {
    background: var(--accent-green, #3fb950);
    box-shadow: 0 0 0 3px rgba(63, 185, 80, 0.15);
  }
  .footer-status-idle .footer-dot {
    background: var(--accent-yellow, #d29922);
  }
  .footer-status-off .footer-dot {
    background: var(--accent-red, #f85149);
  }

  .footer-status-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }
  .footer-status-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .footer-status-detail {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .footer-status-chevron {
    margin-left: auto;
    flex-shrink: 0;
    opacity: 0;
    color: var(--text-muted);
    transition: opacity var(--transition-fast, 0.15s);
  }

  .footer-controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 4px;
    padding: 0 8px;
  }

  .footer-clock {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
    font-size: 10px;
    color: var(--text-muted);
  }
  .footer-clock-date {
    white-space: nowrap;
  }
  .footer-clock-time {
    font-family: var(--font-mono);
    /* Tabular figures stop the row twitching every second as digit widths
       change, which is distracting in the corner of the eye. */
    font-variant-numeric: tabular-nums;
  }

  .footer-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .footer-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .footer-icon-btn:hover {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }
  .footer-icon-btn.active {
    color: var(--accent-blue);
  }

  /* ===== Collapsed (64px) =====
     Everything stacks and loses its text; the status dot alone still answers
     "is Colima up?", which is the one thing worth keeping at this width. */
  :global(.sidebar.collapsed) .footer-status {
    justify-content: center;
    padding: 7px 0;
  }
  :global(.sidebar.collapsed) .footer-status-text,
  :global(.sidebar.collapsed) .footer-status-chevron,
  :global(.sidebar.collapsed) .footer-clock {
    display: none;
  }
  :global(.sidebar.collapsed) .footer-controls {
    justify-content: center;
    padding: 0;
  }
  :global(.sidebar.collapsed) .footer-actions {
    flex-direction: column;
  }
</style>

<aside class="sidebar {uiState.sidebarCollapsed ? 'collapsed' : ''}">
  <div class="sidebar-header" data-tauri-drag-region>
    <img src="/colima_icon.png" alt="ColimaUI" class="sidebar-logo" data-tauri-drag-region />
    <h1 class="sidebar-title" data-tauri-drag-region>ColimaUI</h1>
    <!-- Here rather than in `navGroups`: this opens a panel over the current page
         instead of navigating anywhere, and the nav list is already at the height
         the viewport allows. -->
    <button
      class="notif-button"
      onclick={openNotifications}
      aria-label={unread > 0
        ? t('notifications.unread_label', { default: '{count} unread notifications', count: unread })
        : t('notifications.title', { default: 'Notifications' })}
    >
      {@html Bell}
      {#if unread > 0}
        <span class="notif-badge" aria-hidden="true">{unread > 99 ? '99+' : unread}</span>
      {/if}
    </button>
  </div>

  <nav class="sidebar-nav" data-tour-id="sidebar-nav">
    {#each navGroups as group (group.label)}
      <div class="nav-section">
        <div class="nav-section-label">{group.label}</div>
        {#each group.items as item (item.id)}
          {@const isPanelToggle = item.id === "ai-chat"}
          <!-- The panel toggle carries two cues: the ◧/◫ glyph below is the
               visual one, its `title` the text one. The glyph alone is too
               small to explain why this item behaves unlike every other. -->
          <button
            class="nav-item {isPanelToggle
              ? uiState.aiPanelOpen
                ? 'active'
                : ''
              : uiState.currentPage === item.id
                ? 'active'
                : ''}"
            onclick={() => {
              if (isPanelToggle) {
                // Same rule as the bell: one side panel at a time. The AI panel
                // would otherwise open underneath the notification centre's
                // backdrop, visible but unclickable.
                if (!uiState.aiPanelOpen) closeNotificationPanel();
                uiState.aiPanelOpen = !uiState.aiPanelOpen;
              } else {
                uiState.currentPage = item.id;
              }
            }}
            data-tour-id={`nav-${item.id}`}
            title={isPanelToggle
              ? (uiState.aiPanelOpen
                  ? t("sidebar.ai_panel_close", { default: "Close the AI panel (⌘K)" })
                  : t("sidebar.ai_panel_open", { default: "Open the AI panel beside the page (⌘K)" }))
              : (uiState.sidebarCollapsed ? item.label : undefined)}
            aria-pressed={isPanelToggle ? uiState.aiPanelOpen : undefined}
          >
            {@html item.icon}
            <span class="nav-item-text">{item.label}</span>
            <!-- The one item that opens a panel beside the page instead of
                 replacing it. Without a marker its "active" state reads as
                 "you are on the AI page", which is not where you are. -->
            {#if isPanelToggle}
              <span class="nav-panel-hint" aria-hidden="true">
                {uiState.aiPanelOpen ? "◧" : "◫"}
              </span>
            {/if}
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <div class="sidebar-footer">
    <!-- Status row. Actionable: a dead "Colima not detected" label told the
         user something was wrong and gave them nowhere to go. -->
    <button
      class="footer-status footer-status-{colimaStatus.tone}"
      onclick={() => (uiState.currentPage = colimaStatus.target)}
      title="{colimaStatus.label} — {colimaStatus.detail}"
    >
      <span class="footer-dot" aria-hidden="true"></span>
      <span class="footer-status-text">
        <span class="footer-status-label">{colimaStatus.label}</span>
        <span class="footer-status-detail">{colimaStatus.detail}</span>
      </span>
      <svg class="footer-status-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <polyline points="9 18 15 12 9 6" />
      </svg>
    </button>

    <!-- Control row: clock plus the three actions that used to each own a
         full-width row of their own. -->
    <div class="footer-controls">
      <div class="footer-clock" title={`${formatDate()} ${formatTime()}`}>
        <span class="footer-clock-date">{formatDate()}</span>
        <span class="footer-clock-time">{formatTime()}</span>
      </div>

      <div class="footer-actions">
        <button
          class="footer-icon-btn"
          onclick={() => {
            setAppSetting("colimaui_tour_complete", "false");
            onStartTour();
          }}
          title={t("sidebar.tour_guide", { default: "Tour Guide" })}
          aria-label={t("sidebar.tour_guide", { default: "Tour Guide" })}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 22V4a1 1 0 0 1 1-1h13l-3 5 3 5H5" />
          </svg>
        </button>

        <button
          class="footer-icon-btn {uiState.currentPage === 'help' ? 'active' : ''}"
          onclick={() => (uiState.currentPage = "help")}
          title={t("sidebar.help", { default: "Help" })}
          aria-label={t("sidebar.help", { default: "Help" })}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" /><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" /><line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        </button>

        <button
          class="footer-icon-btn"
          onclick={() => {
            uiState.sidebarCollapsed = !uiState.sidebarCollapsed;
            setAppSetting("colimaui_sidebar_collapsed", uiState.sidebarCollapsed ? "true" : "false");
          }}
          title={uiState.sidebarCollapsed
            ? t("sidebar.expand", { default: "Expand sidebar" })
            : t("sidebar.collapse", { default: "Collapse sidebar" })}
          aria-label={uiState.sidebarCollapsed
            ? t("sidebar.expand", { default: "Expand sidebar" })
            : t("sidebar.collapse", { default: "Collapse sidebar" })}
        >
          {#if uiState.sidebarCollapsed}
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="13 17 18 12 13 7"/><polyline points="6 17 11 12 6 7"/></svg>
          {:else}
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="11 17 6 12 11 7"/><polyline points="18 17 13 12 18 7"/></svg>
          {/if}
        </button>
      </div>
    </div>
  </div>
</aside>

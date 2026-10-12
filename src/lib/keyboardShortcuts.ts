import { uiState } from "../store.svelte";
import { notificationState, closeNotificationPanel } from "../store/notifications.svelte";
import { refreshManual, refetchAllResources } from "./dataPoller";

/**
 * ⌘1-9 in sidebar order. `ai-chat` is deliberately absent: it opens a panel
 * beside the page rather than changing page, and has ⌘K of its own.
 */
export const QUICK_PAGES = [
  "dashboard", "instances", "containers", "images", "volumes",
  "networks", "compose", "topology", "activity",
];

/** True when the key belongs to an input, not to the application. */
function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return true;
  // xterm builds a hidden textarea of its own; every key inside the Terminal
  // page belongs to the shell running there.
  return target.closest(".xterm") !== null;
}

export function handleGlobalKeydown(e: KeyboardEvent): void {
  const meta = e.metaKey || e.ctrlKey;

  // Escape leaves the input first, and only closes a panel on the next press.
  if (e.key === "Escape") {
    if (isTypingTarget(e.target)) {
      (e.target as HTMLElement).blur();
      return;
    }
    // ErrorDetailPanel handles its own Escape. Only the two panels below are
    // this handler's business, and each press closes exactly one — outermost
    // first — rather than dismissing everything at once.
    if (uiState.aiPanelOpen) { uiState.aiPanelOpen = false; return; }
    if (notificationState.panelOpen) { closeNotificationPanel(); return; }
    return;
  }

  if (!meta) return;
  // The user is typing: ⌘C/⌘V/⌘A belong to the input, not to the app.
  if (isTypingTarget(e.target)) return;

  // ⌘K — toggle the AI panel. Same one-panel-at-a-time rule as the sidebar item.
  if (e.key.toLowerCase() === "k") {
    e.preventDefault();
    if (!uiState.aiPanelOpen) closeNotificationPanel();
    uiState.aiPanelOpen = !uiState.aiPanelOpen;
    return;
  }

  // ⌘⇧R — refresh data. NOT ⌘R: that reloads the window in Tauri and the page
  // in browser mode, and claiming it takes away a key the user needs.
  if (e.shiftKey && e.key.toLowerCase() === "r") {
    e.preventDefault();
    refreshManual();
    refetchAllResources();
    return;
  }

  // ⌘1-9 — jump to a page.
  if (e.key >= "1" && e.key <= "9") {
    const page = QUICK_PAGES[Number(e.key) - 1];
    if (!page) return;
    e.preventDefault();
    uiState.currentPage = page;
  }
}

import type { ColimaInstance, DockerContainer, DockerImage, DockerVolume, DockerNetwork, EngineResources, LimaInstance, SystemInfo } from "./lib/api";

export const dockerState = $state({
  containers: [] as DockerContainer[],
  images: [] as DockerImage[],
  loading: true,
  eventCooldownUntil: 0,
});

export const resourceState = $state({
  volumes: [] as DockerVolume[],
  volumesLoading: true,
  networks: [] as DockerNetwork[],
  networksLoading: true,
});

export const dashboardState = $state({
  colimaInstances: [] as ColimaInstance[],
  /**
   * Whether `colimaInstances` has been filled at least once.
   *
   * An empty array is ambiguous — "this machine has no instances" and "the
   * poller has not answered yet" look identical, and a UI that treats the
   * second as the first flashes an empty state on every cold start.
   */
  instancesLoaded: false,
  composeProjectsCount: 0,
  k8sStatus: { connected: false, pods: 0, namespaces: 0, kindClusters: 0 },
  linuxVMs: [] as LimaInstance[],
  lastFetch: 0,
  systemInfo: null as SystemInfo | null,
  /**
   * CPU / RAM / disk reported by the container engine itself. Null until the
   * first poll answers; `available: false` when no engine is reachable.
   */
  engineResources: null as EngineResources | null,
});

export const uiState = $state({
  aiPanelOpen: false,
  currentPage: "dashboard",
  globalError: null as string | null,
  sidebarCollapsed: false,
  /**
   * Slug of the Help article to open on the next visit to the Help page.
   *
   * Set by anything that carries a `doc_id` — an error entry, a missing
   * capability — immediately before navigating. The Help page consumes it and
   * clears it, so returning to Help later lands on the index rather than
   * re-opening a stale article.
   */
  helpArticle: null as string | null,
  /**
   * Section of the Settings page to reveal on the next visit.
   *
   * Settings is one long scrolling page, so navigating to it alone drops the
   * user at the top with no hint that the thing they clicked through for is
   * several cards further down. Consumed and cleared by the owning section,
   * same one-shot contract as `helpArticle`.
   */
  settingsSection: null as string | null,
  /**
   * Terminal session to open on the next visit to the Terminal page.
   *
   * Set by anything that wants a shell — today the Kubernetes pod detail
   * drawer. Consumed and cleared by the Terminal page, same one-shot contract
   * as `helpArticle` and `settingsSection`.
   *
   * Typed as `unknown` to keep the store free of a transport import; the
   * Terminal page casts it back to `SessionKind`.
   */
  pendingTerminalSession: null as unknown,
});

/** Navigate to the Help page with a specific article open. */
export function openHelpArticle(slug: string) {
  uiState.helpArticle = slug;
  uiState.currentPage = "help";
}

/** Navigate to Settings, scrolled to a specific section. */
export function openSettingsSection(section: string) {
  uiState.settingsSection = section;
  uiState.currentPage = "settings";
}

/**
 * Open a shell in the app's own Terminal page.
 *
 * The alternative — and what the Kubernetes drawer used to do — is shelling out
 * to `osascript` to drive Terminal.app, which drops the user out of the app and
 * puts a hand-escaped command string on a shell command line.
 */
export function openTerminalSession(kind: unknown) {
  uiState.pendingTerminalSession = kind;
  uiState.currentPage = "terminal";
}

export function isEventCooldownActive() {
  return Date.now() < dockerState.eventCooldownUntil;
}

export function setEventCooldown(durationMs: number = 3000) {
  dockerState.eventCooldownUntil = Date.now() + durationMs;
}

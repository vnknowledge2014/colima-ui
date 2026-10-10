import { colimaApi, dockerApi, volumesApi, networksApi, sysMethods, getApiToken, type ColimaInstance } from "./api";
import { resolveApiBase } from "./api/client";
import { loadCapabilities } from "../store/capabilities.svelte";
import { normalizeContainer, normalizeImage } from "./normalizers";
import { dockerState, resourceState, dashboardState, isEventCooldownActive, uiState } from "../store.svelte";
import { getAppSetting } from "./settingsStore.svelte";
import { setVisibleInterval } from "./visibleInterval";

import { isRunningInTauri } from "./env";
const isTauri = isRunningInTauri();
let _refetchRetryCount = 0;
const MAX_REFETCH_RETRIES = 10;
let stopPolling: (() => void) | null = null;
let refetchRetryTimeout: ReturnType<typeof setTimeout> | null = null;
// Module-level, not inside the browser-mode branch: the teardown returned by
// startDataPoller() lives outside that branch and has to be able to close it.
let currentEventSource: EventSource | null = null;
// A refetch already in flight when teardown runs would otherwise schedule its
// retry after the timeout was cleared, and keep polling a dead context.
let disposed = false;
let stopSysPoll: (() => void) | null = null;
let sseRetryTimeout: ReturnType<typeof setTimeout> | null = null;

export async function refreshManual() {
  try {
    const [instanceList, sysInfo] = await Promise.all([
      colimaApi.listInstances().catch(() => [] as ColimaInstance[]),
      sysMethods.checkSystem().catch(() => null),
    ]);
    dashboardState.colimaInstances = instanceList;
    dashboardState.instancesLoaded = true;
    if (sysInfo) dashboardState.systemInfo = sysInfo;
  } catch (e) {
    uiState.globalError = String(e);
  } finally {
    // loading handled by consumer or not needed globally anymore
  }
}

export async function refetchAllResources() {
  if (disposed) return;
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
    // Checked before the give-up branch below: a teardown mid-flight must not
    // log "gave up after 10 attempts" when it never got to retry at all.
    if (disposed) return;
    _refetchRetryCount++;
    if (_refetchRetryCount < MAX_REFETCH_RETRIES) {
      refetchRetryTimeout = setTimeout(refetchAllResources, 5000);
    } else {
      console.warn(`[ColimaUI] refetchAllResources gave up after ${MAX_REFETCH_RETRIES} attempts`);
    }
  } else {
    _refetchRetryCount = 0;
  }
}

function handleDockerStateUpdated(data: { containers?: unknown[]; images?: unknown[] }) {
  if (isEventCooldownActive()) return;

  dockerState.containers = (data.containers || []).map((c) => normalizeContainer(c as Record<string, unknown>));
  dockerState.images = (data.images || []).map((i) => normalizeImage(i as Record<string, unknown>));
  dockerState.loading = false;
  
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

export function startDataPoller() {
  // A second start after a teardown (HMR, or a re-mounted root) must not
  // inherit the previous run's disposed flag or its retry count.
  disposed = false;
  _refetchRetryCount = 0;
  refreshManual();
  refetchAllResources();
  // Load once at startup so pages can explain an empty list ("Colima isn't
  // installed") instead of just showing nothing. The backend caches this and
  // drops the cache when an instance changes state, so refreshing is cheap.
  loadCapabilities();

  const rsEnabled = getAppSetting("colimaui_auto_pause") === "true";
  const rsMins = parseInt(getAppSetting("colimaui_auto_pause_mins") || "15", 10);
  sysMethods.setResourceSaver(rsEnabled, rsMins).catch(() => {});

  stopSysPoll = setVisibleInterval(() => { sysMethods.checkSystem().then(s => dashboardState.systemInfo = s).catch(()=>{}); }, 30000);

  if (isTauri) {
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen("instances-update", (event: { payload: { instances?: ColimaInstance[] } }) => {
        if (event.payload.instances) dashboardState.colimaInstances = event.payload.instances;
        dashboardState.instancesLoaded = true;
        // Starting or stopping an instance flips Colima and Docker between
        // "installed but not running" and "usable".
        loadCapabilities();
      });
      mod.listen("docker-state-updated", (event: { payload: { containers?: unknown[]; images?: unknown[] } }) =>
        handleDockerStateUpdated(event.payload)
      );
      mod.listen("docker-connection-lost", handleConnectionLost);
      mod.listen("docker-reconnected", refetchAllResources);
      // Tray menu navigation (container click / Help) drives the active page.
      mod.listen("navigate", (event: { payload?: { page?: unknown } }) => {
        const page = event.payload?.page;
        if (typeof page === "string") uiState.currentPage = page;
      });
    });
  } else {
    let sseRetryDelay = 2000;

    async function connectSSE() {
      // A reconnect must not leave the previous socket open: Chrome caps a
      // single origin at 6 concurrent connections, after which every request
      // to 127.0.0.1:11420 blocks — including the plain HTTP polling fallback.
      if (currentEventSource) {
        currentEventSource.close();
        currentEventSource = null;
      }
      if (disposed) return;

      // Re-read the token on every attempt rather than capturing it once.
      // Browser mode receives it from a URL fragment, so the very first attempt
      // can legitimately run before one is available; a captured empty string
      // would then be retried forever against an endpoint that always 401s.
      const token = await getApiToken();

      // Must use the resolved base: the server falls back to 11421-11429 when
      // the default port is taken, and an EventSource pointed at the wrong port
      // fails silently — the UI would just stop updating.
      const base = await resolveApiBase();
      const sseUrl = token ? `${base}/api/events?token=${token}` : `${base}/api/events`;
      // Re-checked after the awaits above: a teardown during token/base
      // resolution would otherwise open a socket nobody is left to close.
      if (disposed) return;
      const es = new EventSource(sseUrl);
      currentEventSource = es;
      es.addEventListener("instances-update", (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data) as { instances?: ColimaInstance[] };
          if (data.instances) dashboardState.colimaInstances = data.instances;
          dashboardState.instancesLoaded = true;
          loadCapabilities();
        } catch {
          // A malformed SSE frame is ignored; the next update will retry.
        }
      });
      es.addEventListener("docker-state-updated", (e: MessageEvent) => {
        try {
          handleDockerStateUpdated(JSON.parse(e.data) as { containers?: unknown[]; images?: unknown[] });
        } catch {
          // A malformed SSE frame is ignored; the next update will retry.
        }
      });
      es.addEventListener("docker-connection-lost", handleConnectionLost);
      es.addEventListener("docker-reconnected", refetchAllResources);
      es.onopen = () => {
        sseRetryDelay = 2000;
        if (stopPolling) {
          stopPolling();
          stopPolling = null;
        }
      };
      es.onerror = () => {
        es.close();
        if (!stopPolling) {
          stopPolling = setVisibleInterval(() => {
            refreshManual();
            refetchAllResources();
          }, 5000);
        }
        sseRetryTimeout = setTimeout(() => {
          connectSSE();
        }, sseRetryDelay);
        sseRetryDelay = Math.min(sseRetryDelay * 2, 30000);
      };
    }

    connectSSE();
  }

  return () => {
    disposed = true;
    if (stopSysPoll) { stopSysPoll(); stopSysPoll = null; }
    if (stopPolling) { stopPolling(); stopPolling = null; }
    if (refetchRetryTimeout) clearTimeout(refetchRetryTimeout);
    if (sseRetryTimeout) clearTimeout(sseRetryTimeout);
    // close() does not fire onerror, so no reconnect is woken by the teardown.
    if (currentEventSource) {
      currentEventSource.close();
      currentEventSource = null;
    }
  };
}

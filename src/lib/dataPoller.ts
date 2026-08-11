import { colimaApi, dockerApi, volumesApi, networksApi, sysMethods, getApiToken, type ColimaInstance } from "./api";
import { resolveApiBase } from "./api/client";
import { loadCapabilities } from "../store/capabilities.svelte";
import { normalizeContainer, normalizeImage } from "./normalizers";
import { dockerState, resourceState, dashboardState, isEventCooldownActive, uiState } from "../store.svelte";
import { getAppSetting } from "./settingsStore.svelte";

import { isRunningInTauri } from "./env";
const isTauri = isRunningInTauri();
let _refetchRetryCount = 0;
const MAX_REFETCH_RETRIES = 10;
let pollInterval: ReturnType<typeof setInterval> | null = null;
let sysInterval: ReturnType<typeof setInterval> | null = null;
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
    _refetchRetryCount = 0;
  }
}

function handleDockerStateUpdated(data: any) {
  if (isEventCooldownActive()) return;

  dockerState.containers = (data.containers || []).map(normalizeContainer);
  dockerState.images = (data.images || []).map(normalizeImage);
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
  refreshManual();
  refetchAllResources();
  // Load once at startup so pages can explain an empty list ("Colima isn't
  // installed") instead of just showing nothing. The backend caches this and
  // drops the cache when an instance changes state, so refreshing is cheap.
  loadCapabilities();

  const rsEnabled = getAppSetting("colimaui_auto_pause") === "true";
  const rsMins = parseInt(getAppSetting("colimaui_auto_pause_mins") || "15", 10);
  sysMethods.setResourceSaver(rsEnabled, rsMins).catch(() => {});

  sysInterval = setInterval(() => { sysMethods.checkSystem().then(s => dashboardState.systemInfo = s).catch(()=>{}); }, 30000);

  if (isTauri) {
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen("instances-update", (event: any) => {
        dashboardState.colimaInstances = event.payload.instances;
        dashboardState.instancesLoaded = true;
        // Starting or stopping an instance flips Colima and Docker between
        // "installed but not running" and "usable".
        loadCapabilities();
      });
      mod.listen("docker-state-updated", (event: any) => handleDockerStateUpdated(event.payload));
      mod.listen("docker-connection-lost", handleConnectionLost);
      mod.listen("docker-reconnected", refetchAllResources);
      // Tray menu navigation (container click / Help) drives the active page.
      mod.listen("navigate", (event: any) => {
        const page = event.payload?.page;
        if (typeof page === "string") uiState.currentPage = page;
      });
    });
  } else {
    let sseRetryDelay = 2000;

    async function connectSSE(token: string) {
      // Must use the resolved base: the server falls back to 11421-11429 when
      // the default port is taken, and an EventSource pointed at the wrong port
      // fails silently — the UI would just stop updating.
      const base = await resolveApiBase();
      const sseUrl = token ? `${base}/api/events?token=${token}` : `${base}/api/events`;
      const es = new EventSource(sseUrl);
      es.addEventListener("instances-update", (e: any) => {
        try {
          const data = JSON.parse(e.data);
          dashboardState.colimaInstances = data.instances;
          dashboardState.instancesLoaded = true;
          loadCapabilities();
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
        sseRetryDelay = 2000;
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
      };
      es.onerror = () => {
        es.close();
        if (!pollInterval) {
          pollInterval = setInterval(() => {
            refreshManual();
            refetchAllResources();
          }, 5000);
        }
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
    if (sysInterval) clearInterval(sysInterval);
    if (pollInterval) clearInterval(pollInterval);
    if (sseRetryTimeout) clearTimeout(sseRetryTimeout);
  };
}

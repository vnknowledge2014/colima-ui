import type { ColimaInstance, DockerContainer, DockerImage, DockerVolume, DockerNetwork, LimaInstance } from "./lib/api";

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
  composeProjectsCount: 0,
  k8sStatus: { connected: false, pods: 0, namespaces: 0, kindClusters: 0 },
  linuxVMs: [] as LimaInstance[],
  lastFetch: 0,
});

export const uiState = $state({
  aiPanelOpen: false,
  currentPage: "dashboard",
});

export function isEventCooldownActive() {
  return Date.now() < dockerState.eventCooldownUntil;
}

export function setEventCooldown(durationMs: number = 3000) {
  dockerState.eventCooldownUntil = Date.now() + durationMs;
}

import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Docker API =====

export const dockerApi = {
  listContainers: async (all = true): Promise<DockerContainer[]> => {
    const raw = await call<any>("list_containers", { all }, "GET", "/api/containers", { all: String(all) });
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    // Normalize field names for Tauri IPC compatibility
    return items.map((v: any) => ({
      Id: v.Id || v.id || v.ID || "",
      Names: v.Names || v.names || "",
      Image: v.Image || v.image || "",
      Status: v.Status || v.status || "",
      State: v.State || v.state || "",
      Ports: v.Ports || v.ports || "",
      CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
      Size: v.Size || v.size || "",
      Command: v.Command || v.command || "",
    }));
  },

  startContainer: (containerId: string) =>
    call<string>("start_container", { containerId }, "POST", "/api/containers/start", { containerId }),

  stopContainer: (containerId: string) =>
    call<string>("stop_container", { containerId }, "POST", "/api/containers/stop", { containerId }),

  restartContainer: (containerId: string) =>
    call<string>("restart_container", { containerId }, "POST", "/api/containers/restart", { containerId }),

  removeContainer: (containerId: string, force = false) =>
    call<string>("remove_container", { containerId, force }, "POST", "/api/containers/remove", { containerId, force: String(force) }),

  containerLogs: (containerId: string, lines = 200) =>
    call<string>("container_logs", { containerId, lines }, "GET", "/api/containers/logs", { containerId, lines: String(lines) }),

  listImages: async (): Promise<DockerImage[]> => {
    const raw = await call<any>("list_images", undefined, "GET", "/api/images");
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    return items.map((v: any) => ({
      Id: v.Id || v.id || v.ID || "",
      Repository: v.Repository || v.repository || "",
      Tag: v.Tag || v.tag || "",
      Size: v.Size || v.size || "",
      CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
    }));
  },

  inspectContainer: (containerId: string) =>
    call<string>("inspect_container", { containerId }, "GET", "/api/containers/inspect", { containerId }),

  // Image management
  removeImage: (imageId: string, force = false) =>
    call<string>("remove_image", { imageId, force }, "POST", "/api/images/remove", { imageId, force: String(force) }),

  pullImage: (imageName: string) =>
    call<string>("pull_image", { imageName }, "POST", "/api/images/pull", { imageName }),

  pruneImages: () =>
    call<string>("prune_images", undefined, "POST", "/api/images/prune"),

  inspectImage: (imageId: string) =>
    call<string>("inspect_image", { imageId }, "GET", "/api/images/inspect", { imageId }),

  tagImage: (source: string, target: string) =>
    call<string>("tag_image", { source, target }, "POST", "/api/images/tag", undefined, { source, target }),

  // Container enhancement
  containerStats: (containerId: string) =>
    call<string>("container_stats", { containerId }, "GET", "/api/containers/stats", { containerId }),

  allContainerStats: () =>
    call<string>("all_container_stats", undefined, "GET", "/api/containers/stats/all"),

  containerTop: (containerId: string) =>
    call<string>("container_top", { containerId }, "GET", "/api/containers/top", { containerId }),

  containerExec: (containerId: string, command: string) =>
    call<string>("container_exec", { containerId, command }, "POST", "/api/containers/exec", undefined, { containerId, command }),

  runContainer: (image: string, name = "", ports: string[] = [], envVars: string[] = [], volumes: string[] = [], detach = true, removeOnExit = false, extraArgs: string[] = []) =>
    call<string>("run_container", { image, name, ports, envVars, volumes, detach, removeOnExit, extraArgs }, "POST", "/api/containers/run", undefined, { image, name, ports, envVars, volumes, detach, removeOnExit, extraArgs }),

  renameContainer: (containerId: string, newName: string) =>
    call<string>("rename_container", { containerId, newName }, "POST", "/api/containers/rename", undefined, { containerId, newName }),

  pauseContainer: (containerId: string) =>
    call<string>("pause_container", { containerId }, "POST", "/api/containers/pause", { containerId }),

  unpauseContainer: (containerId: string) =>
    call<string>("unpause_container", { containerId }, "POST", "/api/containers/unpause", undefined, { id: containerId }),
  systemDf: () =>
    call<string>("system_df", undefined, "GET", "/api/docker/df"),
  systemPrune: (all = true) =>
    call<string>("system_prune", { all }, "POST", "/api/docker/prune"),
};

import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Volumes API =====

export const volumesApi = {
  listVolumes: async (): Promise<DockerVolume[]> => {
    const raw = await call<any>("list_volumes", undefined, "GET", "/api/volumes");
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    return items.map((v: any) => ({
      Name: v.Name || v.name || "",
      Driver: v.Driver || v.driver || "",
      Mountpoint: v.Mountpoint || v.mountpoint || v.mount_point || "",
      Scope: v.Scope || v.scope || "",
      Labels: v.Labels || v.labels || "",
    }));
  },

  createVolume: (name: string, driver = "local") =>
    call<string>("create_volume", { name, driver }, "POST", "/api/volumes/create", undefined, { name, driver }),

  removeVolume: (name: string, force = false) =>
    call<string>("remove_volume", { name, force }, "POST", "/api/volumes/remove", { name, force: String(force) }),

  pruneVolumes: () =>
    call<string>("prune_volumes", undefined, "POST", "/api/volumes/prune"),

  inspectVolume: (name: string) =>
    call<string>("inspect_volume", { name }, "GET", "/api/volumes/inspect", { name }),
};

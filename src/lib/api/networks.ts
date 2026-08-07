import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Networks API =====

export const networksApi = {
  listNetworks: async (): Promise<DockerNetwork[]> => {
    const raw = await call<any>("list_networks", undefined, "GET", "/api/networks");
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    // Normalize field names for Tauri IPC compatibility
    return items.map((v: any) => ({
      Id: v.Id || v.id || v.ID || "",
      Name: v.Name || v.name || "",
      Driver: v.Driver || v.driver || "",
      Scope: v.Scope || v.scope || "",
      Ipv6: v.Ipv6 || v.ipv6 || v.IPv6 || "",
      Internal: v.Internal || v.internal || "",
      Labels: v.Labels || v.labels || "",
    }));
  },

  createNetwork: (name: string, driver = "bridge", subnet = "") =>
    call<string>("create_network", { name, driver, subnet }, "POST", "/api/networks/create", undefined, { name, driver, subnet }),

  removeNetwork: (name: string) =>
    call<string>("remove_network", { name }, "POST", "/api/networks/remove", { name }),

  inspectNetwork: (name: string) =>
    call<string>("inspect_network", { name }, "GET", "/api/networks/inspect", { name }),

  pruneNetworks: () =>
    call<string>("prune_networks", undefined, "POST", "/api/networks/prune"),
};

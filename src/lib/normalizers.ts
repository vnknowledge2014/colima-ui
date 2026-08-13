/**
 * Shared Docker data normalizers — single source of truth.
 *
 * Tauri IPC and the HTTP API can return different field casings
 * (PascalCase vs snake_case vs camelCase). These normalizers
 * produce a consistent shape matching the TypeScript interfaces.
 */

import type { DockerContainer, DockerImage, DockerVolume, DockerNetwork } from "./api";

/** Normalize a raw container object into a consistent DockerContainer shape. */
export function normalizeContainer(v: Record<string, unknown>): DockerContainer {
  return {
    Id: String(v.Id || v.id || v.ID || ""),
    Names: String(v.Names || v.names || ""),
    Image: String(v.Image || v.image || ""),
    Status: String(v.Status || v.status || ""),
    State: String(v.State || v.state || ""),
    Ports: String(v.Ports || v.ports || ""),
    CreatedAt: String(v.CreatedAt || v.created_at || v.createdAt || ""),
    Size: String(v.Size || v.size || ""),
    Command: String(v.Command || v.command || ""),
    // The backend always sends an object; guard anyway so a container from an
    // older payload cannot break grouping.
    Labels:
      v.Labels && typeof v.Labels === "object" && !Array.isArray(v.Labels)
        ? (v.Labels as Record<string, string>)
        : {},
  };
}

/** Normalize a raw image object into a consistent DockerImage shape. */
export function normalizeImage(v: Record<string, unknown>): DockerImage {
  return {
    Id: String(v.Id || v.id || v.ID || ""),
    Repository: String(v.Repository || v.repository || ""),
    Tag: String(v.Tag || v.tag || ""),
    Size: String(v.Size || v.size || ""),
    CreatedAt: String(v.CreatedAt || v.created_at || v.createdAt || ""),
  };
}

/** Normalize a raw volume object into a consistent DockerVolume shape. */
export function normalizeVolume(v: Record<string, unknown>): DockerVolume {
  return {
    Name: String(v.Name || v.name || ""),
    Driver: String(v.Driver || v.driver || ""),
    Mountpoint: String(v.Mountpoint || v.mountpoint || v.mount_point || ""),
    Scope: String(v.Scope || v.scope || ""),
    Labels: String(v.Labels || v.labels || ""),
  };
}

/** Normalize a raw network object into a consistent DockerNetwork shape. */
export function normalizeNetwork(v: Record<string, unknown>): DockerNetwork {
  return {
    Id: String(v.Id || v.id || v.ID || ""),
    Name: String(v.Name || v.name || ""),
    Driver: String(v.Driver || v.driver || ""),
    Scope: String(v.Scope || v.scope || ""),
    Ipv6: String(v.Ipv6 || v.ipv6 || v.IPv6 || ""),
    Internal: String(v.Internal || v.internal || ""),
    Labels: String(v.Labels || v.labels || ""),
  };
}

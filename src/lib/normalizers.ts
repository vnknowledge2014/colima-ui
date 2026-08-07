/**
 * Shared Docker data normalizers — single source of truth.
 *
 * Tauri IPC and the HTTP API can return different field casings
 * (PascalCase vs snake_case vs camelCase). These normalizers
 * produce a consistent shape matching the TypeScript interfaces.
 */

import type { DockerContainer, DockerImage, DockerVolume, DockerNetwork } from "./api";

/** Normalize a raw container object into a consistent DockerContainer shape. */
export function normalizeContainer(v: Record<string, any>): DockerContainer {
  return {
    Id: v.Id || v.id || v.ID || "",
    Names: v.Names || v.names || "",
    Image: v.Image || v.image || "",
    Status: v.Status || v.status || "",
    State: v.State || v.state || "",
    Ports: v.Ports || v.ports || "",
    CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
    Size: v.Size || v.size || "",
    Command: v.Command || v.command || "",
  };
}

/** Normalize a raw image object into a consistent DockerImage shape. */
export function normalizeImage(v: Record<string, any>): DockerImage {
  return {
    Id: v.Id || v.id || v.ID || "",
    Repository: v.Repository || v.repository || "",
    Tag: v.Tag || v.tag || "",
    Size: v.Size || v.size || "",
    CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
  };
}

/** Normalize a raw volume object into a consistent DockerVolume shape. */
export function normalizeVolume(v: Record<string, any>): DockerVolume {
  return {
    Name: v.Name || v.name || "",
    Driver: v.Driver || v.driver || "",
    Mountpoint: v.Mountpoint || v.mountpoint || v.mount_point || "",
    Scope: v.Scope || v.scope || "",
    Labels: v.Labels || v.labels || "",
  };
}

/** Normalize a raw network object into a consistent DockerNetwork shape. */
export function normalizeNetwork(v: Record<string, any>): DockerNetwork {
  return {
    Id: v.Id || v.id || v.ID || "",
    Name: v.Name || v.name || "",
    Driver: v.Driver || v.driver || "",
    Scope: v.Scope || v.scope || "",
    Ipv6: v.Ipv6 || v.ipv6 || v.IPv6 || "",
    Internal: v.Internal || v.internal || "",
    Labels: v.Labels || v.labels || "",
  };
}

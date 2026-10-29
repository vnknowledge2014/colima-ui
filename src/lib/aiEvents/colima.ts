import { EventHandler } from "./types";
import type { StartConfig } from "../api";
import { colimaApi } from "../api";

function buildStartConfig(p: Record<string, unknown> = {}, fallbackProfile = "default"): StartConfig {
  const profile = String(p.profile || fallbackProfile);
  const memoryNum = Number(p.memory);
  const diskNum = Number(p.disk);
  const memory = memoryNum >= 1_073_741_824 ? Math.max(1, Math.round(memoryNum / 1_073_741_824)) : (memoryNum || 2);
  const disk = diskNum >= 1_073_741_824 ? Math.max(1, Math.round(diskNum / 1_073_741_824)) : (diskNum || 60);

  return {
    profile,
    runtime: String(p.runtime || "docker"),
    cpus: Number(p.cpus) || 2,
    memory,
    disk,
    vm_type: String(p.vm_type || "vz"),
    kubernetes: Boolean(p.kubernetes || false),
    kubernetes_version: String(p.kubernetes_version || ""),
    arch: String(p.arch || ""),
    mount_type: String(p.mount_type || ""),
    mounts: Array.isArray(p.mounts) ? (p.mounts as string[]) : [],
    dns: Array.isArray(p.dns) ? (p.dns as string[]) : [],
    network_address: Boolean(p.network_address || false),
  };
}

export const colimaRegistry: Record<string, EventHandler> = {
  "list-instances": {
    category: "SAFE",
    description: "List all Colima instances",
    handler: async () => JSON.stringify(await colimaApi.listInstances(), null, 2)
  },
  "create-instance": {
    category: "NORMAL",
    description: "Create and start a new Colima instance",
    handler: async (p) => {
      const config = buildStartConfig(p);
      await colimaApi.startInstance(config);
      return `Instance '${config.profile}' starting...`;
    }
  },
  "start-instance": {
    category: "NORMAL",
    description: "Start an existing Colima instance",
    handler: async (p) => {
      const profile = String(p?.profile || "default");
      try {
        const config = buildStartConfig(p, profile);
        await colimaApi.startInstance(config);
        return `Instance '${profile}' starting...`;
      } catch (err) {
        throw new Error(`Failed to start instance '${profile}': ${err}`, { cause: err });
      }
    }
  },
  "stop-instance": {
    category: "NORMAL",
    description: "Stop a Colima instance",
    handler: async (p) => {
      await colimaApi.stopInstance(p.profile);
      return `Instance '${p.profile}' stopped.`;
    }
  },
  "colima-restart": {
    category: "NORMAL",
    description: "Restart a colima instance",
    handler: async (p) => {
      const profile = String(p?.profile || "default");
      try {
        await colimaApi.stopInstance(profile).catch(() => {});
        const config = buildStartConfig(p, profile);
        await colimaApi.startInstance(config);
        return `Instance '${profile}' restarted.`;
      } catch (err) {
        throw new Error(`Failed to restart instance '${profile}': ${err}`, { cause: err });
      }
    }
  },
  "delete-instance": {
    category: "DANGEROUS",
    description: "Delete a Colima instance permanently",
    handler: async (p) => {
      await colimaApi.deleteInstance(p.profile);
      return `Instance '${p.profile}' deleted.`;
    }
  },
  "colima-status": {
    category: "SAFE",
    description: "Get detailed status of an instance",
    handler: async (p) => JSON.stringify(await colimaApi.instanceStatus(p.profile), null, 2)
  }
};

import { EventHandler } from "./types";
import type { StartConfig } from "../api";
import { colimaApi } from "../api";

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
      const config: StartConfig = {
        profile: p.profile || "default",
        runtime: p.runtime || "docker",
        cpus: p.cpus || 2,
        memory: p.memory || 2,
        disk: p.disk || 60,
        vm_type: p.vm_type || "vz",
        kubernetes: p.kubernetes || false,
        kubernetes_version: p.kubernetes_version || "",
        arch: p.arch || "",
        mount_type: p.mount_type || "",
        mounts: p.mounts || [],
        dns: p.dns || [],
        network_address: p.network_address || false,
      };
      await colimaApi.startInstance(config);
      return `Instance '${config.profile}' starting...`;
    }
  },
  "start-instance": {
    category: "NORMAL",
    description: "Start an existing Colima instance",
    handler: async (p) => {
      await colimaApi.startInstance({ profile: p.profile } as StartConfig);
      return `Instance '${p.profile}' starting...`;
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
      await colimaApi.stopInstance(p.profile);
      await colimaApi.startInstance({ profile: p.profile } as StartConfig);
      return `Instance '${p.profile}' restarted.`;
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

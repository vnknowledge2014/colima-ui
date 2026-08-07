// @ts-nocheck
import { EventHandler } from "./types";
import { 
  colimaApi, dockerApi, volumesApi, networksApi, sysMethods, 
  composeApi, modelsApi, k8sApi, kindApi, limaApi
} from "../api";

export const volumesRegistry: Record<string, EventHandler> = {
  "volume-list": {
    category: "SAFE",
    description: "List all volumes",
    handler: async () => JSON.stringify(await volumesApi.listVolumes(), null, 2)
  },
  "volume-inspect": {
    category: "SAFE",
    description: "Inspect a volume",
    handler: async (p) => JSON.stringify(await volumesApi.inspectVolume(p.name), null, 2)
  },
  "volume-create": {
    category: "NORMAL",
    description: "Create a docker volume",
    handler: async (p) => {
      await volumesApi.createVolume(p.name);
      return `Volume '${p.name}' created.`;
    }
  },
  "volume-remove": {
    category: "DANGEROUS",
    description: "Remove a docker volume",
    handler: async (p) => {
      await volumesApi.removeVolume(p.name);
      return `Volume '${p.name}' removed.`;
    }
  },
  "volume-prune": {
    category: "DANGEROUS",
    description: "Remove all unused volumes",
    handler: async () => {
      await volumesApi.pruneVolumes();
      return `Unused volumes pruned.`;
    }
  },
  "network-list": {
    category: "SAFE",
    description: "List all networks",
    handler: async () => JSON.stringify(await networksApi.listNetworks(), null, 2)
  },
  "network-inspect": {
    category: "SAFE",
    description: "Inspect a network",
    handler: async (p) => JSON.stringify(await networksApi.inspectNetwork(p.name), null, 2)
  },
  "network-create": {
    category: "NORMAL",
    description: "Create a docker network",
    handler: async (p) => {
      await networksApi.createNetwork(p.name);
      return `Network '${p.name}' created.`;
    }
  },
  "network-remove": {
    category: "DANGEROUS",
    description: "Remove a docker network",
    handler: async (p) => {
      await networksApi.removeNetwork(p.name);
      return `Network '${p.name}' removed.`;
    }
  },
  "network-prune": {
    category: "DANGEROUS",
    description: "Remove all unused networks",
    handler: async () => {
      await networksApi.pruneNetworks();
      return `Unused networks pruned.`;
    }
  }
};

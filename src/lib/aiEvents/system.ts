// @ts-nocheck
import { EventHandler } from "./types";
import { 
  colimaApi, dockerApi, volumesApi, networksApi, sysMethods, 
  composeApi, modelsApi, k8sApi, kindApi, limaApi
} from "../api";

export const systemRegistry: Record<string, EventHandler> = {
  "system-df": {
    category: "SAFE",
    description: "Get docker disk usage",
    handler: async () => JSON.stringify(await sysMethods.systemDf(), null, 2)
  },
  "system-check": {
    category: "SAFE",
    description: "Check system prerequisites",
    handler: async () => JSON.stringify(await sysMethods.checkSystem(), null, 2)
  },
  "system-host-specs": {
    category: "SAFE",
    description: "Get host specifications",
    handler: async () => JSON.stringify(await sysMethods.hostSpecs(), null, 2)
  },
  "system-check-tool": {
    category: "SAFE",
    description: "Check if a specific tool is installed (e.g., helm, kubectl)",
    handler: async (p) => JSON.stringify(await sysMethods.checkTool(p.name), null, 2)
  },
  "system-prune": {
    category: "DANGEROUS",
    description: "Prune entire system (containers, networks, images, volumes)",
    handler: async () => {
      await sysMethods.systemPrune();
      return `System pruned successfully.`;
    }
  }
};

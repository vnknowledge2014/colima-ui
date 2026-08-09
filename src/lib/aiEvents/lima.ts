import { isRunningInTauri } from "../env";
// @ts-nocheck
import { EventHandler } from "./types";
import { 
  colimaApi, dockerApi, volumesApi, networksApi, sysMethods, 
  composeApi, modelsApi, k8sApi, kindApi, limaApi
} from "../api";

export const limaRegistry: Record<string, EventHandler> = {
  "lima-list": {
    category: "SAFE",
    description: "List lima vms",
    handler: async () => JSON.stringify(await limaApi.list(), null, 2)
  },
  "lima-start": {
    category: "NORMAL",
    description: "Start a lima vm",
    handler: async (p) => {
      await limaApi.start(p.name);
      return `Lima VM '${p.name}' started.`;
    }
  },
  "lima-stop": {
    category: "NORMAL",
    description: "Stop a lima vm",
    handler: async (p) => {
      await limaApi.stop(p.name);
      return `Lima VM '${p.name}' stopped.`;
    }
  },
  "lima-delete": {
    category: "DANGEROUS",
    description: "Delete a lima vm",
    handler: async (p) => {
      await limaApi.delete(p.name);
      return `Lima VM '${p.name}' deleted.`;
    }
  },
  "lima-create": {
    category: "NORMAL",
    description: "Create a lima vm from a template url",
    handler: async (p) => {
      await limaApi.create({ name: p.name, template: p.templateUrl });
      return `Lima VM '${p.name}' created from template.`;
    }
  },
  "lima-info": {
    category: "SAFE",
    description: "Get Lima system info",
    handler: async () => JSON.stringify(await limaApi.info(), null, 2)
  },
  "lima-templates": {
    category: "SAFE",
    description: "List available Lima VM templates",
    handler: async () => JSON.stringify(await limaApi.templates(), null, 2)
  },
  "lima-shell": {
    category: "NORMAL",
    description: "Execute a command inside a Lima VM",
    handler: async (p) => {
      const result = await limaApi.shell(p.name, p.command);
      return String(result);
    }
  }
};

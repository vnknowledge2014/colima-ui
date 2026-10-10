import { isRunningInTauri } from "../env";
import { runSandboxed } from "./shell";
import { EventHandler } from "./types";
import { dockerApi } from "../api";

export const dockerRegistry: Record<string, EventHandler> = {
  "list-containers": {
    category: "SAFE",
    description: "List all containers",
    handler: async () => JSON.stringify(await dockerApi.listContainers(), null, 2)
  },
  "list-images": {
    category: "SAFE",
    description: "List all images",
    handler: async () => JSON.stringify(await dockerApi.listImages(), null, 2)
  },
  "start-container": {
    category: "NORMAL",
    description: "Start a container",
    handler: async (p) => {
      await dockerApi.startContainer(p.id);
      return `Container '${p.id}' started.`;
    }
  },
  "stop-container": {
    category: "NORMAL",
    description: "Stop a container",
    handler: async (p) => {
      await dockerApi.stopContainer(p.id);
      return `Container '${p.id}' stopped.`;
    }
  },
  "restart-container": {
    category: "NORMAL",
    description: "Restart a container",
    handler: async (p) => {
      await dockerApi.restartContainer(p.id);
      return `Container '${p.id}' restarted.`;
    }
  },
  "remove-container": {
    category: "DANGEROUS",
    description: "Remove a container",
    handler: async (p) => {
      await dockerApi.removeContainer(p.id);
      return `Container '${p.id}' removed.`;
    }
  },
  "remove-image": {
    category: "DANGEROUS",
    description: "Remove an image",
    handler: async (p) => {
      await dockerApi.removeImage(p.id);
      return `Image '${p.id}' removed.`;
    }
  },
  "pull-image": {
    category: "NORMAL",
    description: "Pull an image",
    handler: async (p) => {
      await dockerApi.pullImage(p.image);
      return `Image '${p.image}' pulled.`;
    }
  },
  "build-image": {
    category: "NORMAL",
    description: "Build a docker image from a Dockerfile",
    handler: async (p) => {
      if (isRunningInTauri()) {
        const args = ["build"];
        if (p.tag) { args.push("-t", p.tag); }
        if (p.file) { args.push("-f", p.file); }
        args.push(p.dir || ".");
        return await runSandboxed("docker", args);
      }
      return `[SIMULATED] Built image ${p.tag || "untagged"}`;
    }
  },
  "prune-images": {
    category: "DANGEROUS",
    description: "Prune unused images",
    handler: async () => {
      await dockerApi.pruneImages();
      return `Unused images pruned.`;
    }
  },
  "container-logs": {
    category: "SAFE",
    description: "Get logs from a container",
    handler: async (p) => await dockerApi.containerLogs(p.id, p.lines || 200)
  },
  "container-stats": {
    category: "SAFE",
    description: "Get resource usage stats for a container",
    handler: async (p) => JSON.stringify(await dockerApi.containerStats(p.id), null, 2)
  },
  "all-container-stats": {
    category: "SAFE",
    description: "Get resource usage stats for all containers",
    handler: async () => JSON.stringify(await dockerApi.allContainerStats(), null, 2)
  },
  "container-top": {
    category: "SAFE",
    description: "Get running processes in a container",
    handler: async (p) => JSON.stringify(await dockerApi.containerTop(p.id), null, 2)
  },
  "inspect-container": {
    category: "SAFE",
    description: "Inspect a container (full JSON config)",
    handler: async (p) => JSON.stringify(await dockerApi.inspectContainer(p.id), null, 2)
  },
  "inspect-image": {
    category: "SAFE",
    description: "Inspect an image (layers, config, size)",
    handler: async (p) => JSON.stringify(await dockerApi.inspectImage(p.id), null, 2)
  },
  "container-exec": {
    category: "NORMAL",
    description: "Execute a command inside a running container",
    handler: async (p) => {
      const result = await dockerApi.containerExec(p.id, p.command);
      return String(result);
    }
  },
  "run-container": {
    category: "NORMAL",
    description: "Run a new container from an image",
    handler: async (p) => {
      await dockerApi.runContainer(p.image, p.name || "", p.ports || [], p.envVars || [], p.volumes || [], p.detach !== false, p.removeOnExit || false, p.extraArgs || []);
      return `Container from image '${p.image}' started.`;
    }
  },
  "pause-container": {
    category: "NORMAL",
    description: "Pause a running container",
    handler: async (p) => {
      await dockerApi.pauseContainer(p.id);
      return `Container '${p.id}' paused.`;
    }
  },
  "unpause-container": {
    category: "NORMAL",
    description: "Unpause a paused container",
    handler: async (p) => {
      await dockerApi.unpauseContainer(p.id);
      return `Container '${p.id}' unpaused.`;
    }
  },
  "rename-container": {
    category: "NORMAL",
    description: "Rename a container",
    handler: async (p) => {
      await dockerApi.renameContainer(p.id, p.newName);
      return `Container '${p.id}' renamed to '${p.newName}'.`;
    }
  },
  "tag-image": {
    category: "NORMAL",
    description: "Tag an image with a new name",
    handler: async (p) => {
      await dockerApi.tagImage(p.source, p.target);
      return `Image '${p.source}' tagged as '${p.target}'.`;
    }
  }
};

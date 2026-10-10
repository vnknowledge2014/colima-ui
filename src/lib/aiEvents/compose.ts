import { isRunningInTauri } from "../env";
import { runSandboxed } from "./shell";
import { EventHandler } from "./types";
import { composeApi } from "../api";

export const composeRegistry: Record<string, EventHandler> = {
  "compose-list": {
    category: "SAFE",
    description: "List docker compose projects",
    handler: async () => JSON.stringify(await composeApi.list(), null, 2)
  },
  "compose-build": {
    category: "NORMAL",
    description: "Build or rebuild services in a compose project",
    handler: async (p) => {
      if (isRunningInTauri()) {
        const args = ["compose"];
        if (p.file) args.push("-f", p.file);
        if (p.dir) args.push("--project-directory", p.dir);
        args.push("build");
        return await runSandboxed("docker", args);
      }
      return `[SIMULATED] Compose build for ${p.dir || "current dir"}`;
    }
  },
  "compose-pull": {
    category: "NORMAL",
    description: "Pull images associated with a compose project",
    handler: async (p) => {
      if (isRunningInTauri()) {
        const args = ["compose"];
        if (p.file) args.push("-f", p.file);
        if (p.dir) args.push("--project-directory", p.dir);
        args.push("pull");
        return await runSandboxed("docker", args);
      }
      return `[SIMULATED] Compose pull for ${p.dir || "current dir"}`;
    }
  },
  "compose-up": {
    category: "NORMAL",
    description: "Docker compose up",
    handler: async (p) => {
      await composeApi.up(p.dir, p.file);
      return `Compose project in '${p.dir}' started.`;
    }
  },
  "compose-down": {
    category: "NORMAL",
    description: "Stop and remove compose containers",
    handler: async (p) => {
      await composeApi.down(p.dir || p.projectName); // use projectName as dir was removed from api
      return `Compose project in '${p.dir || p.projectName}' stopped.`;
    }
  },
  "compose-restart": {
    category: "NORMAL",
    description: "Restart compose containers",
    handler: async (p) => {
      await composeApi.restart(p.dir || p.projectName);
      return `Compose project in '${p.dir || p.projectName}' restarted.`;
    }
  },
  "compose-logs": {
    category: "SAFE",
    description: "Get logs from compose containers",
    handler: async (p) => {
      const logs = await composeApi.logs(p.dir || p.projectName, p.lines || 100);
      return logs;
    }
  }
};

import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Compose API =====

export interface ComposeProject {
  Name: string;
  Status: string;
  ConfigFiles: string;
}

export const composeApi = {
  list: async (): Promise<ComposeProject[]> => {
    const raw = await call<any>("list_compose_projects", undefined, "GET", "/api/compose");
    if (!raw) return [];

    // Normalize field names: Tauri IPC returns snake_case (name, status, config_files)
    // but TypeScript interface expects PascalCase (Name, Status, ConfigFiles)
    const normalize = (items: any[]): ComposeProject[] =>
      items.map((v: any) => ({
        Name: v.Name || v.name || "",
        Status: v.Status || v.status || "",
        ConfigFiles: v.ConfigFiles || v.config_files || v.configFiles || "",
      }));

    // Tauri IPC may return parsed array directly
    if (Array.isArray(raw)) return normalize(raw);
    if (typeof raw === 'string') {
      if (!raw.trim()) return [];
      try { return normalize(JSON.parse(raw)); } catch { return []; }
    }
    return [];
  },
  up: (projectDir = "", detach = true) =>
    call<string>("compose_up", { projectDir, detach }, "POST", "/api/compose/up", undefined, { projectDir, detach }),
  down: (projectName: string) =>
    call<string>("compose_down", { projectName }, "POST", "/api/compose/down", undefined, { projectName }),
  restart: (projectName: string) =>
    call<string>("compose_restart", { projectName }, "POST", "/api/compose/restart", undefined, { projectName }),
  logs: (projectName: string, lines = 200) =>
    call<string>("compose_logs", { projectName, lines }, "GET", "/api/compose/logs", { projectName, lines: String(lines) }),
  ps: (projectName: string) =>
    call<string>("compose_ps", { projectName }, "GET", "/api/compose/ps", { projectName }),
};

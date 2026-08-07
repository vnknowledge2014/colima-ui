import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Settings API =====

export const settingsApi = {
  getAll: () =>
    call<Record<string, string>>("get_all_settings", undefined, "GET", "/api/settings"),
  get: (key: string) =>
    call<string | null>("get_setting", { key }, "GET", "/api/settings", { key }),
  set: (key: string, value: string) =>
    call<string>("set_setting", { key, value }, "POST", "/api/settings", undefined, { key, value }),
};

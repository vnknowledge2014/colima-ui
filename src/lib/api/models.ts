import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Models API =====

export const modelsApi = {
  listModels: (profile: string, runner = "") =>
    call<AiModel[]>("list_models", { profile, runner: runner || undefined }, "GET", "/api/models", { profile, ...(runner ? { runner } : {}) }),

  pullModel: (profile: string, modelName: string, runner = "") =>
    call<string>("pull_model", { profile, modelName, runner: runner || undefined }, "POST", "/api/models/pull", { profile, modelName, ...(runner ? { runner } : {}) }),

  serveModel: (profile: string, modelName: string, port: number, runner = "") =>
    call<string>("serve_model", { profile, modelName, port, runner: runner || undefined }, "POST", "/api/models/serve", { profile, modelName, port: String(port), ...(runner ? { runner } : {}) }),

  deleteModel: (profile: string, modelName: string, runner = "") =>
    call<string>("delete_model", { profile, modelName, runner: runner || undefined }, "POST", "/api/models/delete", { profile, modelName, ...(runner ? { runner } : {}) }),
};

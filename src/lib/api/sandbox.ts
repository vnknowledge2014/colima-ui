import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Shell Sandbox API =====

export const sandboxApi = {
  execute: (command: string) =>
    call<{ stdout: string; stderr: string; exit_code: number }>(
      "sandbox_execute", { command }, "POST", "/api/sandbox/execute", undefined, { command }
    ),
  executeApproved: (command: string) =>
    call<{ stdout: string; stderr: string; exit_code: number }>(
      "sandbox_execute_approved", { command }, "POST", "/api/sandbox/execute-approved", undefined, { command }
    ),
};

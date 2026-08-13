import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig } from "./types";

// ===== Colima API =====

export const colimaApi = {
  listInstances: () =>
    call<ColimaInstance[]>("list_instances", undefined, "GET", "/api/instances"),

  startInstance: (config: StartConfig) =>
    call<string>("start_instance", { config }, "POST", "/api/instances/start", undefined, config),

  stopInstance: (profile: string, force = false) =>
    call<string>("stop_instance", { profile, force }, "POST", "/api/instances/stop", { profile, force: String(force) }),

  deleteInstance: (profile: string, force = true) =>
    call<string>("delete_instance", { profile, force }, "POST", "/api/instances/delete", { profile, force: String(force) }),

  instanceStatus: (profile: string) =>
    call<InstanceStatus>("instance_status", { profile }, "GET", "/api/instances/status", { profile }),

  getSshCommand: (profile: string) =>
    call<string[]>("get_ssh_command", { profile }, "GET", "/api/instances/ssh", { profile }),

  kubernetesAction: (profile: string, action: string) =>
    call<string>("kubernetes_action", { profile, action }, "POST", "/api/instances/k8s", { profile, action }),

  createWorkerNode: (masterProfile: string, workerProfile: string, cpus: number, memory: number) =>
    call<string>("create_worker_node", { master_profile: masterProfile, worker_profile: workerProfile, cpu: cpus, memory }, "POST", "/api/instances/create-worker", undefined, { master_profile: masterProfile, worker_profile: workerProfile, cpu: cpus, memory }),
};

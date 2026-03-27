/**
 * ColimaUI API Layer — Dual Mode (Tauri native + Browser HTTP)
 * 
 * Automatically detects if running inside Tauri window or a regular browser.
 * - Tauri: uses `invoke` IPC (fast, direct)
 * - Browser: uses `fetch` to HTTP API on port 11420
 */

// ===== Runtime Detection =====

const isTauri = (): boolean => {
  return !!(window as any).__TAURI_INTERNALS__;
};

const API_BASE = "http://127.0.0.1:11420";

// Lazy-loaded Tauri invoke to avoid import errors in browser
let _invoke: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;

async function getInvoke() {
  if (_invoke) return _invoke;
  try {
    const mod = await import("@tauri-apps/api/core");
    _invoke = mod.invoke;
    return _invoke;
  } catch {
    return null;
  }
}

// ===== Unified call function =====

async function call<T>(
  tauriCmd: string,
  tauriArgs: Record<string, unknown> | undefined,
  httpMethod: "GET" | "POST",
  httpPath: string,
  httpParams?: Record<string, string>,
  httpBody?: unknown
): Promise<T> {
  if (isTauri()) {
    const invoke = await getInvoke();
    if (invoke) {
      try {
        return await (invoke(tauriCmd, tauriArgs) as Promise<T>);
      } catch {
        // Tauri command not found — fall through to HTTP API
      }
    }
  }

  // Browser mode: use HTTP API
  let url = `${API_BASE}${httpPath}`;
  if (httpParams) {
    const params = new URLSearchParams(httpParams);
    url += `?${params.toString()}`;
  }

  const opts: RequestInit = {
    method: httpMethod,
    headers: { "Content-Type": "application/json" },
  };
  if (httpBody && httpMethod === "POST") {
    opts.body = JSON.stringify(httpBody);
  }

  const res = await fetch(url, opts);
  const json = await res.json();
  
  if (!json.success) {
    throw new Error(json.error || "API call failed");
  }
  return json.data as T;
}

// ===== Types =====

export interface ColimaInstance {
  name: string;
  status: string;
  arch: string;
  cpus: number;
  memory: number;
  disk: number;
  runtime: string;
  address: string;
  kubernetes: boolean;
}

export interface InstanceStatus {
  profile: string;
  status: string;
  arch: string;
  runtime: string;
  port_forwarding: string;
  cpu_usage: string;
  memory_usage: string;
  disk_usage: string;
  address: string;
}

export interface StartConfig {
  profile: string;
  runtime: string;
  cpus: number;
  memory: number;
  disk: number;
  vm_type: string;
  kubernetes: boolean;
  kubernetes_version: string;
  arch: string;
  mount_type: string;
  mounts: string[];
  dns: string[];
  network_address: boolean;
}

export interface DockerContainer {
  Id: string;
  Names: string;
  Image: string;
  Status: string;
  State: string;
  Ports: string;
  CreatedAt: string;
  Size: string;
  Command: string;
}

export interface DockerImage {
  Id: string;
  Repository: string;
  Tag: string;
  Size: string;
  CreatedAt: string;
}

export interface SystemInfo {
  colima_installed: boolean;
  colima_version: string;
  docker_installed: boolean;
  docker_version: string;
  lima_installed: boolean;
  lima_version: string;
}

export interface AiModel {
  name: string;
  size: string;
  format: string;
  family: string;
  parameters: string;
  quantization: string;
}

export interface DockerVolume {
  Name: string;
  Driver: string;
  Mountpoint: string;
  Scope: string;
  Labels: string;
}

export interface DockerNetwork {
  Id: string;
  Name: string;
  Driver: string;
  Scope: string;
  Ipv6: string;
  Internal: string;
  Labels: string;
}

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
};

// ===== Docker API =====

export const dockerApi = {
  listContainers: async (all = true): Promise<DockerContainer[]> => {
    const raw = await call<any>("list_containers", { all }, "GET", "/api/containers", { all: String(all) });
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    // Normalize field names for Tauri IPC compatibility
    return items.map((v: any) => ({
      Id: v.Id || v.id || v.ID || "",
      Names: v.Names || v.names || "",
      Image: v.Image || v.image || "",
      Status: v.Status || v.status || "",
      State: v.State || v.state || "",
      Ports: v.Ports || v.ports || "",
      CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
      Size: v.Size || v.size || "",
      Command: v.Command || v.command || "",
    }));
  },

  startContainer: (containerId: string) =>
    call<string>("start_container", { containerId }, "POST", "/api/containers/start", { containerId }),

  stopContainer: (containerId: string) =>
    call<string>("stop_container", { containerId }, "POST", "/api/containers/stop", { containerId }),

  restartContainer: (containerId: string) =>
    call<string>("restart_container", { containerId }, "POST", "/api/containers/restart", { containerId }),

  removeContainer: (containerId: string, force = false) =>
    call<string>("remove_container", { containerId, force }, "POST", "/api/containers/remove", { containerId, force: String(force) }),

  containerLogs: (containerId: string, lines = 200) =>
    call<string>("container_logs", { containerId, lines }, "GET", "/api/containers/logs", { containerId, lines: String(lines) }),

  listImages: async (): Promise<DockerImage[]> => {
    const raw = await call<any>("list_images", undefined, "GET", "/api/images");
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    return items.map((v: any) => ({
      Id: v.Id || v.id || v.ID || "",
      Repository: v.Repository || v.repository || "",
      Tag: v.Tag || v.tag || "",
      Size: v.Size || v.size || "",
      CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
    }));
  },

  inspectContainer: (containerId: string) =>
    call<string>("inspect_container", { containerId }, "GET", "/api/containers/inspect", { containerId }),

  // Image management
  removeImage: (imageId: string, force = false) =>
    call<string>("remove_image", { imageId, force }, "POST", "/api/images/remove", { imageId, force: String(force) }),

  pullImage: (imageName: string) =>
    call<string>("pull_image", { imageName }, "POST", "/api/images/pull", { imageName }),

  pruneImages: () =>
    call<string>("prune_images", undefined, "POST", "/api/images/prune"),

  inspectImage: (imageId: string) =>
    call<string>("inspect_image", { imageId }, "GET", "/api/images/inspect", { imageId }),

  tagImage: (source: string, target: string) =>
    call<string>("tag_image", { source, target }, "POST", "/api/images/tag", undefined, { source, target }),

  // Container enhancement
  containerStats: (containerId: string) =>
    call<string>("container_stats", { containerId }, "GET", "/api/containers/stats", { containerId }),

  allContainerStats: () =>
    call<string>("all_container_stats", undefined, "GET", "/api/containers/stats/all"),

  containerTop: (containerId: string) =>
    call<string>("container_top", { containerId }, "GET", "/api/containers/top", { containerId }),

  containerExec: (containerId: string, command: string) =>
    call<string>("container_exec", { containerId, command }, "POST", "/api/containers/exec", undefined, { containerId, command }),

  runContainer: (image: string, name = "", ports: string[] = [], envVars: string[] = [], volumes: string[] = [], detach = true, removeOnExit = false, extraArgs: string[] = []) =>
    call<string>("run_container", { image, name, ports, envVars, volumes, detach, removeOnExit, extraArgs }, "POST", "/api/containers/run", undefined, { image, name, ports, envVars, volumes, detach, removeOnExit, extraArgs }),

  renameContainer: (containerId: string, newName: string) =>
    call<string>("rename_container", { containerId, newName }, "POST", "/api/containers/rename", undefined, { containerId, newName }),

  pauseContainer: (containerId: string) =>
    call<string>("pause_container", { containerId }, "POST", "/api/containers/pause", { containerId }),

  unpauseContainer: (containerId: string) =>
    call<string>("unpause_container", { containerId }, "POST", "/api/containers/unpause", undefined, { id: containerId }),
  systemDf: () =>
    call<string>("system_df", undefined, "GET", "/api/docker/df"),
  systemPrune: () =>
    call<string>("system_prune", undefined, "POST", "/api/docker/prune"),
};

// ===== Volumes API =====

export const volumesApi = {
  listVolumes: async (): Promise<DockerVolume[]> => {
    const raw = await call<any>("list_volumes", undefined, "GET", "/api/volumes");
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    return items.map((v: any) => ({
      Name: v.Name || v.name || "",
      Driver: v.Driver || v.driver || "",
      Mountpoint: v.Mountpoint || v.mountpoint || v.mount_point || "",
      Scope: v.Scope || v.scope || "",
      Labels: v.Labels || v.labels || "",
    }));
  },

  createVolume: (name: string, driver = "local") =>
    call<string>("create_volume", { name, driver }, "POST", "/api/volumes/create", undefined, { name, driver }),

  removeVolume: (name: string, force = false) =>
    call<string>("remove_volume", { name, force }, "POST", "/api/volumes/remove", { name, force: String(force) }),

  pruneVolumes: () =>
    call<string>("prune_volumes", undefined, "POST", "/api/volumes/prune"),

  inspectVolume: (name: string) =>
    call<string>("inspect_volume", { name }, "GET", "/api/volumes/inspect", { name }),
};

// ===== Networks API =====

export const networksApi = {
  listNetworks: async (): Promise<DockerNetwork[]> => {
    const raw = await call<any>("list_networks", undefined, "GET", "/api/networks");
    if (!raw) return [];
    const items = Array.isArray(raw) ? raw : [];
    // Normalize field names for Tauri IPC compatibility
    return items.map((v: any) => ({
      Id: v.Id || v.id || v.ID || "",
      Name: v.Name || v.name || "",
      Driver: v.Driver || v.driver || "",
      Scope: v.Scope || v.scope || "",
      Ipv6: v.Ipv6 || v.ipv6 || v.IPv6 || "",
      Internal: v.Internal || v.internal || "",
      Labels: v.Labels || v.labels || "",
    }));
  },

  createNetwork: (name: string, driver = "bridge", subnet = "") =>
    call<string>("create_network", { name, driver, subnet }, "POST", "/api/networks/create", undefined, { name, driver, subnet }),

  removeNetwork: (name: string) =>
    call<string>("remove_network", { name }, "POST", "/api/networks/remove", { name }),

  inspectNetwork: (name: string) =>
    call<string>("inspect_network", { name }, "GET", "/api/networks/inspect", { name }),

  pruneNetworks: () =>
    call<string>("prune_networks", undefined, "POST", "/api/networks/prune"),
};

// ===== System API =====

export interface PlatformInfo {
  os: "macos" | "linux" | "windows";
  arch: string;
  wsl: boolean;
  wsl_available: boolean;
  package_managers: Array<{ name: string; available: boolean; version: string }>;
}

export const systemApi = {
  checkSystem: () =>
    call<SystemInfo>("check_system", undefined, "GET", "/api/system/check"),
  getColimaVersion: () =>
    call<string>("get_colima_version", undefined, "GET", "/api/system/version"),
  systemPrune: (all = false) =>
    call<string>("system_prune", { all }, "POST", "/api/system/prune", { all: String(all) }),
  systemDf: () =>
    call<string>("system_df", undefined, "GET", "/api/system/df"),

  // Setup Wizard APIs
  getPlatform: () =>
    call<PlatformInfo>("get_platform", undefined, "GET", "/api/system/platform"),
  installDep: (name: "colima" | "docker" | "lima", method = "brew") =>
    call<{ success: boolean; output: string }>(
      "install_dependency", { name, method }, "POST", "/api/system/install", undefined, { name, method }
    ),
  checkHomebrew: () =>
    call<{ installed: boolean; version: string }>(
      "check_homebrew", undefined, "GET", "/api/system/homebrew"
    ),
  configureAutostart: (enable: boolean) =>
    call<string>(
      "configure_autostart", { enable }, "POST", "/api/system/autostart", undefined, { enable }
    ),
  getAutostartStatus: () =>
    call<{ enabled: boolean }>(
      "get_autostart_status", undefined, "GET", "/api/system/autostart"
    ),
  checkTool: (name: string) =>
    call<{ installed: boolean; version: string }>(
      "check_tool", { name }, "GET", "/api/system/check-tool", { name }
    ),
};

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

// ===== Kubernetes API =====

export const k8sApi = {
  check: () =>
    call<string>("k8s_check", undefined, "GET", "/api/k8s/check"),
  namespaces: () =>
    call<string>("k8s_namespaces", undefined, "GET", "/api/k8s/namespaces"),
  pods: (namespace = "all") =>
    call<string>("k8s_pods", { namespace }, "GET", "/api/k8s/pods", { namespace }),
  services: (namespace = "all") =>
    call<string>("k8s_services", { namespace }, "GET", "/api/k8s/services", { namespace }),
  deployments: (namespace = "all") =>
    call<string>("k8s_deployments", { namespace }, "GET", "/api/k8s/deployments", { namespace }),
  podLogs: (namespace: string, pod: string, lines = 200) =>
    call<string>("k8s_pod_logs", { namespace, pod, lines }, "GET", "/api/k8s/pods/logs", { namespace, pod, lines: String(lines) }),
  deletePod: (namespace: string, pod: string) =>
    call<string>("k8s_delete_pod", { namespace, pod }, "POST", "/api/k8s/pods/delete", undefined, { namespace, pod }),
  describe: (namespace: string, resourceType: string, name: string) =>
    call<string>("k8s_describe", { namespace, resourceType, name }, "GET", "/api/k8s/describe", { namespace, resourceType, name }),
  scale: (namespace: string, deployment: string, replicas: number) =>
    call<string>("k8s_scale", { namespace, deployment, replicas }, "POST", "/api/k8s/scale", undefined, { namespace, deployment, replicas }),
  nodes: () =>
    call<string>("k8s_nodes", undefined, "GET", "/api/k8s/nodes"),
  events: (namespace = "all") =>
    call<string>("k8s_events", { namespace }, "GET", "/api/k8s/events", { namespace }),
  // Phase 1: New endpoints
  resources: (resource: string, namespace = "all") =>
    call<string>("k8s_resources", { resource, namespace }, "GET", "/api/k8s/resources", { resource, namespace }),
  deleteResource: (resourceType: string, namespace: string, name: string) =>
    call<string>("k8s_delete_resource", { resourceType, namespace, name }, "POST", "/api/k8s/resources/delete", undefined, { resourceType, namespace, name }),
  restart: (resourceType: string, namespace: string, name: string) =>
    call<string>("k8s_restart", { resourceType, namespace, name }, "POST", "/api/k8s/resources/restart", undefined, { resourceType, namespace, name }),
  yaml: (resourceType: string, namespace: string, name: string) =>
    call<string>("k8s_yaml", { resourceType, namespace, name }, "GET", "/api/k8s/resources/yaml", { resourceType, namespace, name }),
  nodesJson: () =>
    call<string>("k8s_nodes_json", undefined, "GET", "/api/k8s/nodes/json"),
  eventsJson: (namespace = "all") =>
    call<string>("k8s_events_json", { namespace }, "GET", "/api/k8s/events/json", { namespace }),
  contexts: () =>
    call<string>("k8s_contexts", undefined, "GET", "/api/k8s/contexts"),
  currentContext: () =>
    call<string>("k8s_current_context", undefined, "GET", "/api/k8s/contexts/current"),
  setContext: (context: string) =>
    call<string>("k8s_set_context", { context }, "POST", "/api/k8s/contexts/set", undefined, { context }),
  // Phase 2
  apply: (yaml: string, namespace = "") =>
    call<string>("k8s_apply", { yaml, namespace }, "POST", "/api/k8s/apply", undefined, { yaml, namespace }),
  portForwardStart: (namespace: string, name: string, localPort: number, remotePort: number, resourceType = "pod") =>
    call<string>("k8s_pf_start", { namespace, name, localPort, remotePort, resourceType }, "POST", "/api/k8s/port-forward/start", undefined, { namespace, name, localPort, remotePort, resourceType }),
  portForwardStop: (localPort: number) =>
    call<string>("k8s_pf_stop", { localPort }, "POST", "/api/k8s/port-forward/stop", undefined, { localPort }),
  portForwardList: () =>
    call<string>("k8s_pf_list", undefined, "GET", "/api/k8s/port-forward/list"),
  exec: (namespace: string, pod: string, container = "") =>
    call<string>("k8s_exec", { namespace, pod, container }, "POST", "/api/k8s/exec", undefined, { namespace, pod, container }),
  podContainers: (namespace: string, pod: string) =>
    call<string>("k8s_pod_containers", { namespace, pod }, "GET", "/api/k8s/pods/containers", { namespace, pod }),
  containerLogs: (namespace: string, pod: string, container = "", lines = 200, previous = false) =>
    call<string>("k8s_container_logs", { namespace, pod, container, lines, previous }, "GET", "/api/k8s/pods/container-logs", { namespace, pod, container, lines: String(lines), previous: String(previous) }),
  nodeAction: (name: string, action: string) =>
    call<string>("k8s_node_action", { name, action }, "POST", "/api/k8s/nodes/action", undefined, { name, action }),
  // Phase 3
  genericScale: (resourceType: string, namespace: string, name: string, replicas: number) =>
    call<string>("k8s_generic_scale", { resourceType, namespace, name, replicas }, "POST", "/api/k8s/scale-generic", undefined, { resourceType, namespace, name, replicas }),
  clusterHealth: () =>
    call<string>("k8s_cluster_health", undefined, "GET", "/api/k8s/cluster-health"),
  // CRDs
  crds: () =>
    call<string>("k8s_crds", undefined, "GET", "/api/k8s/crds"),
  crdResources: (resource: string, namespace = "all") =>
    call<string>("k8s_crd_resources", { resource, namespace }, "GET", "/api/k8s/crds/resources", { resource, namespace }),
  // Log streaming — returns URL for EventSource (SSE)
  logStreamUrl: (namespace: string, pod: string, container = "", tail = 50) => {
    const params = new URLSearchParams({ namespace, pod, tail: String(tail) });
    if (container) params.set("container", container);
    return `http://127.0.0.1:11420/api/k8s/pods/logs/stream?${params}`;
  },
  // Benchmark
  benchmark: (url: string, concurrency = 5, requests = 50, method = "GET") =>
    call<string>("k8s_benchmark", { url, concurrency, requests, method }, "POST", "/api/k8s/benchmark", undefined, { url, concurrency, requests, method }),
};

// ===== Kind API =====

export const kindApi = {
  list: () =>
    call<string>("kind_list", undefined, "GET", "/api/kind"),
  create: (name: string, image = "") =>
    call<string>("kind_create", { name, image }, "POST", "/api/kind/create", undefined, { name, image }),
  delete: (name: string) =>
    call<string>("kind_delete", { name }, "POST", "/api/kind/delete", undefined, { name }),
};

// ===== Lima API =====

export interface LimaInstance {
  name: string;
  status: string;
  arch: string;
  cpus: string;
  memory: string;
  disk: string;
  dir: string;
}

export const limaApi = {
  list: async (): Promise<LimaInstance[]> => {
    const raw = await call<any>("lima_list", undefined, "GET", "/api/lima");
    if (!raw) return [];
    // Tauri IPC may return parsed array directly
    if (Array.isArray(raw)) return raw.map((v: any) => ({
      name: v.name || "",
      status: v.status || "Unknown",
      arch: v.arch || "",
      cpus: String(v.cpus || 0),
      memory: v.memory ? (typeof v.memory === 'number' ? formatLimaBytes(v.memory) : v.memory) : "0",
      disk: v.disk ? (typeof v.disk === 'number' ? formatLimaBytes(v.disk) : v.disk) : "0",
      dir: v.dir || "",
    }));
    if (typeof raw !== 'string') return [];
    if (!raw.trim()) return [];
    try {
      return raw.split("\n").filter((l: string) => l.trim()).map((l: string) => {
        const v = JSON.parse(l);
        return {
          name: v.name || "",
          status: v.status || "Unknown",
          arch: v.arch || "",
          cpus: String(v.cpus || 0),
          memory: v.memory ? formatLimaBytes(v.memory) : "0",
          disk: v.disk ? formatLimaBytes(v.disk) : "0",
          dir: v.dir || "",
        };
      });
    } catch { return []; }
  },
  start: (name: string) =>
    call<string>("lima_start", { name }, "POST", "/api/lima/start", undefined, { name }),
  stop: (name: string) =>
    call<string>("lima_stop", { name }, "POST", "/api/lima/stop", undefined, { name }),
  delete: (name: string, force = false) =>
    call<string>("lima_delete", { name, force }, "POST", "/api/lima/delete", undefined, { name, force }),
  info: () =>
    call<string>("lima_info", { name: "" }, "GET", "/api/lima/info"),
  shell: (name: string, command: string) =>
    call<string>("lima_shell", { name, command }, "POST", "/api/lima/shell", undefined, { name, command }),
  templates: () =>
    call<string>("lima_templates", undefined, "GET", "/api/lima/templates"),
  create: (config: { name: string; cpus?: number; memory?: number; disk?: number; template?: string }) =>
    call<string>("lima_create", config, "POST", "/api/lima/create", undefined, config),
};

function formatLimaBytes(bytes: number): string {
  if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
  if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
  return `${bytes} B`;
}

// ===== AI Chat API =====

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export const aiApi = {
  chat: (provider: string, model: string, apiKey: string, messages: ChatMessage[], endpoint = "") =>
    call<string>("ai_chat", {
      request: { provider, model, api_key: apiKey, messages, endpoint }
    }, "POST", "/api/ai/chat", undefined, {
      provider, model, api_key: apiKey, messages, endpoint
    }),
  listModels: (provider: string, apiKey: string, endpoint = "") =>
    call<string>("ai_list_models", {
      provider, api_key: apiKey, endpoint
    }, "POST", "/api/ai/models", undefined, {
      provider, api_key: apiKey, endpoint
    }),
};


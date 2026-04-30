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
      return invoke(tauriCmd, tauriArgs) as Promise<T>;
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
  listContainers: (all = true) =>
    call<DockerContainer[]>("list_containers", { all }, "GET", "/api/containers", { all: String(all) }),

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

  listImages: () =>
    call<DockerImage[]>("list_images", undefined, "GET", "/api/images"),

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
  listVolumes: () =>
    call<DockerVolume[]>("list_volumes", undefined, "GET", "/api/volumes"),

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
  listNetworks: () =>
    call<DockerNetwork[]>("list_networks", undefined, "GET", "/api/networks"),

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
  os: string;
  arch: string;
  wsl: boolean;
  wsl_available: boolean;
  package_managers: { name: string; available: boolean; version: string }[];
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
  // SetupWizard helpers — graceful stubs until backend commands are implemented
  getPlatform: async (): Promise<PlatformInfo> => {
    try {
      return await call<PlatformInfo>("get_platform", undefined, "GET", "/api/system/platform");
    } catch {
      // Fallback: detect from browser UA
      const ua = navigator.userAgent.toLowerCase();
      const os = ua.includes("mac") ? "macos" : ua.includes("linux") ? "linux" : ua.includes("win") ? "windows" : "macos";
      return { os, arch: "aarch64", wsl: false, wsl_available: false, package_managers: [{ name: "brew", available: true, version: "" }] };
    }
  },
  checkHomebrew: async (): Promise<{ installed: boolean; version: string }> => {
    try {
      return await call<{ installed: boolean; version: string }>("check_homebrew", undefined, "GET", "/api/system/homebrew");
    } catch {
      return { installed: false, version: "" };
    }
  },
  checkTool: async (name: string): Promise<{ installed: boolean; version: string }> => {
    try {
      return await call<{ installed: boolean; version: string }>("check_tool", { name }, "GET", "/api/system/tool", { name });
    } catch {
      return { installed: false, version: "" };
    }
  },
  installDep: async (name: string, method: string): Promise<{ success: boolean }> => {
    try {
      return await call<{ success: boolean }>("install_dep", { name, method }, "POST", "/api/system/install", undefined, { name, method });
    } catch {
      return { success: false };
    }
  },
  configureAutostart: async (enable: boolean): Promise<void> => {
    try {
      await call<string>("configure_autostart", { enable }, "POST", "/api/system/autostart", undefined, { enable });
    } catch {
      // Silently fail — not critical
    }
  },
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
    // Tauri IPC may return parsed array directly
    if (Array.isArray(raw)) return raw;
    if (typeof raw === 'string') {
      if (!raw.trim()) return [];
      try { return JSON.parse(raw); } catch { return []; }
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
  listModels: (profile: string) =>
    call<AiModel[]>("list_models", { profile }, "GET", "/api/models", { profile }),

  pullModel: (profile: string, modelName: string) =>
    call<string>("pull_model", { profile, modelName }, "POST", "/api/models/pull", { profile, modelName }),

  serveModel: (profile: string, modelName: string, port: number) =>
    call<string>("serve_model", { profile, modelName, port }, "POST", "/api/models/serve", { profile, modelName, port: String(port) }),

  deleteModel: (profile: string, modelName: string) =>
    call<string>("delete_model", { profile, modelName }, "POST", "/api/models/delete", { profile, modelName }),
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

export interface SearchResult {
  title: string;
  url: string;
  content: string;
  engine: string;
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
  search: (query: string, instances: string[] = [], maxResults = 5) =>
    call<SearchResult[]>("searxng_search", {
      query, instances: instances.length > 0 ? instances : null, maxResults, timeoutSecs: null
    }, "POST", "/api/ai/search", undefined, {
      query, instances: instances.length > 0 ? instances : null, max_results: maxResults
    }),
  fetchPageMarkdown: (url: string, maxLength = 8000, mode = "full") =>
    call<string>("fetch_page_as_markdown", {
      url, maxLength, mode
    }, "POST", "/api/ai/fetch-page", undefined, {
      url, max_length: maxLength, mode
    }),
};


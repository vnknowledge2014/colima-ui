import { call, resolveApiBase } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

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
  // Async because the server may bind a fallback port — see resolveApiBase().
  logStreamUrl: async (namespace: string, pod: string, container = "", tail = 50) => {
    const params = new URLSearchParams({ namespace, pod, tail: String(tail) });
    if (container) params.set("container", container);
    return `${await resolveApiBase()}/api/k8s/pods/logs/stream?${params}`;
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

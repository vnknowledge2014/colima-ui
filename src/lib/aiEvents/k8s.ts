import { EventHandler } from "./types";
import { openTerminalSession } from "../../store.svelte";
import { k8sApi, kindApi } from "../api";

export const k8sRegistry: Record<string, EventHandler> = {
  "k8s-list-contexts": {
    category: "SAFE",
    description: "List all contexts",
    handler: async () => JSON.stringify(await k8sApi.contexts(), null, 2)
  },
  "k8s-current-context": {
    category: "SAFE",
    description: "Get current context",
    handler: async () => await k8sApi.currentContext()
  },
  "k8s-set-context": {
    category: "NORMAL",
    description: "Set current context",
    handler: async (p) => await k8sApi.setContext(p.context)
  },
  "k8s-list-namespaces": {
    category: "SAFE",
    description: "List namespaces",
    handler: async () => JSON.stringify(await k8sApi.namespaces(), null, 2)
  },
  "k8s-list-resources": {
    category: "SAFE",
    description: "List resources of a specific type in a namespace",
    handler: async (p) => JSON.stringify(await k8sApi.resources(p.kind, p.namespace), null, 2)
  },
  "k8s-describe": {
    category: "SAFE",
    description: "Describe a resource",
    handler: async (p) => await k8sApi.describe(p.namespace, p.kind, p.name)
  },
  "k8s-logs": {
    category: "SAFE",
    description: "Get logs from a pod",
    handler: async (p) => await k8sApi.podLogs(p.namespace, p.podName, p.lines || 100)
  },
  "k8s-apply": {
    category: "NORMAL",
    description: "Apply a yaml manifest (file path)",
    handler: async (p) => {
      await k8sApi.apply(p.filePath);
      return `Applied manifest '${p.filePath}'.`;
    }
  },
  "k8s-delete": {
    category: "DANGEROUS",
    description: "Delete a kubernetes resource",
    handler: async (p) => {
      await k8sApi.deleteResource(p.kind, p.namespace, p.name);
      return `Resource '${p.kind}/${p.name}' in namespace '${p.namespace}' deleted.`;
    }
  },
  "k8s-benchmark": {
    category: "SAFE",
    description: "Run k8s cluster benchmark",
    handler: async (p) => JSON.stringify(await k8sApi.benchmark(p.url, p.concurrency, p.requests, p.method), null, 2)
  },
  "k8s-events": {
    category: "SAFE",
    description: "Get cluster events",
    handler: async (p) => JSON.stringify(await k8sApi.events(p.namespace || "all"), null, 2)
  },
  "k8s-yaml": {
    category: "SAFE",
    description: "Get YAML manifest of a resource",
    handler: async (p) => await k8sApi.yaml(p.resourceType, p.namespace, p.name)
  },
  "k8s-cluster-health": {
    category: "SAFE",
    description: "Get cluster health status",
    handler: async () => JSON.stringify(await k8sApi.clusterHealth(), null, 2)
  },
  "k8s-nodes": {
    category: "SAFE",
    description: "List cluster nodes",
    handler: async () => await k8sApi.nodes()
  },
  "k8s-crds": {
    category: "SAFE",
    description: "List custom resource definitions",
    handler: async () => JSON.stringify(await k8sApi.crds(), null, 2)
  },
  "k8s-crd-resources": {
    category: "SAFE",
    description: "List resources for a CRD",
    handler: async (p) => JSON.stringify(await k8sApi.crdResources(p.resource, p.namespace || "all"), null, 2)
  },
  "k8s-pod-containers": {
    category: "SAFE",
    description: "List containers in a pod",
    handler: async (p) => JSON.stringify(await k8sApi.podContainers(p.namespace, p.pod), null, 2)
  },
  "k8s-container-logs": {
    category: "SAFE",
    description: "Get logs from a specific container in a pod",
    handler: async (p) => await k8sApi.containerLogs(p.namespace, p.pod, p.container || "", p.lines || 200, p.previous || false)
  },
  "k8s-port-forward-list": {
    category: "SAFE",
    description: "List active port forwards",
    handler: async () => JSON.stringify(await k8sApi.portForwardList(), null, 2)
  },
  "k8s-scale": {
    category: "NORMAL",
    description: "Scale a deployment",
    handler: async (p) => {
      await k8sApi.scale(p.namespace, p.deployment, p.replicas);
      return `Deployment '${p.deployment}' scaled to ${p.replicas} replicas.`;
    }
  },
  "k8s-restart-resource": {
    category: "NORMAL",
    description: "Restart a K8s resource (rolling restart)",
    handler: async (p) => {
      await k8sApi.restart(p.resourceType, p.namespace, p.name);
      return `${p.resourceType} '${p.name}' restarted.`;
    }
  },
  "k8s-port-forward-start": {
    category: "NORMAL",
    description: "Start port forwarding",
    handler: async (p) => {
      await k8sApi.portForwardStart(p.namespace, p.name, p.localPort, p.remotePort, p.resourceType || "pod");
      return `Port forward started: localhost:${p.localPort} → ${p.name}:${p.remotePort}`;
    }
  },
  "k8s-port-forward-stop": {
    category: "NORMAL",
    description: "Stop port forwarding",
    handler: async (p) => {
      await k8sApi.portForwardStop(p.localPort);
      return `Port forward on port ${p.localPort} stopped.`;
    }
  },
  "k8s-node-action": {
    category: "DANGEROUS",
    description: "Perform action on a node (cordon/uncordon/drain)",
    handler: async (p) => {
      await k8sApi.nodeAction(p.name, p.action);
      return `Node '${p.name}' ${p.action} completed.`;
    }
  },
  "k8s-generic-scale": {
    category: "NORMAL",
    description: "Scale any scalable resource type",
    handler: async (p) => {
      await k8sApi.genericScale(p.resourceType, p.namespace, p.name, p.replicas);
      return `${p.resourceType} '${p.name}' scaled to ${p.replicas} replicas.`;
    }
  },
  "k8s-exec": {
    category: "NORMAL",
    description: "Open exec session into a pod",
    handler: async (p) => {
      // Opens a tab in the app's Terminal page. This used to go through
      // `k8sApi.exec`, which launched Terminal.app via osascript — the agent
      // would report success while the shell appeared outside the app.
      openTerminalSession({
        kind: "k8sExec",
        namespace: p.namespace,
        pod: p.pod,
        container: p.container || "",
      });
      return `Exec session opened for pod '${p.pod}' in the Terminal tab.`;
    }
  },

  "kind-list": {
    category: "SAFE",
    description: "List kind clusters",
    handler: async () => JSON.stringify(await kindApi.list(), null, 2)
  },
  "kind-create": {
    category: "NORMAL",
    description: "Create a kind cluster",
    handler: async (p) => {
      await kindApi.create(p.name);
      return `Kind cluster '${p.name}' created.`;
    }
  },
  "kind-delete": {
    category: "DANGEROUS",
    description: "Delete a kind cluster",
    handler: async (p) => {
      await kindApi.delete(p.name);
      return `Kind cluster '${p.name}' deleted.`;
    }
  }
};

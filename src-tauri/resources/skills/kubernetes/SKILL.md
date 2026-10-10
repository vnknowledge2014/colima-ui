---
name: kubernetes
description: Kubernetes cluster management through App API. Resource listing, scaling, port-forwarding, and troubleshooting.
---

# Kubernetes Management Skill

Manage K8s clusters (K3s inside Colima, Kind, or remote) via the App Event Bus.

## Core Rule: App-First

**ALWAYS use `[QUERY]` for read-only and `[EVENT_APPROVE]` for mutations.** Only use CLI via `cli-exec` for `helm` commands and advanced CRD operations.

## 1. Cluster Context & Discovery (Read-Only)

- **List contexts**: `[QUERY: k8s-list-contexts]`
- **Current context**: `[QUERY: k8s-current-context]`
- **List namespaces**: `[QUERY: k8s-list-namespaces]`
- **Cluster health**: `[QUERY: k8s-cluster-health]`
- **List nodes**: `[QUERY: k8s-nodes]`
- **Cluster events**: `[QUERY: k8s-events | {"namespace": "all"}]`

## 2. Resource Introspection (Read-Only)

- **List resources**: `[QUERY: k8s-list-resources | {"kind": "pods", "namespace": "default"}]`
  Supported kinds: pods, deployments, services, statefulsets, daemonsets, jobs, cronjobs, ingresses, configmaps, secrets, pvcs, pvs, events, nodes
- **Describe resource**: `[QUERY: k8s-describe | {"namespace": "default", "kind": "pod", "name": "nginx-xyz"}]`
- **Get YAML**: `[QUERY: k8s-yaml | {"resourceType": "deployment", "namespace": "default", "name": "api"}]`
- **Pod logs**: `[QUERY: k8s-logs | {"namespace": "default", "podName": "nginx-xyz", "lines": 100}]`
- **Container logs**: `[QUERY: k8s-container-logs | {"namespace": "default", "pod": "nginx-xyz", "container": "sidecar", "lines": 100}]`
- **Pod containers**: `[QUERY: k8s-pod-containers | {"namespace": "default", "pod": "nginx-xyz"}]`
- **CRDs**: `[QUERY: k8s-crds]`
- **CRD resources**: `[QUERY: k8s-crd-resources | {"resource": "certificates.cert-manager.io", "namespace": "all"}]`
- **Port forwards**: `[QUERY: k8s-port-forward-list]`
- **Benchmark**: `[QUERY: k8s-benchmark | {"url": "http://localhost:8080", "concurrency": 5, "requests": 50}]`

## 3. Mutations (State Changes)

- **Switch context**: `[EVENT_APPROVE: k8s-set-context | {"context": "colima"}]`
- **Scale deployment**: `[EVENT_APPROVE: k8s-scale | {"namespace": "default", "deployment": "api", "replicas": 3}]`
- **Scale any resource**: `[EVENT_APPROVE: k8s-generic-scale | {"resourceType": "statefulset", "namespace": "default", "name": "db", "replicas": 2}]`
- **Restart resource**: `[EVENT_APPROVE: k8s-restart-resource | {"resourceType": "deployment", "namespace": "default", "name": "api"}]`
- **Apply manifest**: `[EVENT_APPROVE: k8s-apply | {"filePath": "/path/to/manifest.yaml"}]`
- **Port forward start**: `[EVENT_APPROVE: k8s-port-forward-start | {"namespace": "default", "name": "api-pod", "localPort": 8080, "remotePort": 80}]`
- **Port forward stop**: `[EVENT_APPROVE: k8s-port-forward-stop | {"localPort": 8080}]`
- **Exec into pod**: `[EVENT_APPROVE: k8s-exec | {"namespace": "default", "pod": "nginx-xyz"}]`
- **Delete resource**: `[EVENT_APPROVE: k8s-delete | {"kind": "pod", "namespace": "default", "name": "old-pod"}]` ⚠️ DANGEROUS
- **Node action**: `[EVENT_APPROVE: k8s-node-action | {"name": "node-1", "action": "cordon"}]` ⚠️ DANGEROUS (cordon/uncordon/drain)

## 4. CLI Escape Hatch

Only for operations NOT covered by the App API:
- **Helm**: `[EVENT_APPROVE: cli-exec | {"command": "helm", "args": ["install", "my-release", "bitnami/nginx"]}]`
- **Custom CRD mutations**: `[EVENT_APPROVE: cli-exec | {"command": "kubectl", "args": ["apply", "-f", "custom-crd.yaml"]}]`

## 5. Troubleshooting

### Pod stuck in CrashLoopBackOff
1. `[QUERY: k8s-logs | {"namespace": "default", "podName": "<pod>", "lines": 50}]`
2. `[QUERY: k8s-describe | {"namespace": "default", "kind": "pod", "name": "<pod>"}]`
3. Check Events section for OOM, image pull errors, or liveness probe failures.

### Node NotReady
1. `[QUERY: k8s-nodes]` — check which node is NotReady.
2. `[QUERY: k8s-events | {"namespace": "all"}]` — look for node-level events.

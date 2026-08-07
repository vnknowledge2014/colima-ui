import type { K8sResource } from "../store/k8s.svelte";

export function parseItems(raw: any): K8sResource[] {
  if (!raw) return [];
  try {
    const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
    const items = parsed.items || (Array.isArray(parsed) ? parsed : []);
    return items.map((item: any) => {
      const meta = item.metadata || {};
      const spec = item.spec || {};
      const status = item.status || {};
      const statuses = status.containerStatuses || [];

      const base: K8sResource = {
        name: meta.name || "",
        namespace: meta.namespace || "",
        age: meta.creationTimestamp || "",
        _raw: item,
      };

      if (statuses.length > 0 || status.phase) {
        const ready = statuses.filter((s: any) => s.ready).length;
        base.status = status.phase || "Unknown";
        base.ready = `${ready}/${statuses.length || spec.containers?.length || 0}`;
        base.restarts = String(statuses.reduce((s: number, c: any) => s + (c.restartCount || 0), 0));
        base.node = spec.nodeName || "";
      }

      if (spec.replicas !== undefined) {
        base.replicas = `${status.readyReplicas || 0}/${spec.replicas}`;
        base.available = String(status.availableReplicas || status.readyReplicas || 0);
      }

      if (spec.type) {
        base.svcType = spec.type;
        base.clusterIP = spec.clusterIP || "None";
        base.ports = (spec.ports || []).map((p: any) => `${p.port}/${p.protocol}`).join(", ");
        base._ports = spec.ports || [];
      }

      if (status.nodeInfo) {
        const conds = (status.conditions || []).find((c: any) => c.type === "Ready");
        base.status = conds?.status === "True" ? "Ready" : "NotReady";
        base.roles = (Object.keys(meta.labels || {})
          .filter((k: string) => k.startsWith("node-role.kubernetes.io/"))
          .map((k: string) => k.replace("node-role.kubernetes.io/", "")) || ["<none>"]).join(",");
        base.version = status.nodeInfo.kubeletVersion || "";
        base.os = `${status.nodeInfo.operatingSystem}/${status.nodeInfo.architecture}`;
        base.schedulable = !spec.unschedulable;
      }

      if (status.succeeded !== undefined || status.failed !== undefined) {
        base.status = status.succeeded ? "Complete" : status.active ? "Running" : status.failed ? "Failed" : "Pending";
        base.completions = `${status.succeeded || 0}/${spec.completions || 1}`;
      }

      if (spec.schedule) {
        base.schedule = spec.schedule;
        base.lastSchedule = status.lastScheduleTime || "Never";
        base.status = status.active?.length ? "Active" : "Idle";
      }

      if (spec.rules) {
        base.hosts = (spec.rules || []).map((r: any) => r.host || "*").join(", ");
        base.paths = (spec.rules || []).flatMap((r: any) =>
          (r.http?.paths || []).map((p: any) => p.path || "/")
        ).join(", ");
        const lbIngress = status.loadBalancer?.ingress || [];
        base.address = lbIngress.map((i: any) => i.ip || i.hostname || "").join(",") || "<pending>";
      }

      if (item.data !== undefined && !spec.type && !spec.replicas) {
        base.dataCount = String(Object.keys(item.data || {}).length);
      }
      if (item.type && !spec.type) {
        base.secretType = item.type;
      }

      if (spec.capacity) {
        base.capacity = spec.capacity?.storage || "";
        base.accessModes = (spec.accessModes || []).join(",");
        base.reclaimPolicy = spec.persistentVolumeReclaimPolicy || "";
        base.status = status.phase || "";
        base.storageClass = spec.storageClassName || "";
      }

      if (spec.accessModes && !spec.capacity) {
        base.status = status.phase || "";
        base.volume = spec.volumeName || "";
        base.capacity = status.capacity?.storage || spec.resources?.requests?.storage || "";
        base.accessModes = (spec.accessModes || []).join(",");
        base.storageClass = spec.storageClassName || "";
      }

      if (!meta.namespace && status.phase && !status.nodeInfo && !statuses.length && !spec.type) {
        base.status = status.phase || "";
      }

      if (item.reason) {
        base.type = item.type || "";
        base.reason = item.reason || "";
        base.message = item.message || "";
        base.count = String(item.count || 1);
        base.source = item.source?.component || "";
        base.object = item.involvedObject ? `${item.involvedObject.kind}/${item.involvedObject.name}` : "";
      }

      // Phase 10: Built-in Heuristic Linter
      const warnings: string[] = [];
      let targetContainers = spec.containers || [];
      let targetSecurity = spec.securityContext || {};
      
      if (!targetContainers.length && spec.template?.spec?.containers) {
        targetContainers = spec.template.spec.containers;
        targetSecurity = spec.template.spec.securityContext || {};
      }

      if (targetContainers.length > 0) {
        for (const c of targetContainers) {
          if (!c.image || c.image.endsWith(":latest") || (!c.image.includes(":") && !c.image.includes("@"))) {
            warnings.push(`Container '${c.name || "unknown"}' uses 'latest' image tag.`);
          }
          if (!c.resources?.requests || !c.resources?.limits) {
            warnings.push(`Container '${c.name || "unknown"}' lacks resource limits/requests.`);
          }
          if (c.securityContext?.privileged) {
            warnings.push(`Container '${c.name || "unknown"}' runs in privileged mode.`);
          }
          if (c.securityContext?.runAsUser === 0 || targetSecurity.runAsUser === 0) {
            warnings.push(`Container '${c.name || "unknown"}' runs as root user.`);
          }
        }
      }
      
      if (warnings.length > 0) {
        base.warnings = warnings;
      }

      return base;
    });
  } catch { return []; }
}

export function timeAgo(ts: string): string {
  if (!ts) return "";
  const diff = Date.now() - new Date(ts).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "<1m";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

export function statusColor(status: string): string {
  if (!status) return "var(--text-muted)";
  const s = status.toLowerCase();
  if (["running", "active", "available", "ready", "complete", "bound", "succeeded"].some(x => s.includes(x))) return "var(--accent-green)";
  if (["pending", "containercreating", "idle", "waiting"].some(x => s.includes(x))) return "var(--accent-yellow)";
  if (["failed", "error", "crashloopbackoff", "notready", "terminated", "evicted"].some(x => s.includes(x))) return "var(--accent-red)";
  return "var(--text-secondary)";
}

export function getColumns(resourceId: string): { key: string; label: string; mono?: boolean; color?: (v: any, row: K8sResource) => string }[] {
  switch (resourceId) {
    case "pods": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "ready", label: "Ready", mono: true },
      { key: "restarts", label: "Restarts", mono: true, color: (v) => parseInt(v) > 0 ? "var(--accent-yellow)" : "var(--text-muted)" },
      { key: "node", label: "Node" },
      { key: "age", label: "Age" },
    ];
    case "deployments": case "statefulsets": case "daemonsets": case "replicasets": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "replicas", label: "Ready", mono: true, color: (v) => v?.startsWith("0/") ? "var(--accent-red)" : "var(--accent-green)" },
      { key: "available", label: "Available", mono: true },
      { key: "age", label: "Age" },
    ];
    case "services": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "svcType", label: "Type" },
      { key: "clusterIP", label: "Cluster IP", mono: true },
      { key: "ports", label: "Ports", mono: true },
      { key: "age", label: "Age" },
    ];
    case "ingresses": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "hosts", label: "Hosts" },
      { key: "paths", label: "Paths", mono: true },
      { key: "address", label: "Address", mono: true },
      { key: "age", label: "Age" },
    ];
    case "configmaps": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "dataCount", label: "Data", mono: true },
      { key: "age", label: "Age" },
    ];
    case "secrets": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "secretType", label: "Type" },
      { key: "dataCount", label: "Data", mono: true },
      { key: "age", label: "Age" },
    ];
    case "jobs": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "completions", label: "Completions", mono: true },
      { key: "age", label: "Age" },
    ];
    case "cronjobs": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "schedule", label: "Schedule", mono: true },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "lastSchedule", label: "Last Schedule" },
      { key: "age", label: "Age" },
    ];
    case "nodes": return [
      { key: "name", label: "Name", mono: true },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "roles", label: "Roles" },
      { key: "version", label: "Version", mono: true },
      { key: "os", label: "OS/Arch" },
      { key: "age", label: "Age" },
    ];
    case "pv": return [
      { key: "name", label: "Name", mono: true },
      { key: "capacity", label: "Capacity", mono: true },
      { key: "accessModes", label: "Access Modes" },
      { key: "reclaimPolicy", label: "Reclaim" },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "storageClass", label: "Class" },
      { key: "age", label: "Age" },
    ];
    case "pvc": return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "volume", label: "Volume", mono: true },
      { key: "capacity", label: "Capacity", mono: true },
      { key: "storageClass", label: "Class" },
      { key: "age", label: "Age" },
    ];
    case "events": return [
      { key: "type", label: "Type", color: (v) => v === "Warning" ? "var(--accent-yellow)" : "var(--text-muted)" },
      { key: "reason", label: "Reason" },
      { key: "object", label: "Object", mono: true },
      { key: "message", label: "Message" },
      { key: "count", label: "#", mono: true },
      { key: "age", label: "Age" },
    ];
    case "namespaces": return [
      { key: "name", label: "Name", mono: true },
      { key: "status", label: "Status", color: (v) => statusColor(v) },
      { key: "age", label: "Age" },
    ];
    default: return [
      { key: "name", label: "Name", mono: true },
      { key: "namespace", label: "Namespace" },
      { key: "age", label: "Age" },
    ];
  }
}

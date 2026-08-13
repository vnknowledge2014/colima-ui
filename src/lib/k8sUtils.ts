import type { K8sResource } from "../store/k8s.svelte";

type JsonRecord = Record<string, unknown>;

const asRecord = (v: unknown): JsonRecord => (v && typeof v === "object" && !Array.isArray(v) ? (v as JsonRecord) : {});
const asArray = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
const asString = (v: unknown, fallback = ""): string => (typeof v === "string" ? v : String(v ?? fallback));

export function parseItems(raw: unknown): K8sResource[] {
  if (!raw) return [];
  try {
    const parsed: JsonRecord = typeof raw === "string" ? (JSON.parse(raw) as JsonRecord) : asRecord(raw);
    const items: JsonRecord[] = asArray(parsed.items).length
      ? (asArray(parsed.items) as JsonRecord[])
      : Array.isArray(raw) ? (raw as JsonRecord[]) : [];
    return items.map((item) => {
      const meta = asRecord(item.metadata);
      const spec = asRecord(item.spec);
      const status = asRecord(item.status);
      const statuses = asArray(status.containerStatuses) as JsonRecord[];

      const base: K8sResource = {
        name: asString(meta.name),
        namespace: asString(meta.namespace),
        age: asString(meta.creationTimestamp),
        _raw: item,
      };

      if (statuses.length > 0 || status.phase) {
        const ready = statuses.filter((s) => s.ready).length;
        base.status = asString(status.phase, "Unknown");
        base.ready = `${ready}/${statuses.length || asArray(spec.containers).length || 0}`;
        base.restarts = String(statuses.reduce((s: number, c: JsonRecord) => s + (typeof c.restartCount === "number" ? c.restartCount : 0), 0));
        base.node = asString(spec.nodeName);
      }

      if (spec.replicas !== undefined) {
        base.replicas = `${status.readyReplicas || 0}/${spec.replicas}`;
        base.available = String(status.availableReplicas || status.readyReplicas || 0);
      }

      if (spec.type) {
        base.svcType = asString(spec.type);
        base.clusterIP = asString(spec.clusterIP, "None");
        base.ports = (asArray(spec.ports) as JsonRecord[]).map((p) => `${p.port}/${p.protocol}`).join(", ");
        base._ports = asArray(spec.ports) as JsonRecord[];
      }

      if (status.nodeInfo) {
        const conds = (asArray(status.conditions) as JsonRecord[]).find((c) => c.type === "Ready");
        base.status = conds?.status === "True" ? "Ready" : "NotReady";
        base.roles = (Object.keys(asRecord(meta.labels))
          .filter((k: string) => k.startsWith("node-role.kubernetes.io/"))
          .map((k: string) => k.replace("node-role.kubernetes.io/", "")) || ["<none>"]).join(",");
        base.version = asString(asRecord(status.nodeInfo).kubeletVersion);
        base.os = `${asString(asRecord(status.nodeInfo).operatingSystem)}/${asString(asRecord(status.nodeInfo).architecture)}`;
        base.schedulable = !spec.unschedulable;
      }

      if (status.succeeded !== undefined || status.failed !== undefined) {
        base.status = status.succeeded ? "Complete" : status.active ? "Running" : status.failed ? "Failed" : "Pending";
        base.completions = `${status.succeeded || 0}/${spec.completions || 1}`;
      }

      if (spec.schedule) {
        base.schedule = asString(spec.schedule);
        base.lastSchedule = asString(status.lastScheduleTime, "Never");
        base.status = asArray(status.active).length ? "Active" : "Idle";
      }

      if (spec.rules) {
        base.hosts = (asArray(spec.rules) as JsonRecord[]).map((r) => asString(r.host, "*")).join(", ");
        base.paths = (asArray(spec.rules) as JsonRecord[]).flatMap((r) =>
          asArray(asRecord(r.http).paths).map((p) => asString(asRecord(p).path, "/"))
        ).join(", ");
        const lbIngress = asArray(asRecord(status.loadBalancer).ingress) as JsonRecord[];
        base.address = lbIngress.map((i) => asString(i.ip) || asString(i.hostname) || "").join(",") || "<pending>";
      }

      if (item.data !== undefined && !spec.type && !spec.replicas) {
        base.dataCount = String(Object.keys(asRecord(item.data)).length);
      }
      if (item.type && !spec.type) {
        base.secretType = asString(item.type);
      }

      if (spec.capacity) {
        base.capacity = asString(asRecord(spec.capacity).storage);
        base.accessModes = (asArray(spec.accessModes) as string[]).join(",");
        base.reclaimPolicy = asString(spec.persistentVolumeReclaimPolicy);
        base.status = asString(status.phase);
        base.storageClass = asString(spec.storageClassName);
      }

      if (spec.accessModes && !spec.capacity) {
        base.status = asString(status.phase);
        base.volume = asString(spec.volumeName);
        base.capacity = asString(asRecord(status.capacity).storage) || asString(asRecord(asRecord(spec.resources).requests).storage);
        base.accessModes = (asArray(spec.accessModes) as string[]).join(",");
        base.storageClass = asString(spec.storageClassName);
      }

      if (!meta.namespace && status.phase && !status.nodeInfo && !statuses.length && !spec.type) {
        base.status = asString(status.phase);
      }

      if (item.reason) {
        base.type = asString(item.type);
        base.reason = asString(item.reason);
        base.message = asString(item.message);
        base.count = String(item.count || 1);
        base.source = asString(asRecord(item.source).component);
        base.object = item.involvedObject ? `${asRecord(item.involvedObject).kind}/${asRecord(item.involvedObject).name}` : "";
      }

      // Phase 10: Built-in Heuristic Linter
      const warnings: string[] = [];
      let targetContainers = asArray(spec.containers) as JsonRecord[];
      let targetSecurity = asRecord(spec.securityContext);

      if (!targetContainers.length) {
        const tplSpec = asRecord(asRecord(spec.template).spec);
        targetContainers = asArray(tplSpec.containers) as JsonRecord[];
        targetSecurity = asRecord(tplSpec.securityContext);
      }

      if (targetContainers.length > 0) {
        for (const c of targetContainers) {
          const image = asString(c.image);
          if (!image || image.endsWith(":latest") || (!image.includes(":") && !image.includes("@"))) {
            warnings.push(`Container '${asString(c.name, "unknown")}' uses 'latest' image tag.`);
          }
          if (!asRecord(c.resources).requests || !asRecord(c.resources).limits) {
            warnings.push(`Container '${asString(c.name, "unknown")}' lacks resource limits/requests.`);
          }
          if (asRecord(c.securityContext).privileged) {
            warnings.push(`Container '${asString(c.name, "unknown")}' runs in privileged mode.`);
          }
          if (asRecord(c.securityContext).runAsUser === 0 || targetSecurity.runAsUser === 0) {
            warnings.push(`Container '${asString(c.name, "unknown")}' runs as root user.`);
          }
        }
      }

      if (warnings.length > 0) {
        base.warnings = warnings;
      }

      return base;
    });
  } catch {
    return [];
  }
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

export function getColumns(resourceId: string): { key: string; label: string; mono?: boolean; color?: (v: string, row: K8sResource) => string }[] {
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

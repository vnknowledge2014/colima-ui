import React, { useState, useEffect, useCallback, useRef } from "react";
import { k8sApi } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { StatusDot, CloseIcon, RestartIcon, TrashIcon } from "../components/Icons";
import { useAtom } from "jotai";
import {
  k8sConnectedAtom, k8sLoadingAtom, k8sDataLoadingAtom,
  k8sNamespacesAtom, k8sItemsAtom, k8sActiveResourceAtom,
  k8sNamespaceAtom, k8sContextsAtom, k8sCurrentCtxAtom,
  K8sResource,
} from "../store/k8sAtom";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import { useHotkeys } from "../hooks/useHotkeys";



// ===== Resource Groups =====
const RESOURCE_GROUPS = [
  {
    label: "Workloads",
    items: [
      { id: "pods", label: "Pods", resource: "pods" },
      { id: "deployments", label: "Deployments", resource: "deployments", canRestart: true },
      { id: "statefulsets", label: "StatefulSets", resource: "statefulsets", canRestart: true },
      { id: "daemonsets", label: "DaemonSets", resource: "daemonsets", canRestart: true },
      { id: "replicasets", label: "ReplicaSets", resource: "replicasets" },
      { id: "jobs", label: "Jobs", resource: "jobs" },
      { id: "cronjobs", label: "CronJobs", resource: "cronjobs" },
    ],
  },
  {
    label: "Networking",
    items: [
      { id: "services", label: "Services", resource: "services" },
      { id: "ingresses", label: "Ingresses", resource: "ingresses" },
    ],
  },
  {
    label: "Config",
    items: [
      { id: "configmaps", label: "ConfigMaps", resource: "configmaps" },
      { id: "secrets", label: "Secrets", resource: "secrets" },
    ],
  },
  {
    label: "Storage",
    items: [
      { id: "pv", label: "PV", resource: "persistentvolumes" },
      { id: "pvc", label: "PVC", resource: "persistentvolumeclaims" },
    ],
  },
  {
    label: "Cluster",
    items: [
      { id: "nodes", label: "Nodes", resource: "nodes" },
      { id: "events", label: "Events", resource: "events" },
      { id: "namespaces", label: "Namespaces", resource: "namespaces" },
      { id: "health", label: "Health", resource: "" },
    ],
  },
];

const ALL_ITEMS = RESOURCE_GROUPS.flatMap(g => g.items);

// ===== Parsers =====
function parseItems(raw: any): K8sResource[] {
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

      // Pod-specific
      if (statuses.length > 0 || status.phase) {
        const ready = statuses.filter((s: any) => s.ready).length;
        base.status = status.phase || "Unknown";
        base.ready = `${ready}/${statuses.length || spec.containers?.length || 0}`;
        base.restarts = String(statuses.reduce((s: number, c: any) => s + (c.restartCount || 0), 0));
        base.node = spec.nodeName || "";
      }

      // Deployment/StatefulSet/DaemonSet/ReplicaSet
      if (spec.replicas !== undefined) {
        base.replicas = `${status.readyReplicas || 0}/${spec.replicas}`;
        base.available = String(status.availableReplicas || status.readyReplicas || 0);
      }

      // Service
      if (spec.type) {
        base.svcType = spec.type;
        base.clusterIP = spec.clusterIP || "None";
        base.ports = (spec.ports || []).map((p: any) => `${p.port}/${p.protocol}`).join(", ");
        base._ports = spec.ports || [];
      }

      // Node
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

      // Job
      if (status.succeeded !== undefined || status.failed !== undefined) {
        base.status = status.succeeded ? "Complete" : status.active ? "Running" : status.failed ? "Failed" : "Pending";
        base.completions = `${status.succeeded || 0}/${spec.completions || 1}`;
      }

      // CronJob
      if (spec.schedule) {
        base.schedule = spec.schedule;
        base.lastSchedule = status.lastScheduleTime || "Never";
        base.status = status.active?.length ? "Active" : "Idle";
      }

      // Ingress
      if (spec.rules) {
        base.hosts = (spec.rules || []).map((r: any) => r.host || "*").join(", ");
        base.paths = (spec.rules || []).flatMap((r: any) =>
          (r.http?.paths || []).map((p: any) => p.path || "/")
        ).join(", ");
        const lbIngress = status.loadBalancer?.ingress || [];
        base.address = lbIngress.map((i: any) => i.ip || i.hostname || "").join(",") || "<pending>";
      }

      // ConfigMap / Secret
      if (item.data !== undefined && !spec.type && !spec.replicas) {
        base.dataCount = String(Object.keys(item.data || {}).length);
      }
      if (item.type && !spec.type) {
        base.secretType = item.type;
      }

      // PV
      if (spec.capacity) {
        base.capacity = spec.capacity?.storage || "";
        base.accessModes = (spec.accessModes || []).join(",");
        base.reclaimPolicy = spec.persistentVolumeReclaimPolicy || "";
        base.status = status.phase || "";
        base.storageClass = spec.storageClassName || "";
      }

      // PVC
      if (spec.accessModes && !spec.capacity) {
        base.status = status.phase || "";
        base.volume = spec.volumeName || "";
        base.capacity = status.capacity?.storage || spec.resources?.requests?.storage || "";
        base.accessModes = (spec.accessModes || []).join(",");
        base.storageClass = spec.storageClassName || "";
      }

      // Namespace
      if (!meta.namespace && status.phase && !status.nodeInfo && !statuses.length && !spec.type) {
        base.status = status.phase || "";
      }

      // Event
      if (item.reason) {
        base.type = item.type || "";
        base.reason = item.reason || "";
        base.message = item.message || "";
        base.count = String(item.count || 1);
        base.source = item.source?.component || "";
        base.object = item.involvedObject ? `${item.involvedObject.kind}/${item.involvedObject.name}` : "";
      }

      return base;
    });
  } catch { return []; }
}

function timeAgo(ts: string): string {
  if (!ts) return "";
  const diff = Date.now() - new Date(ts).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "<1m";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

function statusColor(status: string): string {
  if (!status) return "var(--text-muted)";
  const s = status.toLowerCase();
  if (["running", "active", "available", "ready", "complete", "bound", "succeeded"].some(x => s.includes(x))) return "var(--accent-green)";
  if (["pending", "containercreating", "idle", "waiting"].some(x => s.includes(x))) return "var(--accent-yellow)";
  if (["failed", "error", "crashloopbackoff", "notready", "terminated", "evicted"].some(x => s.includes(x))) return "var(--accent-red)";
  return "var(--text-secondary)";
}

// ===== Column definitions per resource =====
function getColumns(resourceId: string): { key: string; label: string; mono?: boolean; color?: (v: any, row: K8sResource) => string }[] {
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

// Inline icon SVGs
const TerminalSvg = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
  </svg>
);
const PortForwardSvg = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M5 12h14M12 5l7 7-7 7"/>
  </svg>
);
const SaveSvg = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/>
    <polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/>
  </svg>
);

/* ===== Custom Select (cross-platform, div-based) ===== */
function CustomSelect({ value, options, onChange, style }: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
  style?: React.CSSProperties;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = options.find(o => o.value === value);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div ref={ref} style={{ position: "relative", ...style }}>
      <button
        onClick={() => setOpen(!open)}
        style={{
          display: "flex", alignItems: "center", gap: 6, width: "100%",
          background: "var(--bg-secondary)", border: "1px solid var(--border-primary)",
          borderRadius: 6, padding: "4px 8px", color: "var(--text-primary)",
          fontSize: "var(--text-sm)", fontFamily: "var(--font-mono)",
          cursor: "pointer", whiteSpace: "nowrap",
        }}
      >
        {value === "all" && (
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="2">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        )}
        {selected?.label || value}
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
          style={{ marginLeft: "auto", transform: open ? "rotate(180deg)" : "none", transition: "transform 150ms" }}>
          <polyline points="6 9 12 15 18 9"/>
        </svg>
      </button>
      {open && (
        <div style={{
          position: "absolute", top: "calc(100% + 4px)", right: 0, minWidth: "100%",
          background: "var(--bg-card)", border: "1px solid var(--border-primary)",
          borderRadius: 8, boxShadow: "0 8px 32px rgba(0,0,0,0.5)", zIndex: 9999,
          maxHeight: 280, overflowY: "auto", padding: 4,
        }}>
          {options.map(opt => (
            <button
              key={opt.value}
              onClick={() => { onChange(opt.value); setOpen(false); }}
              style={{
                display: "flex", alignItems: "center", gap: 6, width: "100%",
                padding: "6px 10px", border: "none", borderRadius: 4, cursor: "pointer",
                background: value === opt.value ? "rgba(88,166,255,0.12)" : "transparent",
                color: value === opt.value ? "var(--accent-blue)" : "var(--text-primary)",
                fontSize: "var(--text-sm)", fontFamily: "var(--font-mono)",
                fontWeight: value === opt.value ? 600 : 400,
                textAlign: "left", whiteSpace: "nowrap",
              }}
              onMouseEnter={e => (e.currentTarget.style.background = value === opt.value ? "rgba(88,166,255,0.18)" : "rgba(255,255,255,0.05)")}
              onMouseLeave={e => (e.currentTarget.style.background = value === opt.value ? "rgba(88,166,255,0.12)" : "transparent")}
            >
              {value === opt.value && (
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                  <polyline points="20 6 9 17 4 12"/>
                </svg>
              )}
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ===== Component =====
export default function Kubernetes() {
  const [connected, setConnected] = useAtom(k8sConnectedAtom);
  const [activeResource, setActiveResource] = useAtom(k8sActiveResourceAtom);
  const [namespace, setNamespace] = useAtom(k8sNamespaceAtom);
  const [namespaces, setNamespaces] = useAtom(k8sNamespacesAtom);
  const [items, setItems] = useAtom(k8sItemsAtom);
  const [loading, setLoading] = useAtom(k8sLoadingAtom);
  const [dataLoading, setDataLoading] = useAtom(k8sDataLoadingAtom);
  const [contexts, setContexts] = useAtom(k8sContextsAtom);
  const [currentCtx, setCurrentCtx] = useAtom(k8sCurrentCtxAtom);
  const [selectedItem, setSelectedItem] = useState<K8sResource | null>(null);
  const [detailTab, setDetailTab] = useState<"describe" | "yaml" | "logs">("describe");
  const [detailText, setDetailText] = useState("");
  const [yamlText, setYamlText] = useState("");
  const [yamlEdited, setYamlEdited] = useState("");
  const [logsText, setLogsText] = useState("");
  const [filter, setFilter] = useState("");
  const [followLogs, setFollowLogs] = useState(false);
  const eventSourceRef = useRef<EventSource | null>(null);
  const logViewerRef = useRef<HTMLDivElement>(null);
  const [containers, setContainers] = useState<string[]>([]);
  const [selectedContainer, setSelectedContainer] = useState("");
  const [portForwardModal, setPortForwardModal] = useState<K8sResource | null>(null);
  const [pfLocalPort, setPfLocalPort] = useState("");
  const [pfRemotePort, setPfRemotePort] = useState("");
  const [activeForwards, setActiveForwards] = useState<string[]>([]);
  const [applying, setApplying] = useState(false);
  const [scaleValue, setScaleValue] = useState<number | null>(null);
  const [healthData, setHealthData] = useState<any>(null);
  const [healthLoading, setHealthLoading] = useState(false);
  const [kubectlMissing, setKubectlMissing] = useState(false);
  const [crdTypes, setCrdTypes] = useState<{ id: string; label: string; resource: string; group: string }[]>([]);
  const [benchModal, setBenchModal] = useState<K8sResource | null>(null);
  const [benchUrl, setBenchUrl] = useState("");
  const [benchConc, setBenchConc] = useState(5);
  const [benchReqs, setBenchReqs] = useState(50);
  const [benchMethod, setBenchMethod] = useState("GET");
  const [benchRunning, setBenchRunning] = useState(false);
  const [benchResult, setBenchResult] = useState<any>(null);
  const { confirm, ConfirmDialogProps } = useConfirm();
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; item: K8sResource } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // Hotkeys
  useHotkeys({
    "mod+k": () => searchRef.current?.focus(),
    "escape": () => { setSelectedItem(null); setCtxMenu(null); setPortForwardModal(null); },
  });

  const openCtxMenu = (e: React.MouseEvent, item: K8sResource) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY, item });
  };

  const getCtxItems = (item: K8sResource): ContextMenuItem[] => {
    const activeInfo = ALL_ITEMS.find(i => i.id === activeResource);
    const result: ContextMenuItem[] = [
      { label: "View Details", action: () => openDetail(item) },
    ];
    if (activeResource === "pods") {
      result.push({ label: "View Logs", action: () => { openDetail(item); setDetailTab("logs"); } });
      result.push({ label: "Exec Shell", action: () => handleExec(item) });
    }
    if (activeInfo?.canRestart) {
      result.push({ label: "Restart", action: () => handleRestart(item) });
    }
    if (activeResource === "services") {
      result.push({ label: "⚡ Benchmark", action: () => {
        setBenchModal(item);
        const port = item.port?.split("/")[0]?.split(",")[0] || "80";
        setBenchUrl(`http://localhost:${port}`);
        setBenchConc(5); setBenchReqs(50); setBenchMethod("GET"); setBenchResult(null);
      }});
    }
    result.push({ divider: true, label: "", action: () => {} });
    result.push({ label: "Copy Name", action: () => { navigator.clipboard.writeText(item.name); globalToast("success", "Name copied"); } });
    result.push({ divider: true, label: "", action: () => {} });
    result.push({ label: "Delete", danger: true, action: () => handleDelete(item) });
    return result;
  };

  // Check cluster
  const checkCluster = useCallback(async () => {
    // Always fetch contexts so dropdown works even when disconnected
    try {
      const ctxRaw = await k8sApi.contexts();
      const ctxList = ctxRaw.trim().split("\n").filter(Boolean);
      if (ctxList.length > 0) setContexts(ctxList);
      const cur = await k8sApi.currentContext();
      setCurrentCtx(cur.trim());
      setKubectlMissing(false);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("not installed")) {
        setKubectlMissing(true);
        setConnected(false);
        setLoading(false);
        return;
      }
    }

    try {
      await k8sApi.check();
      setConnected(true);
      const nsRaw = await k8sApi.namespaces();
      // Handle both Tauri IPC format (array of {name, status, age}) and HTTP format (kubectl JSON with items[])
      let nsList: { name: string }[] = [];
      if (Array.isArray(nsRaw)) {
        // Tauri IPC returns Vec<K8sNamespace> with camelCase fields
        nsList = nsRaw.map((ns: any) => ({ name: ns.name || "" })).filter(ns => ns.name);
      } else {
        const parsed = parseItems(nsRaw);
        nsList = parsed.map(n => ({ name: n.name })).filter(ns => ns.name);
      }
      setNamespaces(nsList);
      try {
        const fwds = await k8sApi.portForwardList();
        setActiveForwards(fwds.split("\n").filter(Boolean));
      } catch { /* not critical */ }
      // Discover CRDs
      try {
        const crdRaw = await k8sApi.crds();
        const parsed = typeof crdRaw === "string" ? JSON.parse(crdRaw) : crdRaw;
        const items = parsed.items || [];
        const crds = items.map((crd: any) => {
          const name = crd.metadata?.name || "";
          const kind = crd.spec?.names?.kind || name;
          const group = crd.spec?.group || "";
          return { id: `crd:${name}`, label: kind, resource: name, group };
        }).slice(0, 30); // Limit to 30 CRDs to avoid sidebar overload
        setCrdTypes(crds);
      } catch { /* CRDs not available */ }
    } catch {
      setConnected(false);
    }
    setLoading(false);
  }, []);

  // Fetch data for active resource
  const fetchData = useCallback(async () => {
    if (!connected) return;
    setDataLoading(true);
    try {
      // Handle CRD resources
      if (activeResource.startsWith("crd:")) {
        const crdName = activeResource.slice(4);
        const raw = await k8sApi.crdResources(crdName, namespace);
        setItems(parseItems(raw));
        setDataLoading(false);
        return;
      }
      const info = ALL_ITEMS.find(i => i.id === activeResource);
      if (!info || activeResource === "health") { setDataLoading(false); return; }
      let raw: string;
      if (activeResource === "nodes") raw = await k8sApi.nodesJson();
      else if (activeResource === "events") raw = await k8sApi.eventsJson(namespace);
      else {
        // Always use generic resources endpoint — returns raw kubectl JSON
        // which parseItems() can parse consistently (Tauri IPC + HTTP)
        raw = await k8sApi.resources(info.resource, namespace);
      }
      setItems(parseItems(raw));
    } catch (e) {
      globalToast("error", `Failed to load ${activeResource}: ${e}`);
      setItems([]);
    } finally {
      setDataLoading(false);
    }
  }, [connected, activeResource, namespace]);

  useEffect(() => { checkCluster(); }, [checkCluster]);
  useEffect(() => { fetchData(); }, [fetchData]);

  // Open detail modal
  const openDetail = async (item: K8sResource) => {
    setSelectedItem(item);
    setDetailTab(activeResource === "pods" ? "logs" : "describe");
    setContainers([]);
    setSelectedContainer("");
    try {
      const info = ALL_ITEMS.find(i => i.id === activeResource);
      const rt = info?.resource || activeResource;
      // Remove trailing 's' for singular resource type
      const singularRt = rt.endsWith("ses") ? rt.slice(0, -2) : rt.endsWith("s") ? rt.slice(0, -1) : rt;
      const [desc, yaml] = await Promise.all([
        k8sApi.describe(item.namespace || "default", singularRt, item.name),
        k8sApi.yaml(singularRt, item.namespace || "default", item.name),
      ]);
      setDetailText(desc);
      setYamlText(yaml);
      setYamlEdited(yaml);
      if (activeResource === "pods") {
        const [logs, cont] = await Promise.all([
          k8sApi.podLogs(item.namespace, item.name, 200),
          k8sApi.podContainers(item.namespace, item.name).catch(() => ""),
        ]);
        setLogsText(logs);
        const containerList = cont.trim().split(/\s+/).filter(Boolean);
        setContainers(containerList);
        if (containerList.length > 0) setSelectedContainer(containerList[0]);
      } else {
        setLogsText("");
      }
    } catch (e) {
      setDetailText(`Error: ${e}`);
      setYamlText("");
      setYamlEdited("");
      setLogsText("");
    }
  };

  // Fetch container-specific logs
  const fetchContainerLogs = async (container: string) => {
    if (!selectedItem) return;
    setSelectedContainer(container);
    try {
      const logs = await k8sApi.containerLogs(selectedItem.namespace, selectedItem.name, container, 200);
      setLogsText(logs);
    } catch (e) {
      setLogsText(`Error: ${e}`);
    }
  };

  // Apply YAML
  const handleApply = async () => {
    if (!selectedItem) return;
    setApplying(true);
    try {
      const result = await k8sApi.apply(yamlEdited, selectedItem.namespace);
      globalToast("success", result || "Applied successfully");
      setYamlText(yamlEdited);
      setTimeout(fetchData, 1000);
    } catch (e) {
      globalToast("error", `Apply failed: ${e}`);
    } finally {
      setApplying(false);
    }
  };

  // Delete resource
  const handleDelete = async (item: K8sResource) => {
    const info = ALL_ITEMS.find(i => i.id === activeResource);
    const rt = info?.resource || activeResource;
    const ok = await confirm({
      title: `Delete ${rt.replace(/s$/, "")}`,
      message: `Delete ${item.name} from ${item.namespace || "cluster"}?`,
      confirmText: "Delete",
      variant: "danger",
    });
    if (!ok) return;
    try {
      if (activeResource === "pods") await k8sApi.deletePod(item.namespace, item.name);
      else await k8sApi.deleteResource(rt.replace(/s$/, ""), item.namespace || "default", item.name);
      globalToast("success", `${item.name} deleted`);
      fetchData();
    } catch (e) { globalToast("error", String(e)); }
  };

  // Restart resource
  const handleRestart = async (item: K8sResource) => {
    const info = ALL_ITEMS.find(i => i.id === activeResource);
    if (!info?.canRestart) return;
    const ok = await confirm({
      title: `Restart ${info.label.replace(/s$/, "")}`,
      message: `Rollout restart ${item.name}?`,
      confirmText: "Restart",
      variant: "warning",
    });
    if (!ok) return;
    try {
      await k8sApi.restart(info.resource.replace(/s$/, ""), item.namespace, item.name);
      globalToast("success", `${item.name} restarting`);
      setTimeout(fetchData, 2000);
    } catch (e) { globalToast("error", String(e)); }
  };

  // Exec into pod
  const handleExec = async (item: K8sResource) => {
    try {
      const result = await k8sApi.exec(item.namespace, item.name, selectedContainer || "");
      globalToast("success", result || "Shell opened in Terminal");
    } catch (e) { globalToast("error", `Exec failed: ${e}`); }
  };

  // Port forward
  const startPortForward = async () => {
    if (!portForwardModal || !pfLocalPort || !pfRemotePort) return;
    const resourceType = activeResource === "services" ? "service" : "pod";
    try {
      const result = await k8sApi.portForwardStart(
        portForwardModal.namespace, portForwardModal.name,
        parseInt(pfLocalPort), parseInt(pfRemotePort), resourceType
      );
      globalToast("success", result);
      setPortForwardModal(null);
      const fwds = await k8sApi.portForwardList();
      setActiveForwards(fwds.split("\n").filter(Boolean));
    } catch (e) { globalToast("error", String(e)); }
  };

  const stopPortForward = async (port: string) => {
    try {
      await k8sApi.portForwardStop(parseInt(port));
      globalToast("success", `Port forward on ${port} stopped`);
      const fwds = await k8sApi.portForwardList();
      setActiveForwards(fwds.split("\n").filter(Boolean));
    } catch (e) { globalToast("error", String(e)); }
  };

  // Scale resource
  const handleScale = async (item: K8sResource, replicas: number) => {
    const info = ALL_ITEMS.find(i => i.id === activeResource);
    if (!info) return;
    try {
      await k8sApi.genericScale(info.resource.replace(/s$/, ""), item.namespace, item.name, replicas);
      globalToast("success", `Scaled ${item.name} to ${replicas} replica(s)`);
      setScaleValue(replicas);
      setTimeout(fetchData, 1500);
    } catch (e) { globalToast("error", String(e)); }
  };

  // Cluster health scan
  const runHealthScan = async () => {
    setHealthLoading(true);
    try {
      const raw = await k8sApi.clusterHealth();
      setHealthData(JSON.parse(raw));
    } catch (e) { globalToast("error", `Health scan failed: ${e}`); }
    finally { setHealthLoading(false); }
  };

  // Node actions
  const handleNodeAction = async (item: K8sResource, action: string) => {
    const labels: Record<string, string> = { cordon: "Cordon", uncordon: "Uncordon", drain: "Drain" };
    const ok = await confirm({
      title: `${labels[action]} Node`,
      message: `${labels[action]} node ${item.name}?${action === "drain" ? " This will evict all pods." : ""}`,
      confirmText: labels[action],
      variant: action === "drain" ? "danger" : "warning",
    });
    if (!ok) return;
    try {
      await k8sApi.nodeAction(item.name, action);
      globalToast("success", `Node ${item.name} ${action}ed`);
      setTimeout(fetchData, 1000);
    } catch (e) { globalToast("error", String(e)); }
  };

  // Context switch
  const handleContextSwitch = async (ctx: string) => {
    try {
      await k8sApi.setContext(ctx);
      setCurrentCtx(ctx);
      globalToast("success", `Switched to ${ctx}`);
      setLoading(true);
      setTimeout(checkCluster, 500);
    } catch (e) { globalToast("error", `Context switch failed: ${e}`); }
  };

  // Loading
  if (loading) {
    return (
      <>
        <div className="content-header"><h1>Kubernetes</h1></div>
        <div className="loading-screen"><div className="spinner" /><span>Connecting to cluster...</span></div>
      </>
    );
  }

  // Disconnected
  if (!connected) {
    return (
      <>
        <div className="content-header">
          <h1>Kubernetes</h1>
          {!kubectlMissing && contexts.length > 1 && (
            <div className="content-header-actions" style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <select value={currentCtx} onChange={e => handleContextSwitch(e.target.value)} style={{
                background: "var(--bg-secondary)", border: "1px solid var(--border-primary)",
                borderRadius: 6, padding: "4px 8px", color: "var(--accent-purple)",
                fontSize: "var(--text-xs)", fontFamily: "var(--font-mono)",
              }}>
                {contexts.map(c => <option key={c} value={c}>{c}</option>)}
              </select>
            </div>
          )}
        </div>
        <div className="content-body">
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke={kubectlMissing ? "var(--accent-yellow)" : "var(--accent-red)"} strokeWidth="1.5">
                {kubectlMissing ? (
                  <><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></>
                ) : (
                  <><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></>
                )}
              </svg>
            </div>
            <div className="empty-state-title">{kubectlMissing ? "kubectl Not Installed" : "Cluster Not Connected"}</div>
            <div className="empty-state-text">
              {kubectlMissing ? (
                <>
                  <code style={{ display: "block", marginBottom: 12, padding: "8px 12px", background: "var(--bg-primary)", borderRadius: 6, fontFamily: "var(--font-mono)", fontSize: "var(--text-sm)" }}>
                    brew install kubectl
                  </code>
                  Install kubectl to manage Kubernetes clusters.
                </>
              ) : (
                <>
                  {currentCtx && (
                    <span style={{ display: "block", marginBottom: 8, fontFamily: "var(--font-mono)", color: "var(--accent-purple)", fontSize: "var(--text-xs)" }}>
                      Context: {currentCtx}
                    </span>
                  )}
                  Enable Kubernetes in the <strong>Instances</strong> tab, or switch to a different context above.
                </>
              )}
            </div>
            <button className="btn btn-primary" onClick={() => { setLoading(true); checkCluster(); }}>Retry Connection</button>
          </div>
        </div>
      </>
    );
  }

  const activeInfo = ALL_ITEMS.find(i => i.id === activeResource);
  const columns = getColumns(activeResource);
  const filtered = filter
    ? items.filter(i => i.name.toLowerCase().includes(filter.toLowerCase()) || i.namespace?.toLowerCase().includes(filter.toLowerCase()))
    : items;

  return (
    <>
      <div className="content-header">
        <h1>
          Kubernetes
          <span style={{ fontSize: "var(--text-sm)", color: "var(--accent-green)", fontWeight: 400, marginLeft: 12 }}>
            <StatusDot size={8} color="var(--accent-green)" style={{ display: "inline-block", verticalAlign: "middle" }} /> Connected
          </span>
        </h1>
        <div className="content-header-actions" style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {/* Active port forwards indicator */}
          {activeForwards.length > 0 && (
            <div style={{
              display: "flex", alignItems: "center", gap: 4, padding: "3px 8px",
              background: "rgba(188,140,255,0.1)", border: "1px solid rgba(188,140,255,0.3)",
              borderRadius: 6, fontSize: "var(--text-xs)", color: "var(--accent-purple)",
            }}>
              <PortForwardSvg /> {activeForwards.length} forward{activeForwards.length > 1 ? "s" : ""}
            </div>
          )}
          {contexts.length > 1 && (
            <select value={currentCtx} onChange={e => handleContextSwitch(e.target.value)} style={{
              background: "var(--bg-secondary)", border: "1px solid var(--border-primary)",
              borderRadius: 6, padding: "4px 8px", color: "var(--accent-purple)",
              fontSize: "var(--text-xs)", fontFamily: "var(--font-mono)",
            }}>
              {contexts.map(c => <option key={c} value={c}>{c}</option>)}
            </select>
          )}
          <CustomSelect
            value={namespace}
            onChange={setNamespace}
            options={[
              { value: "all", label: "All Namespaces" },
              ...namespaces.map(ns => ({ value: ns.name, label: ns.name })),
            ]}
          />
          <button className="btn btn-ghost" onClick={fetchData}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
        </div>
      </div>

      <div className="content-body" style={{ display: "flex", gap: 0 }}>


        {/* Resource sidebar */}
        <div style={{
          width: 180, minWidth: 180, borderRight: "1px solid var(--border-primary)",
          paddingRight: 12, marginRight: 16, overflowY: "auto",
        }}>
          {RESOURCE_GROUPS.map(group => (
            <div key={group.label} style={{ marginBottom: 12 }}>
              <div style={{
                fontSize: "var(--text-xs)", color: "var(--text-muted)", fontWeight: 600,
                textTransform: "uppercase", letterSpacing: "0.05em", marginBottom: 4, padding: "0 8px",
              }}>{group.label}</div>
              {group.items.map(item => (
                <button key={item.id} onClick={() => { setActiveResource(item.id); setItems([]); setFilter(""); }}
                  style={{
                    display: "block", width: "100%", textAlign: "left", padding: "5px 8px",
                    background: activeResource === item.id ? "rgba(88,166,255,0.1)" : "transparent",
                    border: "none", borderRadius: 6, cursor: "pointer", fontSize: "var(--text-sm)",
                    color: activeResource === item.id ? "var(--accent-blue)" : "var(--text-secondary)",
                    fontWeight: activeResource === item.id ? 600 : 400, transition: "all 150ms",
                  }}>
                  {item.label}
                </button>
              ))}
            </div>
          ))}
          {/* Dynamic CRD group */}
          {crdTypes.length > 0 && (
            <div style={{ marginBottom: 12 }}>
              <div style={{
                fontSize: "var(--text-xs)", color: "var(--text-muted)", fontWeight: 600,
                textTransform: "uppercase", letterSpacing: "0.05em", marginBottom: 4, padding: "0 8px",
              }}>Custom Resources ({crdTypes.length})</div>
              {crdTypes.map(crd => (
                <button key={crd.id} onClick={() => { setActiveResource(crd.id); setItems([]); setFilter(""); }}
                  style={{
                    display: "block", width: "100%", textAlign: "left", padding: "5px 8px",
                    background: activeResource === crd.id ? "rgba(88,166,255,0.1)" : "transparent",
                    border: "none", borderRadius: 6, cursor: "pointer", fontSize: "var(--text-sm)",
                    color: activeResource === crd.id ? "var(--accent-blue)" : "var(--text-secondary)",
                    fontWeight: activeResource === crd.id ? 600 : 400, transition: "all 150ms",
                    overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  }}
                  title={`${crd.label} (${crd.group})`}
                >
                  {crd.label}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Main content */}
        <div style={{ flex: 1, minWidth: 0 }}>
          {/* Health Dashboard */}
          {activeResource === "health" ? (
            <div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
                <h2 style={{ fontSize: "var(--text-lg)", color: "var(--text-primary)", margin: 0 }}>Cluster Health</h2>
                <button className="btn btn-primary" onClick={runHealthScan} disabled={healthLoading}>
                  {healthLoading ? <><div className="spinner" style={{ width: 14, height: 14 }} /> Scanning...</> : "Run Health Scan"}
                </button>
              </div>
              {!healthData ? (
                <div className="empty-state">
                  <div className="empty-state-icon" style={{ fontSize: 48 }}>🩺</div>
                  <div className="empty-state-title">Cluster Health Analysis</div>
                  <div className="empty-state-text">
                    Analyze your cluster for misconfigurations, unhealthy resources, and potential issues.<br/>
                    Inspired by <strong>Popeye</strong> — a Kubernetes cluster sanitizer.
                  </div>
                  <button className="btn btn-primary" onClick={runHealthScan} disabled={healthLoading}>Run Scan</button>
                </div>
              ) : (
                <>
                  {/* Score card */}
                  <div className="card" style={{ display: "flex", gap: 24, padding: 20, marginBottom: 16, alignItems: "center" }}>
                    <div style={{ position: "relative", width: 80, height: 80 }}>
                      <svg width="80" height="80" viewBox="0 0 80 80">
                        <circle cx="40" cy="40" r="36" fill="none" stroke="var(--border-primary)" strokeWidth="6" />
                        <circle cx="40" cy="40" r="36" fill="none"
                          stroke={healthData.score >= 90 ? "var(--accent-green)" : healthData.score >= 60 ? "var(--accent-yellow)" : "var(--accent-red)"}
                          strokeWidth="6" strokeLinecap="round" strokeDasharray={`${(healthData.score / 100) * 226} 226`}
                          transform="rotate(-90 40 40)" />
                      </svg>
                      <div style={{
                        position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center",
                        fontSize: "var(--text-xl)", fontWeight: 700, color: "var(--text-primary)",
                      }}>{healthData.score}</div>
                    </div>
                    <div>
                      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                        <span style={{
                          padding: "2px 10px", borderRadius: 6, fontWeight: 700, fontSize: "var(--text-lg)",
                          background: healthData.grade === "A" ? "rgba(63,185,80,0.15)" :
                            healthData.grade === "B" ? "rgba(88,166,255,0.15)" :
                            healthData.grade === "C" ? "rgba(210,153,34,0.15)" : "rgba(248,81,73,0.15)",
                          color: healthData.grade === "A" ? "var(--accent-green)" :
                            healthData.grade === "B" ? "var(--accent-blue)" :
                            healthData.grade === "C" ? "var(--accent-yellow)" : "var(--accent-red)",
                        }}>Grade: {healthData.grade}</span>
                      </div>
                      <div style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)" }}>
                        {healthData.issues?.filter((i: any) => i.severity === "error").length || 0} errors,{" "}
                        {healthData.issues?.filter((i: any) => i.severity === "warning").length || 0} warnings
                      </div>
                    </div>
                  </div>

                  {/* Issues list */}
                  <div className="card" style={{ padding: 0, overflow: "hidden" }}>
                    <div style={{
                      padding: "10px 16px", borderBottom: "1px solid var(--border-primary)",
                      fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--text-primary)",
                    }}>Issues ({healthData.issues?.length || 0})</div>
                    <div style={{ maxHeight: "50vh", overflow: "auto" }}>
                      {(healthData.issues || []).map((issue: any, idx: number) => (
                        <div key={idx} style={{
                          display: "flex", alignItems: "flex-start", gap: 12, padding: "8px 16px",
                          borderBottom: "1px solid var(--border-subtle)",
                          background: issue.severity === "error" ? "rgba(248,81,73,0.03)" :
                            issue.severity === "warning" ? "rgba(210,153,34,0.03)" : "transparent",
                        }}>
                          <StatusDot size={8} color={
                            issue.severity === "error" ? "var(--accent-red)" :
                            issue.severity === "warning" ? "var(--accent-yellow)" :
                            "var(--accent-blue)"
                          } style={{ marginTop: 5, flexShrink: 0 }} />
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div style={{
                              display: "flex", gap: 8, alignItems: "center", marginBottom: 2,
                            }}>
                              <span style={{
                                padding: "1px 6px", borderRadius: 4, fontSize: "10px", fontWeight: 600,
                                background: "rgba(88,166,255,0.1)", color: "var(--accent-blue)",
                              }}>{issue.category}</span>
                              <span style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                                {issue.resource}
                              </span>
                            </div>
                            <div style={{ fontSize: "var(--text-sm)", color: "var(--text-secondary)" }}>{issue.message}</div>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                </>
              )}
            </div>
          ) : (
          <>
          {/* Filter bar */}
          <div style={{ display: "flex", gap: 8, marginBottom: 12, alignItems: "center" }}>
            <input ref={searchRef} type="text" value={filter} onChange={e => setFilter(e.target.value)}
              placeholder={`Filter ${activeInfo?.label || ""}...`}
              style={{
                flex: 1, padding: "6px 12px", background: "var(--bg-secondary)",
                border: "1px solid var(--border-primary)", borderRadius: 6,
                color: "var(--text-primary)", fontSize: "var(--text-sm)",
              }} />
            <span style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
              {filtered.length} {activeInfo?.label || "items"}
            </span>
          </div>

          {dataLoading ? (
            <div style={{ display: "flex", justifyContent: "center", padding: 40 }}>
              <div className="spinner" />
            </div>
          ) : filtered.length > 0 ? (
            <div className="card" style={{ overflow: "auto" }}>
              {/* CSS Grid-based layout (replaces <table> to avoid WKWebView rendering bugs) */}
              <div style={{ display: "grid", gridTemplateColumns: `${columns.map(col => col.key === "name" ? "minmax(120px, 2fr)" : col.key === "namespace" || col.key === "node" ? "minmax(80px, 1fr)" : "auto").join(" ")} auto`, minWidth: "100%" }}>
                {/* Header row */}
                {columns.map(col => (
                  <div key={`h-${col.key}`} style={{
                    textAlign: "left", padding: "10px 16px", fontSize: "var(--text-xs)", fontWeight: 600,
                    color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em",
                    background: "var(--bg-content)", borderBottom: "1px solid var(--border-primary)",
                    position: "sticky", top: 0, zIndex: 1,
                  }}>{col.label}</div>
                ))}
                <div style={{
                  textAlign: "left", padding: "10px 16px", fontSize: "var(--text-xs)", fontWeight: 600,
                  color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em",
                  background: "var(--bg-content)", borderBottom: "1px solid var(--border-primary)",
                  position: "sticky", top: 0, zIndex: 1,
                }}>Actions</div>

                {/* Data rows */}
                {filtered.map((item, idx) => (
                  <React.Fragment key={`${item.namespace}/${item.name}-${idx}`}>
                    {columns.map(col => {
                      let val = item[col.key];
                      if (col.key === "age") val = timeAgo(val);
                      if (col.key === "lastSchedule") val = val === "Never" ? val : timeAgo(val);
                      const color = col.color ? col.color(val, item) : undefined;
                      return (
                        <div key={col.key} onClick={() => openDetail(item)} onContextMenu={(e) => openCtxMenu(e, item)} style={{
                          padding: "12px 16px", cursor: "pointer",
                          fontFamily: col.mono ? "var(--font-mono)" : undefined,
                          fontSize: "var(--text-xs)",
                          color: color || (col.key === "namespace" || col.key === "age" ? "var(--text-muted)" : "var(--text-primary)"),
                          fontWeight: col.key === "name" ? 500 : undefined,
                          borderBottom: "1px solid var(--border-subtle)",
                          display: "flex", alignItems: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                        }}>
                          {(col.key === "status" || col.key === "type") && color ? (
                            <><StatusDot size={8} color={color} style={{ display: "inline-block", verticalAlign: "middle", flexShrink: 0, marginRight: 6 }} /><span>{val}</span></>
                          ) : (
                            col.key === "svcType" ? (
                              <span style={{
                                padding: "2px 6px", borderRadius: 4, fontSize: "var(--text-xs)", fontWeight: 500,
                                background: val === "ClusterIP" ? "rgba(88,166,255,0.1)" :
                                  val === "NodePort" ? "rgba(63,185,80,0.1)" :
                                  val === "LoadBalancer" ? "rgba(188,140,255,0.1)" : "rgba(255,255,255,0.05)",
                                color: val === "ClusterIP" ? "var(--accent-blue)" :
                                  val === "NodePort" ? "var(--accent-green)" :
                                  val === "LoadBalancer" ? "var(--accent-purple)" : "var(--text-secondary)",
                              }}>{val}</span>
                            ) : <span>{String(val || "")}</span>
                          )}
                        </div>
                      );
                    })}
                    {/* Actions cell */}
                    <div style={{ padding: "12px 16px", borderBottom: "1px solid var(--border-subtle)", display: "flex", gap: 4, alignItems: "center" }}>
                      {activeResource === "pods" && (
                        <>
                          <button className="btn btn-ghost" title="Exec Shell" style={{ padding: "2px 6px", color: "var(--accent-green)" }}
                            onClick={() => handleExec(item)}><TerminalSvg /></button>
                          <button className="btn btn-ghost" title="Port Forward" style={{ padding: "2px 6px", color: "var(--accent-purple)" }}
                            onClick={() => { setPortForwardModal(item); setPfLocalPort(""); setPfRemotePort(""); }}><PortForwardSvg /></button>
                        </>
                      )}
                      {activeResource === "services" && (
                        <button className="btn btn-ghost" title="Port Forward" style={{ padding: "2px 6px", color: "var(--accent-purple)" }}
                          onClick={() => {
                            setPortForwardModal(item);
                            const firstPort = item._ports?.[0]?.port;
                            setPfRemotePort(firstPort ? String(firstPort) : "");
                            setPfLocalPort(firstPort ? String(firstPort) : "");
                          }}><PortForwardSvg /></button>
                      )}
                      {activeResource === "nodes" && (
                        <>
                          <button className="btn btn-ghost" title={item.schedulable === false ? "Uncordon" : "Cordon"}
                            style={{ padding: "2px 6px", color: "var(--accent-yellow)", fontSize: "var(--text-xs)" }}
                            onClick={() => handleNodeAction(item, item.schedulable === false ? "uncordon" : "cordon")}>
                            {item.schedulable === false ? "⊕" : "⊘"}
                          </button>
                          <button className="btn btn-ghost" title="Drain"
                            style={{ padding: "2px 6px", color: "var(--accent-red)", fontSize: "var(--text-xs)" }}
                            onClick={() => handleNodeAction(item, "drain")}>
                            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M12 2v6m0 4v10M4.93 4.93l4.24 4.24m1.66 1.66l4.24 4.24M2 12h6m4 0h10M4.93 19.07l4.24-4.24m1.66-1.66l4.24-4.24"/>
                            </svg>
                          </button>
                        </>
                      )}
                      {activeInfo?.canRestart && (
                        <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-yellow)", padding: "2px 6px" }}
                          onClick={() => handleRestart(item)}><RestartIcon size={11} /></button>
                      )}
                      <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)", padding: "2px 6px" }}
                        onClick={() => handleDelete(item)}><TrashIcon size={11} /></button>
                    </div>
                  </React.Fragment>
                ))}
              </div>
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-state-title">No {activeInfo?.label || "Resources"}</div>
              <div className="empty-state-text">No {activeInfo?.label?.toLowerCase()} found in the selected namespace.</div>
            </div>
          )}
          </>
          )}
        </div>
      </div>

      {/* Detail Modal */}
      {selectedItem && (
        <div className="modal-overlay" onClick={() => setSelectedItem(null)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ width: "min(900px, 95vw)", maxHeight: "85vh" }}>
            <div className="modal-header">
              <h2 className="modal-title" style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-md)" }}>
                {selectedItem.name}
                {selectedItem.status && (
                  <span style={{ color: statusColor(selectedItem.status), fontSize: "var(--text-sm)", marginLeft: 8 }}>
                    <StatusDot size={8} color={statusColor(selectedItem.status)} style={{ display: "inline-block", verticalAlign: "middle" }} /> {selectedItem.status}
                  </span>
                )}
              </h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setSelectedItem(null)}><CloseIcon size={16} /></button>
            </div>

            {/* Meta info */}
            <div style={{ display: "flex", gap: 16, marginBottom: 12, fontSize: "var(--text-xs)", color: "var(--text-muted)", flexWrap: "wrap" }}>
              {selectedItem.namespace && <span>Namespace: <strong>{selectedItem.namespace}</strong></span>}
              {selectedItem.ready && <span>Ready: <strong>{selectedItem.ready}</strong></span>}
              {selectedItem.node && <span>Node: <strong>{selectedItem.node}</strong></span>}
              {selectedItem.age && <span>Age: <strong>{timeAgo(selectedItem.age)}</strong></span>}
            </div>

            {/* Quick actions bar */}
            {activeResource === "pods" && (
              <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
                <button className="btn" style={{
                  background: "rgba(63,185,80,0.1)", border: "1px solid rgba(63,185,80,0.3)",
                  color: "var(--accent-green)", fontSize: "var(--text-xs)", padding: "4px 10px",
                }} onClick={() => handleExec(selectedItem)}>
                  <TerminalSvg /> Exec Shell
                </button>
                <button className="btn" style={{
                  background: "rgba(188,140,255,0.1)", border: "1px solid rgba(188,140,255,0.3)",
                  color: "var(--accent-purple)", fontSize: "var(--text-xs)", padding: "4px 10px",
                }} onClick={() => { setPortForwardModal(selectedItem); setPfLocalPort(""); setPfRemotePort(""); }}>
                  <PortForwardSvg /> Port Forward
                </button>
              </div>
            )}

            {/* Detail tabs */}
            <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--border-primary)", marginBottom: 12, alignItems: "center" }}>
              {(activeResource === "pods" ? ["logs", "describe", "yaml"] : ["describe", "yaml"]).map(t => (
                <button key={t} className="btn" onClick={() => setDetailTab(t as any)} style={{
                  background: "transparent", border: "none",
                  borderBottom: detailTab === t ? "2px solid var(--accent-blue)" : "2px solid transparent",
                  color: detailTab === t ? "var(--text-primary)" : "var(--text-secondary)",
                  borderRadius: 0, padding: "6px 12px", fontWeight: detailTab === t ? 600 : 400,
                  textTransform: "capitalize",
                }}>{t}</button>
              ))}
              {/* Container selector for logs */}
              {detailTab === "logs" && containers.length > 1 && (
                <select value={selectedContainer} onChange={e => fetchContainerLogs(e.target.value)} style={{
                  marginLeft: "auto", background: "var(--bg-secondary)", border: "1px solid var(--border-primary)",
                  borderRadius: 6, padding: "2px 8px", color: "var(--accent-blue)",
                  fontSize: "var(--text-xs)", fontFamily: "var(--font-mono)",
                }}>
                  {containers.map(c => <option key={c} value={c}>{c}</option>)}
                </select>
              )}
            </div>

            {detailTab === "logs" && activeResource === "pods" && (
              <>
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
                  <button
                    className={`btn btn-ghost`}
                    style={{
                      fontSize: "var(--text-xs)", padding: "3px 10px",
                      background: followLogs ? "rgba(88,166,255,0.15)" : undefined,
                      color: followLogs ? "var(--accent-blue)" : "var(--text-secondary)",
                      border: followLogs ? "1px solid var(--accent-blue)" : undefined,
                    }}
                    onClick={() => {
                      if (followLogs) {
                        // Stop following
                        eventSourceRef.current?.close();
                        eventSourceRef.current = null;
                        setFollowLogs(false);
                      } else if (selectedItem) {
                        // Start following
                        setFollowLogs(true);
                        setLogsText("");
                        const url = k8sApi.logStreamUrl(selectedItem.namespace, selectedItem.name, selectedContainer);
                        const es = new EventSource(url);
                        eventSourceRef.current = es;
                        es.onmessage = (e) => {
                          setLogsText(prev => (prev ? prev + "\n" : "") + e.data);
                          // Auto-scroll
                          setTimeout(() => {
                            if (logViewerRef.current) {
                              logViewerRef.current.scrollTop = logViewerRef.current.scrollHeight;
                            }
                          }, 10);
                        };
                        es.onerror = () => {
                          es.close();
                          eventSourceRef.current = null;
                          setFollowLogs(false);
                        };
                      }
                    }}
                  >
                    {followLogs && <span style={{ display: "inline-block", width: 6, height: 6, borderRadius: "50%", background: "var(--accent-blue)", animation: "pulse 1.5s infinite", marginRight: 4 }} />}
                    {followLogs ? "Following" : "Follow"}
                  </button>
                  <span style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                    {followLogs ? "Live streaming..." : "Batch mode (last 200 lines)"}
                  </span>
                </div>
                <div ref={logViewerRef} className="log-viewer" style={{ maxHeight: "50vh" }}>
                  {(logsText || "No logs available").split("\n").map((line, i) => {
                    let cls = "";
                    if (/error|fatal|panic/i.test(line)) cls = "log-error";
                    else if (/warn/i.test(line)) cls = "log-warn";
                    return <div key={i} className={`log-line ${cls}`}>{line}</div>;
                  })}
                </div>
              </>
            )}

            {detailTab === "describe" && (
              <pre style={{
                padding: 12, background: "var(--bg-primary)", borderRadius: 8,
                fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "50vh",
                color: "var(--text-secondary)", margin: 0, fontFamily: "var(--font-mono)",
                whiteSpace: "pre-wrap",
              }}>{detailText || "Loading..."}</pre>
            )}

            {detailTab === "yaml" && (
              <>
                <textarea value={yamlEdited} onChange={e => setYamlEdited(e.target.value)} style={{
                  width: "100%", height: "45vh", padding: 12, background: "var(--bg-primary)",
                  borderRadius: 8, fontSize: "var(--text-xs)", resize: "vertical",
                  color: "var(--text-secondary)", fontFamily: "var(--font-mono)",
                  border: yamlEdited !== yamlText ? "1px solid var(--accent-yellow)" : "1px solid var(--border-primary)",
                  whiteSpace: "pre", overflowWrap: "normal",
                }} />
                {yamlEdited !== yamlText && (
                  <div style={{ display: "flex", gap: 8, marginTop: 8, alignItems: "center" }}>
                    <button className="btn btn-primary" onClick={handleApply} disabled={applying} style={{ fontSize: "var(--text-xs)" }}>
                      <SaveSvg /> {applying ? "Applying..." : "Apply Changes"}
                    </button>
                    <button className="btn btn-ghost" onClick={() => setYamlEdited(yamlText)} style={{ fontSize: "var(--text-xs)" }}>
                      Revert
                    </button>
                    <span style={{ fontSize: "var(--text-xs)", color: "var(--accent-yellow)" }}>
                      ⚠ Unsaved changes
                    </span>
                  </div>
                )}
              </>
            )}

            {/* Scale controls for scalable resources */}
            {["deployments", "statefulsets", "replicasets"].includes(activeResource) && selectedItem.replicas && (
              <div style={{
                display: "flex", alignItems: "center", gap: 12, padding: "10px 0", marginTop: 8,
                borderTop: "1px solid var(--border-primary)",
              }}>
                <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)" }}>Scale Replicas:</span>
                <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                  <button className="btn btn-ghost" onClick={() => {
                    const cur = scaleValue ?? parseInt(selectedItem.replicas?.split("/")[1] || "1");
                    if (cur > 0) handleScale(selectedItem, cur - 1);
                  }} style={{ padding: "4px 8px", fontSize: "var(--text-md)", fontWeight: 700, color: "var(--accent-red)" }}>−</button>
                  <input type="number" min={0} max={50}
                    value={scaleValue ?? parseInt(selectedItem.replicas?.split("/")[1] || "1")}
                    onChange={e => setScaleValue(parseInt(e.target.value) || 0)}
                    onBlur={() => { if (scaleValue !== null) handleScale(selectedItem, scaleValue); }}
                    style={{
                      width: 52, textAlign: "center", padding: "4px 8px", background: "var(--bg-secondary)",
                      border: "1px solid var(--border-primary)", borderRadius: 6,
                      color: "var(--text-primary)", fontSize: "var(--text-md)", fontFamily: "var(--font-mono)",
                    }} />
                  <button className="btn btn-ghost" onClick={() => {
                    const cur = scaleValue ?? parseInt(selectedItem.replicas?.split("/")[1] || "1");
                    handleScale(selectedItem, cur + 1);
                  }} style={{ padding: "4px 8px", fontSize: "var(--text-md)", fontWeight: 700, color: "var(--accent-green)" }}>+</button>
                </div>
              </div>
            )}

            <div className="modal-footer">
              <button className="btn btn-ghost" style={{ color: "var(--accent-red)" }}
                onClick={() => { handleDelete(selectedItem); setSelectedItem(null); }}>
                <TrashIcon size={12} /> Delete
              </button>
              {activeInfo?.canRestart && (
                <button className="btn btn-ghost" style={{ color: "var(--accent-yellow)" }}
                  onClick={() => { handleRestart(selectedItem); setSelectedItem(null); }}>
                  <RestartIcon size={12} /> Restart
                </button>
              )}
              <button className="btn btn-primary" onClick={() => setSelectedItem(null)}>Close</button>
            </div>
          </div>
        </div>
      )}

      {/* Port Forward Modal */}
      {portForwardModal && (
        <div className="modal-overlay" onClick={() => setPortForwardModal(null)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ width: 400 }}>
            <div className="modal-header">
              <h2 className="modal-title">Port Forward</h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setPortForwardModal(null)}><CloseIcon size={16} /></button>
            </div>
            <div style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", marginBottom: 16 }}>
              Forward traffic from localhost to <strong style={{ color: "var(--text-primary)" }}>{portForwardModal.name}</strong>
            </div>
            <div style={{ display: "flex", gap: 12, marginBottom: 16, alignItems: "center" }}>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>Local Port</label>
                <input type="number" value={pfLocalPort} onChange={e => setPfLocalPort(e.target.value)}
                  placeholder="8080" style={{
                    width: "100%", padding: "6px 10px", background: "var(--bg-secondary)",
                    border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)",
                    fontSize: "var(--text-sm)", fontFamily: "var(--font-mono)",
                  }} />
              </div>
              <span style={{ color: "var(--accent-purple)", marginTop: 16 }}>→</span>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>Remote Port</label>
                <input type="number" value={pfRemotePort} onChange={e => setPfRemotePort(e.target.value)}
                  placeholder="80" style={{
                    width: "100%", padding: "6px 10px", background: "var(--bg-secondary)",
                    border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)",
                    fontSize: "var(--text-sm)", fontFamily: "var(--font-mono)",
                  }} />
              </div>
            </div>

            {/* Active forwards */}
            {activeForwards.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: 6 }}>Active Forwards</div>
                {activeForwards.map(fwd => {
                  const port = fwd.split(":")[0];
                  return (
                    <div key={fwd} style={{
                      display: "flex", justifyContent: "space-between", alignItems: "center",
                      padding: "4px 8px", background: "rgba(188,140,255,0.05)", borderRadius: 4,
                      marginBottom: 4, fontSize: "var(--text-xs)",
                    }}>
                      <span style={{ color: "var(--accent-purple)", fontFamily: "var(--font-mono)" }}>localhost:{port}</span>
                      <button className="btn btn-ghost" onClick={() => stopPortForward(port)}
                        style={{ color: "var(--accent-red)", fontSize: "var(--text-xs)", padding: "2px 4px" }}>
                        <StopSvg /> Stop
                      </button>
                    </div>
                  );
                })}
              </div>
            )}

            <div className="modal-footer">
              <button className="btn btn-ghost" onClick={() => setPortForwardModal(null)}>Cancel</button>
              <button className="btn btn-primary" onClick={startPortForward}
                disabled={!pfLocalPort || !pfRemotePort}>
                <PortForwardSvg /> Start Forward
              </button>
            </div>
          </div>
        </div>
      )}

      <ConfirmDialog {...ConfirmDialogProps} />
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={getCtxItems(ctxMenu.item)} onClose={() => setCtxMenu(null)} />}

      {/* Benchmark Modal */}
      {benchModal && (
        <div className="modal-overlay" onClick={() => setBenchModal(null)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ maxWidth: 520 }}>
            <div className="modal-header">
              <h3>⚡ HTTP Benchmark — {benchModal.name}</h3>
              <button className="btn btn-ghost" onClick={() => setBenchModal(null)}><CloseIcon /></button>
            </div>
            <div className="modal-body" style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              <div>
                <label style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Target URL</label>
                <input type="text" value={benchUrl} onChange={e => setBenchUrl(e.target.value)}
                  className="input" style={{ width: "100%", marginTop: 4 }} placeholder="http://localhost:8080" />
              </div>
              <div style={{ display: "flex", gap: 12 }}>
                <div style={{ flex: 1 }}>
                  <label style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Method</label>
                  <select value={benchMethod} onChange={e => setBenchMethod(e.target.value)}
                    className="input" style={{ width: "100%", marginTop: 4 }}>
                    {["GET", "POST", "PUT", "DELETE"].map(m => <option key={m}>{m}</option>)}
                  </select>
                </div>
                <div style={{ flex: 1 }}>
                  <label style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Concurrency</label>
                  <input type="number" value={benchConc} onChange={e => setBenchConc(+e.target.value)}
                    className="input" style={{ width: "100%", marginTop: 4 }} min={1} max={100} />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Requests</label>
                  <input type="number" value={benchReqs} onChange={e => setBenchReqs(+e.target.value)}
                    className="input" style={{ width: "100%", marginTop: 4 }} min={1} max={10000} />
                </div>
              </div>
              <button className="btn btn-primary" disabled={benchRunning || !benchUrl} onClick={async () => {
                setBenchRunning(true); setBenchResult(null);
                try {
                  const raw = await k8sApi.benchmark(benchUrl, benchConc, benchReqs, benchMethod);
                  setBenchResult(typeof raw === "string" ? JSON.parse(raw) : raw);
                } catch (e) {
                  globalToast("error", `Benchmark failed: ${e}`);
                } finally {
                  setBenchRunning(false);
                }
              }}>
                {benchRunning ? <><div className="spinner" style={{ width: 14, height: 14 }} /> Running...</> : "Run Benchmark"}
              </button>
              {benchResult && (
                <div className="card" style={{ padding: 16 }}>
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 8, marginBottom: 12 }}>
                    <div style={{ textAlign: "center" }}>
                      <div style={{ fontSize: "var(--text-xl)", fontWeight: 700, color: "var(--accent-green)" }}>{benchResult.requests_per_sec}</div>
                      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>req/s</div>
                    </div>
                    <div style={{ textAlign: "center" }}>
                      <div style={{ fontSize: "var(--text-xl)", fontWeight: 700, color: "var(--accent-blue)" }}>{benchResult.success}/{benchResult.total_requests}</div>
                      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>success</div>
                    </div>
                    <div style={{ textAlign: "center" }}>
                      <div style={{ fontSize: "var(--text-xl)", fontWeight: 700, color: benchResult.failed > 0 ? "var(--accent-red)" : "var(--text-secondary)" }}>{benchResult.failed}</div>
                      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>failed</div>
                    </div>
                  </div>
                  <table style={{ width: "100%", fontSize: "var(--text-xs)", fontFamily: "var(--font-mono)" }}>
                    <thead>
                      <tr style={{ color: "var(--text-muted)", borderBottom: "1px solid var(--border-primary)" }}>
                        <th style={{ textAlign: "left", padding: 4 }}>Metric</th>
                        <th style={{ textAlign: "right", padding: 4 }}>Value</th>
                      </tr>
                    </thead>
                    <tbody>
                      {[{ l: "Avg", v: benchResult.avg_latency_ms }, { l: "Min", v: benchResult.min_latency_ms }, { l: "Max", v: benchResult.max_latency_ms }, { l: "P50", v: benchResult.p50_ms }, { l: "P95", v: benchResult.p95_ms, c: "var(--accent-yellow)" }, { l: "P99", v: benchResult.p99_ms, c: "var(--accent-red)" }].map(r => (
                        <tr key={r.l}>
                          <td style={{ padding: 4, color: "var(--text-secondary)" }}>{r.l}</td>
                          <td style={{ padding: 4, textAlign: "right", color: r.c || "var(--text-primary)" }}>{r.v}ms</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  <div style={{ marginTop: 8, fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                    Total time: {benchResult.total_time_ms}ms · {benchMethod} · Concurrency: {benchResult.concurrency}
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}

const StopSvg = () => (
  <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor"><rect x="4" y="4" width="16" height="16" rx="2"/></svg>
);

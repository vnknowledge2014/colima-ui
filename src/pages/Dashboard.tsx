import React, { useEffect, useCallback } from "react";
import { ColimaInstance, SystemInfo, dockerApi, volumesApi, networksApi, composeApi, k8sApi, limaApi, kindApi } from "../lib/api";
import { useAtom } from "jotai";
import {
  dashboardCountsAtom, dashboardK8sAtom, dashboardVMsAtom, dashboardLastFetchAtom,
} from "../store/dashboardAtom";

type Page = "dashboard" | "instances" | "containers" | "images" | "volumes" | "networks" | "compose" | "kubernetes" | "linux-vms" | "terminal" | "models" | "settings";

interface DashboardProps {
  instances: ColimaInstance[];
  systemInfo: SystemInfo | null;
  loading: boolean;
  onNavigate: (page: Page) => void;
}

const formatBytes = (bytes: number): string => {
  if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
  if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
  return `${bytes} B`;
};

const STALE_MS = 30_000; // 30 seconds

export default function Dashboard({ instances, systemInfo, loading, onNavigate }: DashboardProps) {
  const runningCount = instances.filter((i) => i.status === "Running").length;
  const stoppedCount = instances.filter((i) => i.status !== "Running").length;
  const totalCpus = instances.filter(i => i.status === "Running").reduce((sum, i) => sum + i.cpus, 0);

  const [dockerCountsRaw, setDockerCounts] = useAtom(dashboardCountsAtom);
  const [k8sStatusRaw, setK8sStatus] = useAtom(dashboardK8sAtom);
  const [linuxVMs, setLinuxVMs] = useAtom(dashboardVMsAtom);
  const [lastFetch, setLastFetch] = useAtom(dashboardLastFetchAtom);

  const dockerCounts = dockerCountsRaw ?? { containers: 0, running: 0, images: 0, volumes: 0, networks: 0, composeProjects: 0 };
  const k8sStatus = k8sStatusRaw ?? { connected: false, pods: 0, namespaces: 0, kindClusters: 0 };

  const fetchDockerCounts = useCallback(async () => {
    try {
      const [containers, images, volumes, networks, compose] = await Promise.allSettled([
        dockerApi.listContainers(true),
        dockerApi.listImages(),
        volumesApi.listVolumes(),
        networksApi.listNetworks(),
        composeApi.list(),
      ]);
      setDockerCounts({
        containers: containers.status === "fulfilled" ? containers.value.length : 0,
        running: containers.status === "fulfilled" ? containers.value.filter(c => c.State === "running").length : 0,
        images: images.status === "fulfilled" ? images.value.length : 0,
        volumes: volumes.status === "fulfilled" ? volumes.value.length : 0,
        networks: networks.status === "fulfilled" ? networks.value.length : 0,
        composeProjects: compose.status === "fulfilled" ? compose.value.length : 0,
      });
    } catch { /* ignore */ }
  }, [setDockerCounts]);

  const fetchK8sStatus = useCallback(async () => {
    try {
      const [checkResult, kindRaw] = await Promise.allSettled([
        k8sApi.check(),
        kindApi.list(),
      ]);
      const connected = checkResult.status === "fulfilled";
      let pods = 0, namespaces = 0;
      if (connected) {
        const [nsRaw, podsRaw] = await Promise.allSettled([
          k8sApi.namespaces(),
          k8sApi.pods(""),
        ]);
        if (nsRaw.status === "fulfilled") {
          const ns = nsRaw.value;
          namespaces = Array.isArray(ns) ? ns.length : (typeof ns === "string" ? (ns.match(/"name"/gi) || []).length : 0);
        }
        if (podsRaw.status === "fulfilled") {
          const p = podsRaw.value;
          pods = Array.isArray(p) ? p.length : (typeof p === "string" ? (p.match(/"name"/gi) || []).length : 0);
        }
      }
      const kindClusters = kindRaw.status === "fulfilled"
        ? kindRaw.value.trim().split("\n").filter(Boolean).filter(c => c !== "No kind clusters found.").length
        : 0;
      setK8sStatus({ connected, pods, namespaces, kindClusters });
    } catch { setK8sStatus({ connected: false, pods: 0, namespaces: 0, kindClusters: 0 }); }
  }, [setK8sStatus]);

  const fetchLinuxVMs = useCallback(async () => {
    try {
      const vms = await limaApi.list();
      setLinuxVMs(vms);
    } catch { setLinuxVMs([]); }
  }, [setLinuxVMs]);

  useEffect(() => {
    if (!loading) {
      const now = Date.now();
      if (now - lastFetch > STALE_MS) {
        fetchDockerCounts();
        fetchK8sStatus();
        fetchLinuxVMs();
        setLastFetch(now);
      }
    }
  }, [loading, lastFetch, setLastFetch, fetchDockerCounts, fetchK8sStatus, fetchLinuxVMs]);

  if (loading) {
    return (
      <div className="loading-screen">
        <div className="spinner" />
        <span>Loading...</span>
      </div>
    );
  }

  return (
    <>
      <div className="content-header">
        <h1>Dashboard</h1>
      </div>

      <div className="content-body">
        {/* Instance Stat Cards */}
        <div className="card-grid" style={{ marginBottom: 24 }}>
          <div className="card stat-card" onClick={() => onNavigate("instances")} style={{ cursor: "pointer" }}>
            <div className="stat-icon" style={{ background: "rgba(88, 166, 255, 0.1)", color: "var(--accent-blue)" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
                <line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/>
              </svg>
            </div>
            <span className="stat-label">VM Instances</span>
            <span className="stat-value">{instances.length}</span>
          </div>

          <div className="card stat-card">
            <div className="stat-icon" style={{ background: "rgba(63, 185, 80, 0.1)", color: "var(--accent-green)" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/>
              </svg>
            </div>
            <span className="stat-label">Running VMs</span>
            <span className="stat-value" style={{ color: "var(--accent-green)" }}>{runningCount}</span>
          </div>

          <div className="card stat-card">
            <div className="stat-icon" style={{ background: "rgba(248, 81, 73, 0.1)", color: "var(--accent-red)" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
              </svg>
            </div>
            <span className="stat-label">Stopped VMs</span>
            <span className="stat-value" style={{ color: "var(--accent-red)" }}>{stoppedCount}</span>
          </div>

          <div className="card stat-card">
            <div className="stat-icon" style={{ background: "rgba(188, 140, 255, 0.1)", color: "var(--accent-purple)" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/>
                <path d="M15 2v2"/><path d="M15 20v2"/><path d="M2 15h2"/><path d="M2 9h2"/><path d="M20 15h2"/><path d="M20 9h2"/><path d="M9 2v2"/><path d="M9 20v2"/>
              </svg>
            </div>
            <span className="stat-label">Total CPUs</span>
            <span className="stat-value">{totalCpus}</span>
          </div>
        </div>

        {/* Docker Resource Overview */}
        <div className="card" style={{ marginBottom: 24 }}>
          <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600, marginBottom: 16 }}>Docker Resources</h3>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: 12 }}>
            <ResourceCard label="Containers" value={dockerCounts.containers} sub={`${dockerCounts.running} running`}
              color="var(--accent-blue)" onClick={() => onNavigate("containers")} />
            <ResourceCard label="Images" value={dockerCounts.images} color="var(--accent-green)" onClick={() => onNavigate("images")} />
            <ResourceCard label="Volumes" value={dockerCounts.volumes} color="var(--accent-purple)" onClick={() => onNavigate("volumes")} />
            <ResourceCard label="Networks" value={dockerCounts.networks} color="var(--accent-orange)" onClick={() => onNavigate("networks")} />
            <ResourceCard label="Compose" value={dockerCounts.composeProjects} color="var(--accent-blue)" onClick={() => onNavigate("compose")} />
          </div>
        </div>

        {/* Kubernetes & Infrastructure */}
        <div className="card" style={{ marginBottom: 24 }}>
          <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600, marginBottom: 16 }}>Infrastructure</h3>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: 12 }}>
            <ResourceCard
              label="Kubernetes"
              value={k8sStatus.connected ? k8sStatus.pods : 0}
              sub={k8sStatus.connected ? `${k8sStatus.namespaces} namespaces` : "Disconnected"}
              color={k8sStatus.connected ? "var(--accent-green)" : "var(--accent-red)"}
              onClick={() => onNavigate("kubernetes")}
            />
            <ResourceCard
              label="Kind Clusters"
              value={k8sStatus.kindClusters}
              color="var(--accent-purple)"
              onClick={() => onNavigate("instances")}
            />
            <ResourceCard
              label="Linux VMs"
              value={linuxVMs.length}
              sub={`${linuxVMs.filter(v => v.status === "Running").length} running`}
              color="var(--accent-orange)"
              onClick={() => onNavigate("linux-vms")}
            />
          </div>
        </div>

        {/* System Info */}
        {systemInfo && (
          <div className="card" style={{ marginBottom: 24 }}>
            <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600, marginBottom: 16 }}>System Status</h3>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 16 }}>
              <StatusBadge label="Colima" installed={systemInfo.colima_installed} />
              <StatusBadge label="Docker" installed={systemInfo.docker_installed} />
              <StatusBadge label="Lima" installed={systemInfo.lima_installed} />
            </div>
          </div>
        )}

        {/* Quick Instance List */}
        {instances.length > 0 ? (
          <div className="card">
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
              <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600 }}>Instances</h3>
              <button className="btn btn-ghost" onClick={() => onNavigate("instances")} style={{ fontSize: "var(--text-xs)" }}>
                View All →
              </button>
            </div>
            <table className="data-table">
              <thead>
                <tr>
                  <th>Profile</th><th>Status</th><th>Runtime</th><th>Arch</th><th>Resources</th>
                </tr>
              </thead>
              <tbody>
                {instances.map((inst) => (
                  <tr key={inst.name}>
                    <td style={{ fontWeight: 500 }}>{inst.name}</td>
                    <td>
                      <span className={`badge badge-${inst.status === "Running" ? "running" : "stopped"}`}>
                        <span className="badge-dot" />{inst.status}
                      </span>
                    </td>
                    <td style={{ color: "var(--text-secondary)" }}>{inst.runtime}</td>
                    <td style={{ color: "var(--text-secondary)" }}>{inst.arch}</td>
                    <td style={{ color: "var(--text-secondary)", fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>
                      {inst.cpus} CPU · {formatBytes(inst.memory)} · {formatBytes(inst.disk)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
              </svg>
            </div>
            <div className="empty-state-title">No instances found</div>
            <div className="empty-state-text">Create your first Colima instance to get started.</div>
            <button className="btn btn-primary" onClick={() => onNavigate("instances")}>Create Instance</button>
          </div>
        )}
      </div>
    </>
  );
}

const ResourceCard = React.memo(function ResourceCard({ label, value, sub, color, onClick }: { label: string; value: number; sub?: string; color: string; onClick: () => void }) {
  return (
    <div onClick={onClick} style={{
      padding: 14, background: "var(--bg-primary)", borderRadius: 8, cursor: "pointer",
      borderLeft: `3px solid ${color}`, transition: "transform 100ms",
    }}>
      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: "var(--text-xl)", fontWeight: 700, fontFamily: "var(--font-mono)", color }}>{value}</div>
      {sub && <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 2 }}>{sub}</div>}
    </div>
  );
});

const StatusBadge = React.memo(function StatusBadge({ label, installed }: { label: string; installed: boolean }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <span className={`badge ${installed ? "badge-running" : "badge-stopped"}`}>
        <span className="badge-dot" />
        {installed ? "Installed" : "Not Found"}
      </span>
      <span style={{ fontSize: "var(--text-sm)", color: "var(--text-secondary)" }}>{label}</span>
    </div>
  );
});

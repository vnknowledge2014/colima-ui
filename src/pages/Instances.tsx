import { useState, useCallback, useEffect } from "react";
import { colimaApi, ColimaInstance, StartConfig, kindApi } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { CloseIcon, CheckIcon, StatusDot } from "../components/Icons";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import { useHotkeys } from "../hooks/useHotkeys";

interface InstancesProps {
  instances: ColimaInstance[];
  onRefresh: () => void;
}

const formatBytes = (bytes: number): string => {
  if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
  if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
  return `${bytes} B`;
};

type SelectedItem = { type: "colima"; data: ColimaInstance } | { type: "kind"; name: string };

/* ===== Confirmation Dialog ===== */
function ConfirmDialog({ title, message, confirmLabel, danger, onConfirm, onCancel }: {
  title: string; message: string; confirmLabel: string; danger?: boolean;
  onConfirm: () => void; onCancel: () => void;
}) {
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: "min(400px, 90vw)" }}>
        <div className="modal-header"><h2 className="modal-title">{title}</h2></div>
        <p style={{ color: "var(--text-secondary)", fontSize: "var(--text-sm)", lineHeight: 1.6 }}>{message}</p>
        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onCancel}>Cancel</button>
          <button className={`btn ${danger ? "btn-danger" : "btn-primary"}`} onClick={onConfirm}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}

/* ===== Create Instance Dialog ===== */
function CreateInstanceDialog({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [config, setConfig] = useState<StartConfig>({
    profile: "default", runtime: "docker", cpus: 2, memory: 2, disk: 60, vm_type: "vz",
    kubernetes: false, kubernetes_version: "", arch: "", mount_type: "", mounts: [], dns: [], network_address: false,
  });
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async () => {
    if (!config.profile.trim()) { setError("Profile name is required"); return; }
    const normalizedConfig = { ...config, profile: config.profile.trim().toLowerCase() };
    setCreating(true); setError(null);
    // Fire-and-forget: close dialog immediately, let poller track progress
    globalToast("success", `Starting instance '${normalizedConfig.profile}'... This may take a minute.`);
    onCreated();
    onClose();
    colimaApi.startInstance(normalizedConfig)
      .then(() => globalToast("success", `Instance '${normalizedConfig.profile}' started successfully`))
      .catch((e) => globalToast("error", `Failed to start '${normalizedConfig.profile}': ${e}`));
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2 className="modal-title">Create Instance</h2>
          <button className="btn btn-icon btn-ghost" onClick={onClose}><CloseIcon size={16} /></button>
        </div>
        {error && <div style={{ padding: "8px 12px", background: "rgba(248, 81, 73, 0.1)", borderRadius: "var(--radius-md)", color: "var(--accent-red)", fontSize: "var(--text-sm)", marginBottom: 16 }}>{error}</div>}

        <div className="form-group">
          <label className="form-label">Profile Name</label>
          <input className="input" value={config.profile} onChange={(e) => setConfig({ ...config, profile: e.target.value })} placeholder="default" />
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
          <div className="form-group">
            <label className="form-label">Runtime</label>
            <select className="input select" value={config.runtime} onChange={(e) => setConfig({ ...config, runtime: e.target.value })}>
              <option value="docker">Docker</option><option value="containerd">Containerd</option><option value="incus">Incus</option>
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">VM Type</label>
            <select className="input select" value={config.vm_type} onChange={(e) => setConfig({ ...config, vm_type: e.target.value })}>
              <option value="vz">VZ (macOS 13+)</option><option value="qemu">QEMU</option>
            </select>
          </div>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 16 }}>
          <div className="form-group">
            <label className="form-label">CPUs</label>
            <input className="input" type="number" min={1} max={32} value={config.cpus} onChange={(e) => setConfig({ ...config, cpus: Number(e.target.value) })} />
          </div>
          <div className="form-group">
            <label className="form-label">Memory (GiB)</label>
            <input className="input" type="number" min={1} max={128} value={config.memory} onChange={(e) => setConfig({ ...config, memory: Number(e.target.value) })} />
          </div>
          <div className="form-group">
            <label className="form-label">Disk (GiB)</label>
            <input className="input" type="number" min={10} max={1000} value={config.disk} onChange={(e) => setConfig({ ...config, disk: Number(e.target.value) })} />
          </div>
        </div>
        <div className="form-group">
          <label className="form-label">Architecture</label>
          <select className="input select" value={config.arch} onChange={(e) => setConfig({ ...config, arch: e.target.value })}>
            <option value="">Default (host)</option><option value="aarch64">aarch64 (ARM64)</option><option value="x86_64">x86_64 (Intel)</option>
          </select>
        </div>
        <div className="form-group">
          <label className="form-label">Mount Type</label>
          <select className="input select" value={config.mount_type} onChange={(e) => setConfig({ ...config, mount_type: e.target.value })}>
            <option value="">Default</option><option value="virtiofs">VirtioFS (macOS)</option><option value="sshfs">SSHFS</option><option value="9p">9P</option>
          </select>
        </div>
        <div style={{ borderTop: "1px solid var(--border-subtle)", paddingTop: 16, marginTop: 8 }}>
          <div className="form-group" style={{ flexDirection: "row", alignItems: "center", gap: 10 }}>
            <input type="checkbox" id="k8s-check" checked={config.kubernetes} onChange={(e) => setConfig({ ...config, kubernetes: e.target.checked })} style={{ accentColor: "var(--accent-blue)" }} />
            <label htmlFor="k8s-check" className="form-label" style={{ marginBottom: 0 }}>Enable Kubernetes (K3s)</label>
          </div>
          <div className="form-group" style={{ flexDirection: "row", alignItems: "center", gap: 10 }}>
            <input type="checkbox" id="net-addr" checked={config.network_address} onChange={(e) => setConfig({ ...config, network_address: e.target.checked })} style={{ accentColor: "var(--accent-blue)" }} />
            <label htmlFor="net-addr" className="form-label" style={{ marginBottom: 0 }}>Reachable Network Address</label>
          </div>
        </div>
        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" onClick={handleCreate} disabled={creating}>
            {creating ? <><div className="spinner" style={{ width: 14, height: 14 }} /> Creating...</> : "Create & Start"}
          </button>
        </div>
      </div>
    </div>
  );
}

/* ===== Create Kind Dialog ===== */
function CreateKindDialog({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [newName, setNewName] = useState("my-cluster");
  const [newImage, setNewImage] = useState("");

  const handleCreate = async () => {
    if (!newName.trim()) return;
    const name = newName.trim().toLowerCase();
    const image = newImage.trim();
    // Fire-and-forget: close dialog immediately, notify when done
    globalToast("success", `Creating Kind cluster '${name}'... This may take a few minutes.`);
    onClose();
    kindApi.create(name, image)
      .then(() => { globalToast("success", `Kind cluster '${name}' created successfully`); onCreated(); })
      .catch((e) => globalToast("error", `Kind cluster creation failed: ${e}`));
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()} style={{ width: 460 }}>
        <div style={{
          padding: "16px 20px", display: "flex", justifyContent: "space-between", alignItems: "center",
          background: "linear-gradient(135deg, rgba(167,139,250,0.1) 0%, rgba(124,58,237,0.05) 100%)",
          borderBottom: "1px solid rgba(167,139,250,0.15)",
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <KindIcon />
            <div>
              <h2 style={{ margin: 0, fontSize: "var(--text-md)", fontWeight: 600 }}>Create Kind Cluster</h2>
              <div style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: 1 }}>Kubernetes in Docker</div>
            </div>
          </div>
          <button className="btn btn-icon btn-ghost" onClick={onClose}><CloseIcon size={16} /></button>
        </div>
        <div style={{ padding: "20px 20px 8px", display: "flex", flexDirection: "column", gap: 16 }}>
          <div>
            <label style={{ fontSize: "var(--text-xs)", color: "var(--text-secondary)", display: "block", marginBottom: 6, fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.04em" }}>Cluster Name</label>
            <input type="text" value={newName} onChange={e => setNewName(e.target.value)} placeholder="my-cluster" autoFocus className="input" style={{ width: "100%", fontFamily: "var(--font-mono)" }} />
          </div>
          <div>
            <label style={{ fontSize: "var(--text-xs)", color: "var(--text-secondary)", display: "block", marginBottom: 6, fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.04em" }}>
              Node Image <span style={{ fontWeight: 400, textTransform: "none", opacity: 0.6 }}>(optional)</span>
            </label>
            <input type="text" value={newImage} onChange={e => setNewImage(e.target.value)} placeholder="kindest/node:v1.30.0" className="input" style={{ width: "100%", fontFamily: "var(--font-mono)" }} />
          </div>
        </div>
        <div style={{ padding: "16px 20px", display: "flex", justifyContent: "flex-end", gap: 8, borderTop: "1px solid var(--border-primary)", marginTop: 12 }}>
          <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" onClick={handleCreate} disabled={!newName.trim()}
            style={{ background: "linear-gradient(135deg, #a78bfa, #7c3aed)", border: "none", boxShadow: "0 2px 8px rgba(124,58,237,0.3)" }}>
            Create Cluster
          </button>
        </div>
      </div>
    </div>
  );
}

const KindIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
    <path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" stroke="url(#kindGrad)" strokeWidth="1.5" strokeLinejoin="round"/>
    <circle cx="12" cy="12" r="3" stroke="url(#kindGrad)" strokeWidth="1.5"/>
    <defs><linearGradient id="kindGrad" x1="3" y1="2" x2="21" y2="22"><stop stopColor="#a78bfa"/><stop offset="1" stopColor="#7c3aed"/></linearGradient></defs>
  </svg>
);

/* ===== Persistent pending operations (survives component unmount) ===== */
const pendingOps = new Map<string, string>(); // key: profileId, value: action being performed

const ColimaIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="1.5">
    <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
    <circle cx="6" cy="6" r="1" fill="var(--accent-blue)"/><circle cx="6" cy="18" r="1" fill="var(--accent-blue)"/>
  </svg>
);

/* ===== Main Instances Page ===== */
export default function Instances({ instances, onRefresh }: InstancesProps) {
  const [showCreate, setShowCreate] = useState(false);
  const [showCreateKind, setShowCreateKind] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(pendingOps.get("instance-action") || null);
  const [confirm, setConfirm] = useState<{ title: string; message: string; confirmLabel: string; danger: boolean; onConfirm: () => void } | null>(null);
  const [selected, setSelected] = useState<SelectedItem | null>(null);
  const [kindClusters, setKindClusters] = useState<string[]>([]);
  const [kindLoading, setKindLoading] = useState(true);
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);

  // Hotkeys
  useHotkeys({
    "escape": () => { setShowCreate(false); setShowCreateKind(false); setConfirm(null); setCtxMenu(null); },
  });

  const openColimaCtx = (e: React.MouseEvent, inst: ColimaInstance) => {
    e.preventDefault();
    const profileId = inst.name === "colima" ? "default" : inst.name.replace("colima-", "");
    const isRunning = inst.status === "Running";
    const items: ContextMenuItem[] = [];
    if (isRunning) {
      items.push({ label: "Stop", action: () => handleAction(profileId, "stop") });
      items.push({ label: "Restart", action: () => handleAction(profileId, "restart") });
    } else {
      items.push({ label: "Start", action: () => handleAction(profileId, "start") });
    }
    items.push({ divider: true, label: "", action: () => {} });
    items.push({ label: "Copy Name", action: () => { navigator.clipboard.writeText(inst.name); globalToast("success", "Name copied"); } });
    items.push({ divider: true, label: "", action: () => {} });
    items.push({ label: "Delete", danger: true, action: () => handleAction(profileId, "delete") });
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  };

  const openKindCtx = (e: React.MouseEvent, name: string) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY, items: [
      { label: "Copy Context", action: () => { navigator.clipboard.writeText(`kind-${name}`); globalToast("success", "Context copied"); } },
      { divider: true, label: "", action: () => {} },
      { label: "Delete", danger: true, action: () => handleDeleteKind(name) },
    ]});
  };

  const fetchKind = useCallback(async () => {
    try {
      const raw = await kindApi.list();
      setKindClusters(raw.trim().split("\n").filter(Boolean).filter(c => c !== "No kind clusters found."));
    } catch { setKindClusters([]); }
    setKindLoading(false);
  }, []);

  useEffect(() => { fetchKind(); }, [fetchKind]);

  // Auto-select first instance if nothing selected
  useEffect(() => {
    if (!selected && instances.length > 0) {
      setSelected({ type: "colima", data: instances[0] });
    }
  }, [instances, selected]);

  // Keep selected instance data fresh
  useEffect(() => {
    if (selected?.type === "colima") {
      const fresh = instances.find(i => i.name === selected.data.name);
      if (fresh && JSON.stringify(fresh) !== JSON.stringify(selected.data)) {
        setSelected({ type: "colima", data: fresh });
      }
    }
  }, [instances, selected]);


  const handleAction = async (profile: string, action: "start" | "stop" | "restart" | "delete") => {
    if (action === "delete") {
      setConfirm({
        title: "Delete Instance", danger: true, confirmLabel: "Delete",
        message: `Are you sure you want to delete instance "${profile}"? This action cannot be undone.`,
        onConfirm: async () => {
          setConfirm(null); setActionLoading(`${profile}-delete`);
          try { await colimaApi.deleteInstance(profile, true); globalToast("success", `Instance '${profile}' deleted`); setSelected(null); onRefresh(); }
          catch (e) { globalToast("error", String(e)); }
          finally { setActionLoading(null); }
        },
      });
      return;
    }

    // Fire-and-forget for long-running actions — poller tracks real-time status
    const labels: Record<string, string> = { start: "Starting", stop: "Stopping", restart: "Restarting" };
    globalToast("success", `${labels[action]} instance '${profile}'...`);
    setActionLoading(`${profile}-${action}`);
    pendingOps.set("instance-action", `${profile}-${action}`);

    const defaultConfig: StartConfig = { profile, runtime: "docker", cpus: 2, memory: 2, disk: 60, vm_type: "", kubernetes: false, kubernetes_version: "", arch: "", mount_type: "", mounts: [], dns: [], network_address: false };
    const runAction = async () => {
      switch (action) {
        case "start": await colimaApi.startInstance(defaultConfig); break;
        case "stop": await colimaApi.stopInstance(profile); break;
        case "restart": await colimaApi.stopInstance(profile); await colimaApi.startInstance(defaultConfig); break;
      }
    };

    runAction()
      .then(() => { const past: Record<string, string> = { start: "started", stop: "stopped", restart: "restarted" }; globalToast("success", `Instance '${profile}' ${past[action]}`); onRefresh(); })
      .catch((e) => globalToast("error", `${action} failed: ${e}`))
      .finally(() => { setActionLoading(null); pendingOps.delete("instance-action"); });
  };

  const handleDeleteKind = async (name: string) => {
    setConfirm({
      title: "Delete Kind Cluster", danger: true, confirmLabel: "Delete",
      message: `Delete Kind cluster "${name}"? This cannot be undone.`,
      onConfirm: async () => {
        setConfirm(null); setActionLoading(`kind-${name}-delete`);
        try { await kindApi.delete(name); globalToast("success", `Kind cluster "${name}" deleted`); setSelected(null); fetchKind(); }
        catch (e) { globalToast("error", String(e)); }
        finally { setActionLoading(null); }
      },
    });
  };

  const runningColima = instances.filter(i => i.status === "Running").length;
  const totalItems = instances.length + kindClusters.length;

  return (
    <>
      <div className="content-header">
        <h1>
          Instances
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {runningColima} running · {totalItems} total
          </span>
        </h1>
        <div className="content-header-actions">
          <button className="btn btn-ghost" onClick={() => { onRefresh(); fetchKind(); }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
            Refresh
          </button>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            New Instance
          </button>
          <button className="btn btn-ghost" onClick={() => setShowCreateKind(true)} style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <KindIcon /> Kind Cluster
          </button>
        </div>
      </div>

      <div className="content-body">

        {totalItems === 0 && !kindLoading ? (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
              </svg>
            </div>
            <div className="empty-state-title">No instances</div>
            <div className="empty-state-text">Create a Colima VM or Kind cluster to get started.</div>
            <button className="btn btn-primary" onClick={() => setShowCreate(true)}>New Instance</button>
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "320px 1fr", gap: 0, minHeight: "calc(100vh - 140px)" }}>
            {/* Left: Item List */}
            <div style={{
              borderRight: "1px solid var(--border-primary)", overflowY: "auto",
              background: "var(--bg-primary)", borderRadius: "12px 0 0 12px",
            }}>
              {/* Colima section */}
              <div style={{ padding: "10px 14px 6px", fontSize: "10px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.06em", color: "var(--text-muted)" }}>
                Colima Instances ({instances.length})
              </div>
              {instances.map(inst => {
                const isRunning = inst.status === "Running";
                const isSelected = selected?.type === "colima" && selected.data.name === inst.name;
                return (
                  <div key={inst.name} onClick={() => setSelected({ type: "colima", data: inst })} onContextMenu={(e) => openColimaCtx(e, inst)} style={{
                    padding: "10px 14px", cursor: "pointer", display: "flex", alignItems: "center", gap: 10,
                    background: isSelected ? "var(--bg-card-hover)" : "transparent",
                    borderLeft: isSelected ? "3px solid var(--accent-blue)" : "3px solid transparent",
                    transition: "all 150ms ease",
                  }}>
                    <div style={{
                      width: 8, height: 8, borderRadius: "50%", flexShrink: 0,
                      background: isRunning ? "var(--accent-green)" : "var(--text-muted)",
                      boxShadow: isRunning ? "0 0 6px var(--accent-green)" : "none",
                    }} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontWeight: 600, fontSize: "var(--text-sm)", color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{inst.name}</div>
                      <div style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: 1 }}>
                        {inst.runtime} · {inst.cpus} CPU · {formatBytes(inst.memory)}
                      </div>
                    </div>
                    <span style={{
                      padding: "2px 6px", borderRadius: 10, fontSize: "10px", fontWeight: 600,
                      background: isRunning ? "rgba(63,185,80,0.1)" : "rgba(139,148,158,0.1)",
                      color: isRunning ? "var(--accent-green)" : "var(--text-muted)",
                    }}>{inst.status}</span>
                  </div>
                );
              })}
              {instances.length === 0 && (
                <div style={{ padding: "16px 14px", fontSize: "var(--text-xs)", color: "var(--text-muted)", textAlign: "center" }}>
                  No Colima instances. Click "New Instance" to create one.
                </div>
              )}

              {/* Kind section */}
              <div style={{ padding: "14px 14px 6px", fontSize: "10px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.06em", color: "var(--text-muted)", borderTop: "1px solid var(--border-primary)", marginTop: 4 }}>
                Kind Clusters ({kindLoading ? "..." : kindClusters.length})
              </div>
              {kindLoading ? (
                <div style={{ display: "flex", justifyContent: "center", padding: 16 }}><div className="spinner" style={{ width: 16, height: 16 }} /></div>
              ) : kindClusters.length > 0 ? (
                kindClusters.map(name => {
                  const isSelected = selected?.type === "kind" && selected.name === name;
                  return (
                    <div key={name} onClick={() => setSelected({ type: "kind", name })} onContextMenu={(e) => openKindCtx(e, name)} style={{
                      padding: "10px 14px", cursor: "pointer", display: "flex", alignItems: "center", gap: 10,
                      background: isSelected ? "rgba(167,139,250,0.08)" : "transparent",
                      borderLeft: isSelected ? "3px solid var(--accent-purple)" : "3px solid transparent",
                      transition: "all 150ms ease",
                    }}>
                      <KindIcon />
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ fontWeight: 600, fontSize: "var(--text-sm)", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{name}</div>
                        <div style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: 1 }}>kind-{name}</div>
                      </div>
                      <span style={{ padding: "2px 6px", borderRadius: 10, fontSize: "10px", fontWeight: 600, background: "rgba(63,185,80,0.1)", color: "var(--accent-green)" }}>Running</span>
                    </div>
                  );
                })
              ) : (
                <div style={{ padding: "16px 14px", fontSize: "var(--text-xs)", color: "var(--text-muted)", textAlign: "center" }}>
                  No Kind clusters.
                </div>
              )}
            </div>

            {/* Right: Detail Panel */}
            <div style={{ padding: 24, overflowY: "auto", background: "var(--bg-secondary)", borderRadius: "0 12px 12px 0" }}>
              {!selected ? (
                <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--text-muted)" }}>
                  <ColimaIcon /><div style={{ marginTop: 12, fontSize: "var(--text-sm)" }}>Select an instance to view details</div>
                </div>
              ) : selected.type === "colima" ? (
                <ColimaDetail inst={selected.data} actionLoading={actionLoading} onAction={handleAction} onRefresh={onRefresh} />
              ) : (
                <KindDetail name={selected.name} onDelete={handleDeleteKind} deleting={actionLoading === `kind-${selected.name}-delete`} />
              )}
            </div>
          </div>
        )}
      </div>

      {showCreate && <CreateInstanceDialog onClose={() => setShowCreate(false)} onCreated={onRefresh} />}
      {showCreateKind && <CreateKindDialog onClose={() => setShowCreateKind(false)} onCreated={() => { fetchKind(); }} />}
      {confirm && <ConfirmDialog title={confirm.title} message={confirm.message} confirmLabel={confirm.confirmLabel} danger={confirm.danger} onConfirm={confirm.onConfirm} onCancel={() => setConfirm(null)} />}
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={ctxMenu.items} onClose={() => setCtxMenu(null)} />}
    </>
  );
}

/* ===== Colima Detail Panel ===== */
function ColimaDetail({ inst, actionLoading, onAction, onRefresh }: { inst: ColimaInstance; actionLoading: string | null; onAction: (profile: string, action: "start" | "stop" | "restart" | "delete") => void; onRefresh: () => void }) {
  const profileId = inst.name === "colima" ? "default" : inst.name.replace("colima-", "");
  const isRunning = inst.status === "Running";
  const isLoading = actionLoading?.startsWith(profileId);
  // Use module-level pendingOps so state persists across tab switches
  const [k8sLoading, setK8sLoading] = useState<string | null>(pendingOps.get(`k8s-${profileId}`) || null);
  const [k8sNotice, setK8sNotice] = useState<{ type: "success" | "error"; text: string } | null>(null);

  const handleK8sAction = (action: "start" | "stop" | "delete" | "reset") => {
    const labels: Record<string, string> = { start: "Enabling", stop: "Stopping", delete: "Removing", reset: "Resetting" };
    const pastLabels: Record<string, string> = { start: "enabled", stop: "stopped", delete: "removed", reset: "reset" };
    
    // Track pending op at module level so it survives unmount
    setK8sLoading(action);
    pendingOps.set(`k8s-${profileId}`, action);
    globalToast("success", `${labels[action]} Kubernetes (K3s)... This may take a minute.`);
    
    colimaApi.kubernetesAction(profileId, action)
      .then(() => {
        setK8sNotice({ type: "success", text: `Kubernetes ${pastLabels[action]} successfully` });
        globalToast("success", `Kubernetes ${pastLabels[action]} successfully`);
        onRefresh();
        setTimeout(() => setK8sNotice(null), 4000);
      })
      .catch((e) => {
        setK8sNotice({ type: "error", text: String(e) });
        globalToast("error", `K3s ${action} failed: ${e}`);
        setTimeout(() => setK8sNotice(null), 6000);
      })
      .finally(() => { setK8sLoading(null); pendingOps.delete(`k8s-${profileId}`); });
  };

  return (
    <div>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 24 }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 6 }}>
            <div style={{ width: 12, height: 12, borderRadius: "50%", background: isRunning ? "var(--accent-green)" : "var(--text-muted)", boxShadow: isRunning ? "0 0 8px var(--accent-green)" : "none" }} />
            <h2 style={{ margin: 0, fontSize: "var(--text-xl)", fontWeight: 700 }}>{inst.name}</h2>
            <span className={`badge badge-${isRunning ? "running" : "stopped"}`}><span className="badge-dot" />{inst.status}</span>
          </div>
          <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginLeft: 22 }}>
            Profile: <span style={{ fontFamily: "var(--font-mono)", color: "var(--accent-blue)" }}>{profileId}</span>
            {inst.address ? <> · Address: <span style={{ fontFamily: "var(--font-mono)" }}>{inst.address}</span></> : null}
          </div>
        </div>
        {/* Actions */}
        <div style={{ display: "flex", gap: 6 }}>
          {isRunning ? (
            <>
              <button className="btn btn-ghost" disabled={!!isLoading} onClick={() => onAction(profileId, "stop")} style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }}>
                {actionLoading === `${profileId}-stop` ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>} Stop
              </button>
              <button className="btn btn-ghost" disabled={!!isLoading} onClick={() => onAction(profileId, "restart")} style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }}>
                {actionLoading === `${profileId}-restart` ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>} Restart
              </button>
            </>
          ) : (
            <button className="btn btn-primary" disabled={!!isLoading} onClick={() => onAction(profileId, "start")} style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }}>
              {actionLoading === `${profileId}-start` ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><polygon points="6,4 20,12 6,20"/></svg>} Start
            </button>
          )}
          <button className="btn btn-ghost" disabled={!!isLoading} onClick={() => onAction(profileId, "delete")} style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)", display: "flex", alignItems: "center", gap: 4 }}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg> Delete
          </button>
        </div>
      </div>

      {/* Resource Stats */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 12, marginBottom: 24 }}>
        {[
          { label: "CPUs", value: String(inst.cpus), color: "var(--accent-blue)" },
          { label: "Memory", value: formatBytes(inst.memory), color: "var(--accent-green)" },
          { label: "Disk", value: formatBytes(inst.disk), color: "var(--accent-orange)" },
        ].map(s => (
          <div key={s.label} style={{ padding: "14px 16px", borderRadius: 10, background: "var(--bg-primary)", borderLeft: `3px solid ${s.color}` }}>
            <div style={{ fontSize: "10px", color: "var(--text-muted)", fontWeight: 600, marginBottom: 4, textTransform: "uppercase", letterSpacing: "0.04em" }}>{s.label}</div>
            <div style={{ fontSize: "var(--text-lg)", fontWeight: 700, fontFamily: "var(--font-mono)", color: s.color }}>{s.value}</div>
          </div>
        ))}
      </div>

      {/* Configuration */}
      <div style={{ marginBottom: 24 }}>
        <h3 style={{ fontSize: "var(--text-sm)", fontWeight: 600, marginBottom: 12, color: "var(--text-primary)" }}>Configuration</h3>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
          {[
            { label: "Runtime", value: inst.runtime },
            { label: "Architecture", value: inst.arch },
            { label: "Kubernetes", value: inst.kubernetes ? "K3s Enabled" : "Disabled" },
            { label: "Network Address", value: inst.address || "None" },
          ].map(r => (
            <div key={r.label} style={{ padding: "10px 14px", background: "var(--bg-primary)", borderRadius: 8 }}>
              <div style={{ fontSize: "10px", color: "var(--text-muted)", fontWeight: 500, marginBottom: 2, textTransform: "uppercase", letterSpacing: "0.04em" }}>{r.label}</div>
              <div style={{ fontSize: "var(--text-sm)", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{r.value}</div>
            </div>
          ))}
        </div>
      </div>

      {/* Kubernetes (K3s) Management */}
      {isRunning && (
        <div style={{ marginBottom: 24 }}>
          <h3 style={{ fontSize: "var(--text-sm)", fontWeight: 600, marginBottom: 12, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: 8 }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-purple)" strokeWidth="1.5"><path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" strokeLinejoin="round"/><circle cx="12" cy="12" r="3"/></svg>
            Kubernetes (K3s)
          </h3>

          {k8sNotice && (
            <div style={{
              padding: "8px 12px", borderRadius: 8, marginBottom: 12, fontSize: "var(--text-xs)", fontWeight: 500,
              background: k8sNotice.type === "success" ? "rgba(63,185,80,0.08)" : "rgba(248,81,73,0.08)",
              border: `1px solid ${k8sNotice.type === "success" ? "rgba(63,185,80,0.25)" : "rgba(248,81,73,0.25)"}`,
              color: k8sNotice.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
            }}>
              {k8sNotice.text}
            </div>
          )}

          <div style={{
            padding: "14px 16px", borderRadius: 10,
            background: inst.kubernetes ? "rgba(167,139,250,0.04)" : "var(--bg-primary)",
            border: `1px solid ${inst.kubernetes ? "rgba(167,139,250,0.15)" : "var(--border-primary)"}`,
          }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <div style={{
                  width: 8, height: 8, borderRadius: "50%",
                  background: inst.kubernetes ? "var(--accent-green)" : "var(--text-muted)",
                  boxShadow: inst.kubernetes ? "0 0 6px var(--accent-green)" : "none",
                }} />
                <span style={{ fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--text-primary)" }}>
                  {inst.kubernetes ? "K3s Active" : "K3s Disabled"}
                </span>
                {inst.kubernetes && (
                  <span style={{ padding: "2px 8px", borderRadius: 10, fontSize: "10px", fontWeight: 600, background: "rgba(63,185,80,0.1)", color: "var(--accent-green)" }}>Running</span>
                )}
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                {inst.kubernetes ? (
                  <>
                    <button className="btn btn-ghost" disabled={!!k8sLoading} onClick={() => handleK8sAction("stop")}
                      style={{ fontSize: "var(--text-xs)", padding: "4px 10px", display: "flex", alignItems: "center", gap: 4 }}>
                      {k8sLoading === "stop" ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>} Stop
                    </button>
                    <button className="btn btn-ghost" disabled={!!k8sLoading} onClick={() => handleK8sAction("reset")}
                      style={{ fontSize: "var(--text-xs)", padding: "4px 10px", display: "flex", alignItems: "center", gap: 4 }}
                      title="Reset K3s cluster (recreates all resources)">
                      {k8sLoading === "reset" ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>} Reset
                    </button>
                    <button className="btn btn-ghost" disabled={!!k8sLoading} onClick={() => handleK8sAction("delete")}
                      style={{ fontSize: "var(--text-xs)", padding: "4px 10px", color: "var(--accent-red)", display: "flex", alignItems: "center", gap: 4 }}
                      title="Remove K3s from this instance">
                      {k8sLoading === "delete" ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>} Remove
                    </button>
                  </>
                ) : (
                  <button className="btn btn-primary" disabled={!!k8sLoading} onClick={() => handleK8sAction("start")}
                    style={{ fontSize: "var(--text-xs)", padding: "4px 12px", display: "flex", alignItems: "center", gap: 4, background: "linear-gradient(135deg, #a78bfa, #7c3aed)", border: "none" }}>
                    {k8sLoading === "start" ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor"><polygon points="6,4 20,12 6,20"/></svg>} Enable K3s
                  </button>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tags */}
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <span style={{ padding: "4px 12px", borderRadius: 20, fontSize: "11px", fontWeight: 500, background: "rgba(88,166,255,0.1)", color: "var(--accent-blue)", border: "1px solid rgba(88,166,255,0.2)" }}>{inst.runtime}</span>
        {inst.kubernetes && <span style={{ padding: "4px 12px", borderRadius: 20, fontSize: "11px", fontWeight: 500, background: "rgba(167,139,250,0.1)", color: "var(--accent-purple)", border: "1px solid rgba(167,139,250,0.2)", display: "inline-flex", alignItems: "center", gap: 4 }}><CheckIcon size={10} /> K3s</span>}
        <span style={{ padding: "4px 12px", borderRadius: 20, fontSize: "11px", fontWeight: 500, background: "rgba(139,148,158,0.08)", color: "var(--text-muted)", border: "1px solid rgba(139,148,158,0.15)" }}>{inst.arch}</span>
      </div>
    </div>
  );
}

/* ===== Kind Detail Panel ===== */
function KindDetail({ name, onDelete, deleting }: { name: string; onDelete: (name: string) => void; deleting: boolean }) {
  return (
    <div>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 24 }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 6 }}>
            <KindIcon />
            <h2 style={{ margin: 0, fontSize: "var(--text-xl)", fontWeight: 700, fontFamily: "var(--font-mono)" }}>{name}</h2>
            <span style={{ padding: "3px 10px", borderRadius: 20, fontSize: "11px", fontWeight: 600, background: "rgba(63,185,80,0.1)", color: "var(--accent-green)", border: "1px solid rgba(63,185,80,0.2)", display: "inline-flex", alignItems: "center", gap: 4 }}>
              <StatusDot size={6} color="var(--accent-green)" style={{ display: "inline-block" }} /> Running
            </span>
          </div>
          <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Kubernetes in Docker — local multi-node cluster</div>
        </div>
        <button className="btn btn-ghost" onClick={() => onDelete(name)} disabled={deleting}
          style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)", display: "flex", alignItems: "center", gap: 4 }}>
          {deleting ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>} Delete
        </button>
      </div>

      {/* Info grid */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginBottom: 24 }}>
        {[
          { label: "Cluster Name", value: name },
          { label: "kubectl Context", value: `kind-${name}` },
          { label: "Provider", value: "Docker" },
          { label: "Type", value: "Kind (Kubernetes in Docker)" },
        ].map(r => (
          <div key={r.label} style={{ padding: "12px 16px", background: "var(--bg-primary)", borderRadius: 10 }}>
            <div style={{ fontSize: "10px", color: "var(--text-muted)", fontWeight: 600, marginBottom: 4, textTransform: "uppercase", letterSpacing: "0.04em" }}>{r.label}</div>
            <div style={{ fontSize: "var(--text-sm)", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{r.value}</div>
          </div>
        ))}
      </div>

      {/* Quick tips */}
      <div style={{ padding: "14px 16px", borderRadius: 10, background: "rgba(167,139,250,0.04)", border: "1px solid rgba(167,139,250,0.12)" }}>
        <div style={{ fontSize: "var(--text-xs)", fontWeight: 600, color: "var(--accent-purple)", marginBottom: 8 }}>Quick Commands</div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {[`kubectl cluster-info --context kind-${name}`, `kubectl get nodes --context kind-${name}`, `kubectl get pods -A --context kind-${name}`].map(cmd => (
            <code key={cmd} style={{ fontSize: "11px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", padding: "4px 8px", background: "var(--bg-primary)", borderRadius: 4, display: "block" }}>{cmd}</code>
          ))}
        </div>
      </div>
    </div>
  );
}

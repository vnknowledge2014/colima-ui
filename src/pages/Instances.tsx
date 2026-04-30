import { useState, useCallback } from "react";
import { colimaApi, ColimaInstance, StartConfig } from "../lib/api";

interface InstancesProps {
  instances: ColimaInstance[];
  onRefresh: () => void;
}

const formatBytes = (bytes: number): string => {
  if (bytes >= 1073741824) return `${Math.round(bytes / 1073741824)} GiB`;
  if (bytes >= 1048576) return `${Math.round(bytes / 1048576)} MiB`;
  return `${bytes} B`;
};

/* ===== Confirmation Dialog ===== */
function ConfirmDialog({
  title,
  message,
  confirmLabel,
  danger,
  onConfirm,
  onCancel,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: "min(400px, 90vw)" }}>
        <div className="modal-header">
          <h2 className="modal-title">{title}</h2>
        </div>
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
    profile: "default",
    runtime: "docker",
    cpus: 2,
    memory: 2,
    disk: 60,
    vm_type: "vz",
    kubernetes: false,
    kubernetes_version: "",
    arch: "",
    mount_type: "",
    mounts: [],
    dns: [],
    network_address: false,
  });
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async () => {
    if (!config.profile.trim()) {
      setError("Profile name is required");
      return;
    }
    setCreating(true);
    setError(null);
    try {
      await colimaApi.startInstance(config);
      onCreated();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2 className="modal-title">Create Instance</h2>
          <button className="btn btn-icon btn-ghost" onClick={onClose}>✕</button>
        </div>

        {error && (
          <div style={{ padding: "8px 12px", background: "rgba(248, 81, 73, 0.1)", borderRadius: "var(--radius-md)", color: "var(--accent-red)", fontSize: "var(--text-sm)", marginBottom: 16 }}>
            {error}
          </div>
        )}

        <div className="form-group">
          <label className="form-label">Profile Name</label>
          <input className="input" value={config.profile} onChange={(e) => setConfig({ ...config, profile: e.target.value })} placeholder="default" />
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
          <div className="form-group">
            <label className="form-label">Runtime</label>
            <select className="input select" value={config.runtime} onChange={(e) => setConfig({ ...config, runtime: e.target.value })}>
              <option value="docker">Docker</option>
              <option value="containerd">Containerd</option>
              <option value="incus">Incus</option>
            </select>
          </div>
          <div className="form-group">
            <label className="form-label">VM Type</label>
            <select className="input select" value={config.vm_type} onChange={(e) => setConfig({ ...config, vm_type: e.target.value })}>
              <option value="vz">VZ (macOS 13+)</option>
              <option value="qemu">QEMU</option>
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
            <option value="">Default (host)</option>
            <option value="aarch64">aarch64 (ARM64)</option>
            <option value="x86_64">x86_64 (Intel)</option>
          </select>
        </div>

        <div className="form-group">
          <label className="form-label">Mount Type</label>
          <select className="input select" value={config.mount_type} onChange={(e) => setConfig({ ...config, mount_type: e.target.value })}>
            <option value="">Default</option>
            <option value="virtiofs">VirtioFS (macOS)</option>
            <option value="sshfs">SSHFS</option>
            <option value="9p">9P</option>
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
            {creating ? (
              <><div className="spinner" style={{ width: 14, height: 14 }} /> Creating...</>
            ) : (
              "Create & Start"
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

/* ===== Main Instances Page ===== */
export default function Instances({ instances, onRefresh }: InstancesProps) {
  const [showCreate, setShowCreate] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{
    title: string;
    message: string;
    confirmLabel: string;
    danger: boolean;
    onConfirm: () => void;
  } | null>(null);
  const [notification, setNotification] = useState<{ type: "success" | "error"; text: string } | null>(null);

  const showNotification = useCallback((type: "success" | "error", text: string) => {
    setNotification({ type, text });
    setTimeout(() => setNotification(null), 4000);
  }, []);

  const getProfileId = (instanceName: string) => {
    if (instanceName === "colima") return "default";
    return instanceName.replace("colima-", "");
  };

  const handleStart = async (profile: string) => {
    setActionLoading(`${profile}-start`);
    try {
      await colimaApi.startInstance({
        profile,
        runtime: "docker",
        cpus: 2,
        memory: 2,
        disk: 60,
        vm_type: "",
        kubernetes: false,
        kubernetes_version: "",
        arch: "",
        mount_type: "",
        mounts: [],
        dns: [],
        network_address: false,
      });
      showNotification("success", `Instance '${profile}' started`);
      onRefresh();
    } catch (e) {
      showNotification("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleStop = async (profile: string) => {
    setActionLoading(`${profile}-stop`);
    try {
      await colimaApi.stopInstance(profile);
      showNotification("success", `Instance '${profile}' stopped`);
      onRefresh();
    } catch (e) {
      showNotification("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleRestart = async (profile: string) => {
    setActionLoading(`${profile}-restart`);
    try {
      await colimaApi.stopInstance(profile);
      await colimaApi.startInstance({
        profile,
        runtime: "docker",
        cpus: 2,
        memory: 2,
        disk: 60,
        vm_type: "",
        kubernetes: false,
        kubernetes_version: "",
        arch: "",
        mount_type: "",
        mounts: [],
        dns: [],
        network_address: false,
      });
      showNotification("success", `Instance '${profile}' restarted`);
      onRefresh();
    } catch (e) {
      showNotification("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleDelete = (profile: string) => {
    setConfirm({
      title: "Delete Instance",
      message: `Are you sure you want to delete instance "${profile}"? This action cannot be undone and all data in the VM will be lost.`,
      confirmLabel: "Delete",
      danger: true,
      onConfirm: async () => {
        setConfirm(null);
        setActionLoading(`${profile}-delete`);
        try {
          await colimaApi.deleteInstance(profile, true);
          showNotification("success", `Instance '${profile}' deleted`);
          onRefresh();
        } catch (e) {
          showNotification("error", String(e));
        } finally {
          setActionLoading(null);
        }
      },
    });
  };

  return (
    <>
      <div className="content-header">
        <h1>
          Instances
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {instances.filter((i) => i.status === "Running").length} running · {instances.length} total
          </span>
        </h1>
        <div className="content-header-actions">
          <button className="btn btn-ghost" onClick={onRefresh}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
            Refresh
          </button>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
            </svg>
            New Instance
          </button>
        </div>
      </div>

      <div className="content-body">
        {/* Toast Notification */}
        {notification && (
          <div
            style={{
              position: "fixed",
              top: 16,
              right: 16,
              padding: "10px 16px",
              borderRadius: "var(--radius-md)",
              background: notification.type === "success" ? "rgba(63, 185, 80, 0.15)" : "rgba(248, 81, 73, 0.15)",
              border: `1px solid ${notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)"}`,
              color: notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
              fontSize: "var(--text-sm)",
              zIndex: 200,
              animation: "slideUp 250ms ease",
              boxShadow: "var(--shadow-lg)",
            }}
          >
            {notification.type === "success" ? "✓" : "✕"} {notification.text}
          </div>
        )}

        {instances.length > 0 ? (
          <table className="data-table">
            <thead>
              <tr>
                <th>Profile</th>
                <th>Status</th>
                <th>Arch</th>
                <th>CPUs</th>
                <th>Memory</th>
                <th>Disk</th>
                <th>Runtime</th>
                <th>K8s</th>
                <th>Address</th>
                <th style={{ textAlign: "right" }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {instances.map((inst) => {
                const isRunning = inst.status === "Running";
                const profileId = getProfileId(inst.name);
                const isLoading = actionLoading?.startsWith(profileId);

                return (
                  <tr key={inst.name} style={{ opacity: isLoading ? 0.6 : 1, transition: "opacity 200ms" }}>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <div style={{
                          width: 8, height: 8, borderRadius: "50%",
                          background: isRunning ? "var(--status-running)" : "var(--status-stopped)",
                          boxShadow: isRunning ? "0 0 6px var(--status-running)" : "none",
                          flexShrink: 0,
                        }}/>
                        <span style={{ fontWeight: 600 }}>{inst.name}</span>
                      </div>
                    </td>
                    <td>
                      <span className={`badge badge-${isRunning ? "running" : "stopped"}`}>
                        <span className="badge-dot" />
                        {inst.status}
                      </span>
                    </td>
                    <td style={{ color: "var(--text-secondary)" }}>{inst.arch}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{inst.cpus}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{formatBytes(inst.memory)}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{formatBytes(inst.disk)}</td>
                    <td>
                      <span style={{
                        padding: "2px 8px",
                        borderRadius: "var(--radius-sm)",
                        background: "rgba(88, 166, 255, 0.1)",
                        color: "var(--accent-blue)",
                        fontSize: "var(--text-xs)",
                        fontWeight: 500,
                      }}>
                        {inst.runtime}
                      </span>
                    </td>
                    <td>
                      {inst.kubernetes ? (
                        <span style={{ color: "var(--accent-purple)", fontSize: "var(--text-xs)", fontWeight: 500 }}>✓ K3s</span>
                      ) : (
                        <span style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>—</span>
                      )}
                    </td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                      {inst.address || "—"}
                    </td>
                    <td>
                      <div className="table-actions" style={{ justifyContent: "flex-end" }}>
                        {isRunning ? (
                          <>
                            <button
                              className="btn btn-ghost btn-icon"
                              data-tooltip="Stop"
                              disabled={!!isLoading}
                              onClick={() => handleStop(profileId)}
                            >
                              {actionLoading === `${profileId}-stop` ? <div className="spinner" style={{ width: 14, height: 14 }} /> : (
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>
                              )}
                            </button>
                            <button
                              className="btn btn-ghost btn-icon"
                              data-tooltip="Restart"
                              disabled={!!isLoading}
                              onClick={() => handleRestart(profileId)}
                            >
                              {actionLoading === `${profileId}-restart` ? <div className="spinner" style={{ width: 14, height: 14 }} /> : (
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                  <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
                                </svg>
                              )}
                            </button>
                          </>
                        ) : (
                          <button
                            className="btn btn-ghost btn-icon"
                            data-tooltip="Start"
                            disabled={!!isLoading}
                            onClick={() => handleStart(profileId)}
                          >
                            {actionLoading === `${profileId}-start` ? <div className="spinner" style={{ width: 14, height: 14 }} /> : (
                              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="6,4 20,12 6,20"/></svg>
                            )}
                          </button>
                        )}
                        <button
                          className="btn btn-ghost btn-icon"
                          data-tooltip="Delete"
                          disabled={!!isLoading}
                          onClick={() => handleDelete(profileId)}
                          style={{ color: "var(--accent-red)" }}
                        >
                          {actionLoading === `${profileId}-delete` ? <div className="spinner" style={{ width: 14, height: 14 }} /> : (
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                            </svg>
                          )}
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/>
              </svg>
            </div>
            <div className="empty-state-title">No instances</div>
            <div className="empty-state-text">
              Click "New Instance" to create your first Colima VM.
            </div>
            <button className="btn btn-primary" onClick={() => setShowCreate(true)}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
              </svg>
              New Instance
            </button>
          </div>
        )}
      </div>

      {showCreate && (
        <CreateInstanceDialog onClose={() => setShowCreate(false)} onCreated={onRefresh} />
      )}

      {confirm && (
        <ConfirmDialog
          title={confirm.title}
          message={confirm.message}
          confirmLabel={confirm.confirmLabel}
          danger={confirm.danger}
          onConfirm={confirm.onConfirm}
          onCancel={() => setConfirm(null)}
        />
      )}
    </>
  );
}

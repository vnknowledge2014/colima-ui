import { useState, useEffect, useCallback } from "react";
import { limaApi, LimaInstance } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { StopIcon, PlayIcon, TrashIcon, CloseIcon, CheckIcon, ErrorIcon, WarningIcon, StatusDot } from "../components/Icons";

export default function LinuxVMs() {
  const [vms, setVMs] = useState<LimaInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [selectedVM, setSelectedVM] = useState<LimaInstance | null>(null);
  const [shellCmd, setShellCmd] = useState("");
  const [shellOutput, setShellOutput] = useState("");
  const [shellCwd, setShellCwd] = useState("~");
  const { confirm, ConfirmDialogProps } = useConfirm();

  // Create VM state
  const [showCreate, setShowCreate] = useState(false);
  const [templates, setTemplates] = useState<string[]>([]);
  const [newVM, setNewVM] = useState({ name: "", cpus: 2, memory: 2, disk: 60, template: "" });

  const fetchVMs = useCallback(async () => {
    try {
      setError(null);
      const list = await limaApi.list();
      setVMs(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchVMs();
    const interval = setInterval(fetchVMs, 8000);
    return () => clearInterval(interval);
  }, [fetchVMs]);

  // Load templates when create modal opens
  useEffect(() => {
    if (showCreate && templates.length === 0) {
      limaApi.templates().then(raw => {
        try {
          const lines = raw.split("\n").map(l => l.trim()).filter(l => l && !l.startsWith("-"));
          setTemplates(lines.length > 0 ? lines : ["default", "docker", "ubuntu", "fedora", "alpine", "debian"]);
        } catch {
          setTemplates(["default", "docker", "ubuntu", "fedora", "alpine", "debian"]);
        }
      }).catch(() => {
        setTemplates(["default", "docker", "ubuntu", "fedora", "alpine", "debian"]);
      });
    }
  }, [showCreate, templates.length]);

  const handleCreate = async () => {
    if (!newVM.name.trim()) return;
    const name = newVM.name.trim().toLowerCase();
    // Fire-and-forget: close dialog immediately
    globalToast("success", `Creating VM '${name}'... This may take a few minutes.`);
    setShowCreate(false);
    setNewVM({ name: "", cpus: 2, memory: 2, disk: 60, template: "" });
    limaApi.create({
      name,
      cpus: newVM.cpus,
      memory: newVM.memory,
      disk: newVM.disk,
      template: newVM.template || undefined,
    })
      .then(() => { globalToast("success", `VM '${name}' created successfully`); setTimeout(fetchVMs, 2000); })
      .catch((e) => globalToast("error", `Failed to create VM: ${e}`));
  };

  const handleAction = async (name: string, action: "start" | "stop" | "delete") => {
    setActionLoading(`${name}-${action}`);
    try {
      if (action === "start") {
        globalToast("success", `Starting VM '${name}'...`);
        limaApi.start(name)
          .then(() => { globalToast("success", `VM '${name}' started`); setTimeout(fetchVMs, 1000); })
          .catch((e) => globalToast("error", String(e)))
          .finally(() => setActionLoading(null));
        return;
      } else if (action === "stop") {
        await limaApi.stop(name);
        globalToast("success", `VM '${name}' stopped`);
      } else {
        const ok = await confirm({ title: "Delete VM", message: `Delete VM '${name}'? This cannot be undone.`, confirmText: "Delete", variant: "danger" });
        if (!ok) {
          setActionLoading(null);
          return;
        }
        await limaApi.delete(name, true);
        globalToast("success", `VM '${name}' deleted`);
      }
      setTimeout(fetchVMs, 1000);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const runShell = async () => {
    if (!selectedVM || !shellCmd.trim()) return;
    const cmd = shellCmd.trim();

    // Handle cd commands locally by tracking CWD
    if (cmd === "cd" || cmd === "cd ~" || cmd === "cd ~/") {
      setShellOutput(prev => prev + `${shellCwd}$ ${cmd}\n`);
      setShellCwd("~");
      setShellCmd("");
      return;
    }
    if (cmd.startsWith("cd ")) {
      const target = cmd.slice(3).trim();
      // Resolve the new path using shell and pwd
      try {
        const cwdCmd = shellCwd === "~" ? `cd ~ && cd ${target} && pwd` : `cd ${shellCwd} && cd ${target} && pwd`;
        const newPath = await limaApi.shell(selectedVM.name, cwdCmd);
        const resolved = newPath.trim();
        if (resolved) {
          setShellOutput(prev => prev + `${shellCwd}$ ${cmd}\n`);
          setShellCwd(resolved);
        } else {
          setShellOutput(prev => prev + `${shellCwd}$ ${cmd}\ncd: no such directory: ${target}\n`);
        }
      } catch (e) {
        setShellOutput(prev => prev + `${shellCwd}$ ${cmd}\n${e}\n`);
      }
      setShellCmd("");
      return;
    }

    // For all other commands, prepend cd to tracked CWD
    const fullCmd = shellCwd === "~" ? `cd ~ && ${cmd}` : `cd ${shellCwd} && ${cmd}`;
    try {
      const output = await limaApi.shell(selectedVM.name, fullCmd);
      setShellOutput(prev => prev + `${shellCwd}$ ${cmd}\n${output}\n`);
      setShellCmd("");
    } catch (e) {
      setShellOutput(prev => prev + `${shellCwd}$ ${cmd}\nError: ${e}\n`);
      setShellCmd("");
    }
  };

  const statusColor = (status: string) => {
    if (status === "Running") return "var(--accent-green)";
    if (status === "Stopped") return "var(--accent-red)";
    return "var(--text-muted)";
  };

  if (loading) {
    return (
      <>
        <div className="content-header"><h1>Linux VMs</h1></div>
        <div className="loading-screen"><div className="spinner" /><span>Loading VMs...</span></div>
      </>
    );
  }

  return (
    <>
      <div className="content-header">
        <h1>
          Linux VMs (Lima)
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {vms.length} VM{vms.length !== 1 ? "s" : ""}
          </span>
        </h1>
        <div className="content-header-actions" style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-ghost" onClick={fetchVMs}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>
            + New VM
          </button>
        </div>
      </div>

      <div className="content-body">
        {notification && (
          <div style={{
            position: "fixed", top: 16, right: 16, padding: "12px 20px",
            borderRadius: "var(--radius-md)",
            background: notification.type === "success" ? "#1a2e1a" : "#2e1a1a",
            border: `1px solid ${notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)"}`,
            color: notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
            fontSize: "var(--text-sm)", fontWeight: 500, zIndex: 9999, boxShadow: "0 8px 32px rgba(0,0,0,0.6)",
            backdropFilter: "blur(12px)", display: "flex", alignItems: "center", gap: 8, maxWidth: 420,
          }}>
            {notification.type === "success" ? <CheckIcon size={14} /> : <ErrorIcon size={14} />} {notification.text}
          </div>
        )}

        {error && (
          <div className="card" style={{ borderColor: "var(--accent-yellow)", marginBottom: 16 }}>
            <p style={{ color: "var(--accent-yellow)", fontSize: "var(--text-sm)", display: "flex", alignItems: "center", gap: 6 }}><WarningIcon size={14} /> {error}</p>
          </div>
        )}

        {vms.length > 0 ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {vms.map(vm => {
              const isLoading = actionLoading?.startsWith(vm.name);
              const isRunning = vm.status === "Running";
              return (
                <div key={vm.name} onClick={() => { setSelectedVM(vm); setShellOutput(""); setShellCwd("~"); }} style={{
                  padding: 16, background: "var(--bg-secondary)", borderRadius: 12,
                  border: "1px solid var(--border-primary)", cursor: "pointer",
                  opacity: isLoading ? 0.6 : 1, transition: "all 200ms",
                }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={isRunning ? "var(--accent-green)" : "var(--text-muted)"} strokeWidth="2">
                          <rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
                        </svg>
                        <span style={{ fontWeight: 600, fontSize: "var(--text-md)" }}>{vm.name}</span>
                        <span style={{ color: statusColor(vm.status), fontWeight: 500, fontSize: "var(--text-xs)" }}>
                          <StatusDot size={8} color={statusColor(vm.status)} style={{ display: "inline-block", verticalAlign: "middle" }} /> {vm.status}
                        </span>
                      </div>
                      <div style={{ display: "flex", gap: 16, marginTop: 4, fontSize: "var(--text-xs)", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                        <span>{vm.arch}</span>
                        <span>{vm.cpus} CPU</span>
                        <span>{vm.memory}</span>
                        <span>{vm.disk}</span>
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 6 }} onClick={e => e.stopPropagation()}>
                      {isRunning ? (
                        <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)" }}
                          disabled={!!isLoading} onClick={() => handleAction(vm.name, "stop")}><StopIcon size={12} /> Stop</button>
                      ) : (
                        <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-green)" }}
                          disabled={!!isLoading} onClick={() => handleAction(vm.name, "start")}><PlayIcon size={12} /> Start</button>
                      )}
                      <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)" }}
                        disabled={!!isLoading} onClick={() => handleAction(vm.name, "delete")}><TrashIcon size={12} /></button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
              </svg>
            </div>
            <div className="empty-state-title">No Linux VMs</div>
            <div className="empty-state-text">Create a Linux VM to get started.</div>
            <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ New VM</button>
          </div>
        )}
      </div>

      {/* Shell Modal */}
      {selectedVM && (
        <div className="modal-overlay" onClick={() => setSelectedVM(null)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ width: "min(800px, 95vw)", maxHeight: "80vh" }}>
            <div className="modal-header">
              <h2 className="modal-title">
                {selectedVM.name}
                <span style={{ color: statusColor(selectedVM.status), fontSize: "var(--text-sm)", marginLeft: 8 }}>
                  <StatusDot size={8} color={statusColor(selectedVM.status)} style={{ display: "inline-block", verticalAlign: "middle" }} /> {selectedVM.status}
                </span>
              </h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setSelectedVM(null)}><CloseIcon size={16} /></button>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 8, marginBottom: 16 }}>
              <div style={{ padding: 10, background: "var(--bg-primary)", borderRadius: 8, textAlign: "center" }}>
                <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Arch</div>
                <div style={{ fontWeight: 600, fontFamily: "var(--font-mono)" }}>{selectedVM.arch}</div>
              </div>
              <div style={{ padding: 10, background: "var(--bg-primary)", borderRadius: 8, textAlign: "center" }}>
                <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>CPUs</div>
                <div style={{ fontWeight: 600, fontFamily: "var(--font-mono)" }}>{selectedVM.cpus}</div>
              </div>
              <div style={{ padding: 10, background: "var(--bg-primary)", borderRadius: 8, textAlign: "center" }}>
                <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Memory</div>
                <div style={{ fontWeight: 600, fontFamily: "var(--font-mono)" }}>{selectedVM.memory}</div>
              </div>
              <div style={{ padding: 10, background: "var(--bg-primary)", borderRadius: 8, textAlign: "center" }}>
                <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>Disk</div>
                <div style={{ fontWeight: 600, fontFamily: "var(--font-mono)" }}>{selectedVM.disk}</div>
              </div>
            </div>

            {selectedVM.status === "Running" && (
              <>
                <h3 style={{ fontSize: "var(--text-sm)", fontWeight: 600, marginBottom: 8 }}>Shell</h3>
                <div style={{
                  background: "var(--bg-primary)", borderRadius: 8, padding: 12, marginBottom: 12,
                  fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)",
                  minHeight: 120, maxHeight: 300, overflow: "auto", whiteSpace: "pre-wrap",
                  color: "var(--text-secondary)",
                }}>
                  {shellOutput || "Run commands inside the VM..."}
                </div>
                <div style={{ display: "flex", gap: 8 }}>
                  <input type="text" value={shellCmd} onChange={e => setShellCmd(e.target.value)}
                    onKeyDown={e => e.key === "Enter" && runShell()}
                    placeholder={`${shellCwd}$ Enter command...`}
                    style={{
                      flex: 1, padding: "8px 12px", background: "var(--bg-primary)",
                      border: "1px solid var(--border-primary)", borderRadius: 6,
                      color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: "var(--text-sm)",
                    }} />
                  <button className="btn btn-primary" onClick={runShell}>Run</button>
                </div>
              </>
            )}

            <div className="modal-footer">
              <button className="btn btn-primary" onClick={() => setSelectedVM(null)}>Close</button>
            </div>
          </div>
        </div>
      )}
      <ConfirmDialog {...ConfirmDialogProps} />

      {/* Create VM Modal */}
      {showCreate && (
        <div className="modal-overlay" onClick={() => setShowCreate(false)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ width: "min(560px, 95vw)" }}>
            <div className="modal-header">
              <h2 className="modal-title">Create VM</h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setShowCreate(false)}><CloseIcon size={16} /></button>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              <div>
                <label className="form-label">VM Name</label>
                <input type="text" value={newVM.name} onChange={e => setNewVM({ ...newVM, name: e.target.value })}
                  placeholder="my-vm"
                  style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)", fontSize: "var(--text-sm)" }} />
              </div>

              <div>
                <label className="form-label">Template</label>
                <select value={newVM.template} onChange={e => setNewVM({ ...newVM, template: e.target.value })}
                  style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)", fontSize: "var(--text-sm)" }}>
                  <option value="">Default (Ubuntu)</option>
                  {templates.map(t => <option key={t} value={t}>{t}</option>)}
                </select>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 12 }}>
                <div>
                  <label className="form-label">CPUs</label>
                  <input type="number" min={1} max={16} value={newVM.cpus}
                    onChange={e => setNewVM({ ...newVM, cpus: parseInt(e.target.value) || 1 })}
                    style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)", fontSize: "var(--text-sm)" }} />
                </div>
                <div>
                  <label className="form-label">Memory (GiB)</label>
                  <input type="number" min={1} max={64} value={newVM.memory}
                    onChange={e => setNewVM({ ...newVM, memory: parseInt(e.target.value) || 1 })}
                    style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)", fontSize: "var(--text-sm)" }} />
                </div>
                <div>
                  <label className="form-label">Disk (GiB)</label>
                  <input type="number" min={10} max={500} value={newVM.disk}
                    onChange={e => setNewVM({ ...newVM, disk: parseInt(e.target.value) || 10 })}
                    style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: 6, color: "var(--text-primary)", fontSize: "var(--text-sm)" }} />
                </div>
              </div>
            </div>

            <div className="modal-footer">
              <button className="btn btn-ghost" onClick={() => setShowCreate(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleCreate}
                disabled={!newVM.name.trim()}>
                Create & Start
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

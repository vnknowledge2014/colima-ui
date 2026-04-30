import { useState, useEffect, useCallback } from "react";
import { limaApi, LimaInstance } from "../lib/api";

export default function LinuxVMs() {
  const [vms, setVMs] = useState<LimaInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [selectedVM, setSelectedVM] = useState<LimaInstance | null>(null);
  const [shellCmd, setShellCmd] = useState("");
  const [shellOutput, setShellOutput] = useState("");

  const notify = useCallback((type: "success" | "error", text: string) => {
    setNotification({ type, text });
    setTimeout(() => setNotification(null), 4000);
  }, []);

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

  const handleAction = async (name: string, action: "start" | "stop" | "delete") => {
    setActionLoading(`${name}-${action}`);
    try {
      if (action === "start") {
        await limaApi.start(name);
        notify("success", `VM '${name}' starting...`);
      } else if (action === "stop") {
        await limaApi.stop(name);
        notify("success", `VM '${name}' stopped`);
      } else {
        if (!confirm(`Delete VM '${name}'? This cannot be undone.`)) {
          setActionLoading(null);
          return;
        }
        await limaApi.delete(name, true);
        notify("success", `VM '${name}' deleted`);
      }
      setTimeout(fetchVMs, 1000);
    } catch (e) {
      notify("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const runShell = async () => {
    if (!selectedVM || !shellCmd.trim()) return;
    try {
      const output = await limaApi.shell(selectedVM.name, shellCmd);
      setShellOutput(prev => prev + `$ ${shellCmd}\n${output}\n`);
      setShellCmd("");
    } catch (e) {
      setShellOutput(prev => prev + `$ ${shellCmd}\nError: ${e}\n`);
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
        <div className="content-header-actions">
          <button className="btn btn-ghost" onClick={fetchVMs}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
        </div>
      </div>

      <div className="content-body">
        {notification && (
          <div style={{
            position: "fixed", top: 16, right: 16, padding: "10px 16px",
            borderRadius: "var(--radius-md)",
            background: notification.type === "success" ? "rgba(63,185,80,0.15)" : "rgba(248,81,73,0.15)",
            border: `1px solid ${notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)"}`,
            color: notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
            fontSize: "var(--text-sm)", zIndex: 200, boxShadow: "var(--shadow-lg)",
          }}>
            {notification.type === "success" ? "✓" : "✕"} {notification.text}
          </div>
        )}

        {error && (
          <div className="card" style={{ borderColor: "var(--accent-yellow)", marginBottom: 16 }}>
            <p style={{ color: "var(--accent-yellow)", fontSize: "var(--text-sm)" }}>⚠ {error}</p>
          </div>
        )}

        {vms.length > 0 ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {vms.map(vm => {
              const isLoading = actionLoading?.startsWith(vm.name);
              const isRunning = vm.status === "Running";
              return (
                <div key={vm.name} onClick={() => { setSelectedVM(vm); setShellOutput(""); }} style={{
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
                          ● {vm.status}
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
                          disabled={!!isLoading} onClick={() => handleAction(vm.name, "stop")}>⏹ Stop</button>
                      ) : (
                        <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-green)" }}
                          disabled={!!isLoading} onClick={() => handleAction(vm.name, "start")}>▶ Start</button>
                      )}
                      <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)" }}
                        disabled={!!isLoading} onClick={() => handleAction(vm.name, "delete")}>🗑</button>
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
            <div className="empty-state-text">Lima VMs will appear here. Create one with <code>limactl start</code>.</div>
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
                  ● {selectedVM.status}
                </span>
              </h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setSelectedVM(null)}>✕</button>
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
                    placeholder="Enter command..."
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
    </>
  );
}

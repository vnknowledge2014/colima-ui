import { useState, useEffect, useCallback } from "react";
import { composeApi, ComposeProject } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { WarningIcon, RestartIcon, StopIcon, CloseIcon } from "../components/Icons";

export default function Compose() {
  const [projects, setProjects] = useState<ComposeProject[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<ComposeProject | null>(null);
  const [logs, setLogs] = useState("");
  const [services, setServices] = useState("");
  const [detailTab, setDetailTab] = useState<"services" | "logs">("services");

  const fetchProjects = useCallback(async () => {
    try {
      setError(null);
      const list = await composeApi.list();
      setProjects(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchProjects();
    const interval = setInterval(() => {
      if (document.visibilityState === "visible") fetchProjects();
    }, 15000);
    return () => clearInterval(interval);
  }, [fetchProjects]);

  const handleAction = async (name: string, action: "down" | "restart") => {
    setActionLoading(`${name}-${action}`);
    try {
      if (action === "down") {
        await composeApi.down(name);
        globalToast("success", `Project '${name}' stopped`);
      } else {
        await composeApi.restart(name);
        globalToast("success", `Project '${name}' restarted`);
      }
      fetchProjects();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const openProject = async (p: ComposeProject) => {
    setSelectedProject(p);
    setDetailTab("services");
    try {
      const [svc, lg] = await Promise.all([
        composeApi.ps(p.Name),
        composeApi.logs(p.Name, 100),
      ]);
      setServices(svc);
      setLogs(lg);
    } catch (e) {
      setServices(`Error: ${e}`);
      setLogs(`Error: ${e}`);
    }
  };

  const parseStatus = (status: string) => {
    if (!status) return { running: 0, display: "" };
    const match = status.match(/running\((\d+)\)/);
    const running = match ? parseInt(match[1]) : 0;
    return { running, display: status };
  };

  if (loading) {
    return (
      <>
        <div className="content-header"><h1>Docker Compose</h1></div>
        <div className="loading-screen"><div className="spinner" /><span>Loading projects...</span></div>
      </>
    );
  }

  return (
    <>
      <div className="content-header">
        <h1>
          Docker Compose
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {projects.length} project{projects.length !== 1 ? "s" : ""}
          </span>
        </h1>
        <div className="content-header-actions">
          <button className="btn btn-ghost" onClick={fetchProjects}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
        </div>
      </div>

      <div className="content-body">


        {error && (
          <div className="card" style={{ borderColor: "var(--accent-yellow)", marginBottom: 16 }}>
            <p style={{ color: "var(--accent-yellow)", fontSize: "var(--text-sm)", display: "flex", alignItems: "center", gap: 6 }}><WarningIcon size={14} /> {error}</p>
          </div>
        )}

        {projects.length > 0 ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {projects.map(p => {
              const { running } = parseStatus(p.Status);
              const isLoading = actionLoading?.startsWith(p.Name);
              const hasRunning = running > 0;
              return (
                <div key={p.Name} onClick={() => openProject(p)} style={{
                  padding: 16, background: "var(--bg-secondary)", borderRadius: 12,
                  border: "1px solid var(--border-primary)", cursor: "pointer",
                  opacity: isLoading ? 0.6 : 1, transition: "all 200ms",
                }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={hasRunning ? "var(--accent-blue)" : "var(--text-muted)"} strokeWidth="2">
                          <path d="M22 8.35V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8.35A2 2 0 0 1 3.26 6.5l8-3.2a2 2 0 0 1 1.48 0l8 3.2A2 2 0 0 1 22 8.35Z"/>
                          <path d="M6 18h12M6 14h12M6 10h12"/>
                        </svg>
                        <span style={{ fontWeight: 600, fontSize: "var(--text-md)" }}>{p.Name}</span>
                        <span className={`badge badge-${hasRunning ? "running" : "stopped"}`}>
                          <span className="badge-dot" />
                          {p.Status}
                        </span>
                      </div>
                      {p.ConfigFiles && (
                        <div style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)", marginTop: 4, fontFamily: "var(--font-mono)" }}>
                          {p.ConfigFiles}
                        </div>
                      )}
                    </div>
                    <div style={{ display: "flex", gap: 6 }} onClick={e => e.stopPropagation()}>
                      <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)" }} disabled={!!isLoading}
                        onClick={() => handleAction(p.Name, "restart")}><RestartIcon size={12} /> Restart</button>
                      <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)" }}
                        disabled={!!isLoading} onClick={() => handleAction(p.Name, "down")}><StopIcon size={12} /> Down</button>
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
                <path d="M22 8.35V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8.35A2 2 0 0 1 3.26 6.5l8-3.2a2 2 0 0 1 1.48 0l8 3.2A2 2 0 0 1 22 8.35Z"/>
              </svg>
            </div>
            <div className="empty-state-title">No Compose Projects</div>
            <div className="empty-state-text">Start a docker-compose project to see it listed here.</div>
          </div>
        )}
      </div>

      {/* Detail Modal */}
      {selectedProject && (
        <div className="modal-overlay" onClick={() => setSelectedProject(null)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ width: "min(800px, 95vw)", maxHeight: "80vh" }}>
            <div className="modal-header">
              <h2 className="modal-title">{selectedProject.Name}</h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setSelectedProject(null)}><CloseIcon size={16} /></button>
            </div>

            <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--border-primary)", marginBottom: 16 }}>
              <button className="btn" style={{
                background: "transparent", border: "none",
                borderBottom: detailTab === "services" ? "2px solid var(--accent-blue)" : "2px solid transparent",
                color: detailTab === "services" ? "var(--text-primary)" : "var(--text-secondary)",
                borderRadius: 0, padding: "8px 16px", fontWeight: detailTab === "services" ? 600 : 400,
              }} onClick={() => setDetailTab("services")}>Services</button>
              <button className="btn" style={{
                background: "transparent", border: "none",
                borderBottom: detailTab === "logs" ? "2px solid var(--accent-blue)" : "2px solid transparent",
                color: detailTab === "logs" ? "var(--text-primary)" : "var(--text-secondary)",
                borderRadius: 0, padding: "8px 16px", fontWeight: detailTab === "logs" ? 600 : 400,
              }} onClick={() => setDetailTab("logs")}>Logs</button>
            </div>

            {detailTab === "services" && (
              <pre style={{ padding: 12, background: "var(--bg-primary)", borderRadius: 8, fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "50vh", color: "var(--text-secondary)", margin: 0, fontFamily: "var(--font-mono)" }}>
                {services || "No services running"}
              </pre>
            )}

            {detailTab === "logs" && (
              <div className="log-viewer" style={{ maxHeight: "50vh" }}>
                {logs.split("\n").map((line, i) => {
                  let cls = "";
                  if (/error|fatal|panic/i.test(line)) cls = "log-error";
                  else if (/warn/i.test(line)) cls = "log-warn";
                  return (
                    <div key={i} className={`log-line ${cls}`}>
                      <span style={{ color: "var(--text-muted)", marginRight: 8, userSelect: "none" }}>{i + 1}</span>
                      {line}
                    </div>
                  );
                })}
              </div>
            )}

            <div className="modal-footer">
              <button className="btn btn-primary" onClick={() => setSelectedProject(null)}>Close</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

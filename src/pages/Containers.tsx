import { useState, useEffect, useRef, useCallback, useDeferredValue } from "react";
import { dockerApi, DockerContainer } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { useAtom, useAtomValue } from "jotai";
import { containersAtom, dockerLoadingAtom } from "../store/dockerAtom";
import { StopIcon, PlayIcon, PauseIcon, RestartIcon, CloseIcon, WarningIcon } from "../components/Icons";
import { useVirtualizer } from "@tanstack/react-virtual";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import { useHotkeys } from "../hooks/useHotkeys";

/* ===== Container Detail Panel ===== */
function ContainerDetail({
  container,
  onClose,
  onAction,
}: {
  container: DockerContainer;
  onClose: () => void;
  onAction: (id: string, name: string, action: string) => void;
}) {
  const [tab, setTab] = useState<"overview" | "logs" | "stats" | "exec" | "inspect">("overview");
  const [logs, setLogs] = useState("");
  const [inspect, setInspect] = useState("");
  const [stats, setStats] = useState("");
  const [topOutput, setTopOutput] = useState("");
  const [logLoading, setLogLoading] = useState(false);
  const [inspectLoading, setInspectLoading] = useState(false);
  const [logLines, setLogLines] = useState(200);
  const [logFilter, setLogFilter] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const logRef = useRef<HTMLDivElement>(null);

  // Exec state
  const [execCmd, setExecCmd] = useState("");
  const [execOutput, setExecOutput] = useState<string[]>([]);
  const [execLoading, setExecLoading] = useState(false);
  const execRef = useRef<HTMLDivElement>(null);

  const fetchLogs = useCallback(async () => {
    setLogLoading(true);
    try {
      const result = await dockerApi.containerLogs(container.Id, logLines);
      setLogs(result);
      if (autoScroll && logRef.current) {
        setTimeout(() => {
          if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
        }, 50);
      }
    } catch (e) {
      setLogs(`Error fetching logs: ${e}`);
    } finally {
      setLogLoading(false);
    }
  }, [container.Id, logLines, autoScroll]);

  const fetchInspect = useCallback(async () => {
    setInspectLoading(true);
    try {
      const result = await dockerApi.inspectContainer(container.Id);
      setInspect(JSON.stringify(JSON.parse(result), null, 2));
    } catch (e) {
      setInspect(`Error: ${e}`);
    } finally {
      setInspectLoading(false);
    }
  }, [container.Id]);

  const fetchStats = useCallback(async () => {
    try {
      const result = await dockerApi.containerStats(container.Id);
      setStats(result);
    } catch (e) {
      setStats(`Error: ${e}`);
    }
  }, [container.Id]);

  const fetchTop = useCallback(async () => {
    try {
      const result = await dockerApi.containerTop(container.Id);
      setTopOutput(result);
    } catch (e) {
      setTopOutput(`Error: ${e}`);
    }
  }, [container.Id]);

  useEffect(() => {
    if (tab === "logs") fetchLogs();
    if (tab === "inspect") fetchInspect();
    if (tab === "stats") { fetchStats(); fetchTop(); }
  }, [tab, fetchLogs, fetchInspect, fetchStats, fetchTop]);

  // Auto-refresh logs every 3s
  useEffect(() => {
    if (tab !== "logs") return;
    const interval = setInterval(fetchLogs, 3000);
    return () => clearInterval(interval);
  }, [tab, fetchLogs]);

  // Auto-refresh stats every 5s
  useEffect(() => {
    if (tab !== "stats") return;
    const interval = setInterval(() => { fetchStats(); fetchTop(); }, 5000);
    return () => clearInterval(interval);
  }, [tab, fetchStats, fetchTop]);

  const handleExec = async () => {
    if (!execCmd.trim()) return;
    setExecLoading(true);
    const cmd = execCmd;
    setExecOutput(prev => [...prev, `${container.Names}$ ${cmd}`]);
    setExecCmd("");
    try {
      const result = await dockerApi.containerExec(container.Id, cmd);
      setExecOutput(prev => [...prev, result]);
    } catch (e) {
      setExecOutput(prev => [...prev, `Error: ${e}`]);
    } finally {
      setExecLoading(false);
      if (execRef.current) execRef.current.scrollTop = execRef.current.scrollHeight;
    }
  };

  const filteredLogLines = logs.split("\n").filter((line) => {
    if (!logFilter) return true;
    return line.toLowerCase().includes(logFilter.toLowerCase());
  });

  // Parse stats JSON
  let parsedStats: Record<string, string> = {};
  try {
    const trimmed = stats.trim();
    if (trimmed) parsedStats = JSON.parse(trimmed);
  } catch { /* ignore */ }

  const tabs = [
    { id: "overview" as const, label: "Overview" },
    { id: "logs" as const, label: "Logs" },
    { id: "stats" as const, label: "Stats" },
    { id: "exec" as const, label: "Exec" },
    { id: "inspect" as const, label: "Inspect" },
  ];

  const isRunning = container.State === "running";

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: "min(960px, 95vw)", maxHeight: "85vh" }}>
        <div className="modal-header">
          <div>
            <h2 className="modal-title" style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{
                width: 8, height: 8, borderRadius: "50%",
                background: isRunning ? "var(--status-running)" : "var(--status-stopped)",
                display: "inline-block",
              }}/>
              {container.Names}
            </h2>
            <p style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", fontFamily: "var(--font-mono)", marginTop: 4 }}>
              {container.Image} · {container.Id.substring(0, 12)}
            </p>
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            {isRunning ? (
              <>
                <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }} onClick={() => onAction(container.Id, container.Names, "stop")}><StopIcon size={12} /> Stop</button>
                <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }} onClick={() => onAction(container.Id, container.Names, "restart")}><RestartIcon size={12} /> Restart</button>
                <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }} onClick={() => onAction(container.Id, container.Names, "pause")}><PauseIcon size={12} /> Pause</button>
              </>
            ) : (
              <button className="btn btn-primary" style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }} onClick={() => onAction(container.Id, container.Names, "start")}><PlayIcon size={12} /> Start</button>
            )}
            <button className="btn btn-icon btn-ghost" onClick={onClose}><CloseIcon size={16} /></button>
          </div>
        </div>

        {/* Tabs */}
        <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--border-primary)", marginBottom: 16 }}>
          {tabs.map((t) => (
            <button
              key={t.id}
              className="btn"
              style={{
                background: "transparent", border: "none",
                borderBottom: tab === t.id ? "2px solid var(--accent-blue)" : "2px solid transparent",
                color: tab === t.id ? "var(--text-primary)" : "var(--text-secondary)",
                borderRadius: 0, padding: "8px 16px", fontWeight: tab === t.id ? 600 : 400,
              }}
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>

        {/* Tab Content */}
        {tab === "overview" && (
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
            <InfoRow label="Container ID" value={container.Id.substring(0, 12)} mono />
            <InfoRow label="Status" value={container.Status} />
            <InfoRow label="State" value={container.State} />
            <InfoRow label="Image" value={container.Image} mono />
            <InfoRow label="Command" value={container.Command} mono />
            <InfoRow label="Ports" value={container.Ports || "None"} mono />
            <InfoRow label="Created" value={container.CreatedAt} />
            <InfoRow label="Size" value={container.Size || "N/A"} />
          </div>
        )}

        {tab === "logs" && (
          <div>
            <div style={{ display: "flex", gap: 8, marginBottom: 12, alignItems: "center" }}>
              <input className="input" placeholder="Filter logs..." value={logFilter}
                onChange={(e) => setLogFilter(e.target.value)} style={{ flex: 1, maxWidth: 300 }} />
              <select className="input select" style={{ width: 100 }} value={logLines}
                onChange={(e) => setLogLines(Number(e.target.value))}>
                <option value={50}>50 lines</option>
                <option value={200}>200 lines</option>
                <option value={500}>500 lines</option>
                <option value={1000}>1000 lines</option>
              </select>
              <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: "var(--text-xs)", color: "var(--text-secondary)", cursor: "pointer" }}>
                <input type="checkbox" checked={autoScroll} onChange={(e) => setAutoScroll(e.target.checked)} style={{ accentColor: "var(--accent-blue)" }} />
                Auto-scroll
              </label>
              <button className="btn btn-ghost" style={{ padding: "4px 8px", fontSize: "var(--text-xs)" }} onClick={fetchLogs}>
                {logLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : "↻ Refresh"}
              </button>
            </div>
            <div className="log-viewer" ref={logRef} style={{ maxHeight: "50vh" }}>
              {filteredLogLines.map((line, i) => {
                let cls = "";
                if (/error|fatal|panic|exception/i.test(line)) cls = "log-error";
                else if (/warn/i.test(line)) cls = "log-warn";
                else if (/info/i.test(line)) cls = "log-info";
                return (
                  <div key={i} className={`log-line ${cls}`}>
                    <span style={{ color: "var(--text-muted)", marginRight: 8, userSelect: "none" }}>{i + 1}</span>
                    {line}
                  </div>
                );
              })}
              {filteredLogLines.length === 0 && (
                <div style={{ color: "var(--text-muted)", textAlign: "center", padding: 20 }}>
                  {logFilter ? "No matching log lines" : "No logs available"}
                </div>
              )}
            </div>
          </div>
        )}

        {tab === "stats" && (
          <div>
            {isRunning && Object.keys(parsedStats).length > 0 ? (
              <>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr 1fr", gap: 12, marginBottom: 16 }}>
                  <StatCard label="CPU" value={parsedStats.CPUPerc || "0%"} accent="var(--accent-blue)" />
                  <StatCard label="Memory" value={parsedStats.MemPerc || "0%"} sub={parsedStats.MemUsage || ""} accent="var(--accent-green)" />
                  <StatCard label="Net I/O" value={parsedStats.NetIO || "0B/0B"} accent="var(--accent-purple)" />
                  <StatCard label="Block I/O" value={parsedStats.BlockIO || "0B/0B"} accent="var(--accent-orange)" />
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
                  <h3 style={{ margin: 0, fontSize: "var(--text-sm)", fontWeight: 600 }}>Processes</h3>
                  <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", padding: "2px 8px" }} onClick={() => { fetchStats(); fetchTop(); }}>↻</button>
                </div>
                <pre style={{ padding: 12, background: "var(--bg-primary)", borderRadius: 8, fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "30vh", color: "var(--text-secondary)", margin: 0 }}>
                  {topOutput || "No processes running"}
                </pre>
              </>
            ) : (
              <div style={{ textAlign: "center", padding: 40, color: "var(--text-muted)" }}>
                {isRunning ? "Loading stats..." : "Container is not running"}
              </div>
            )}
          </div>
        )}

        {tab === "exec" && (
          <div>
            {isRunning ? (
              <>
                <div ref={execRef} style={{ background: "var(--bg-primary)", borderRadius: 8, padding: 12, maxHeight: "40vh", overflow: "auto", marginBottom: 12, minHeight: 120 }}>
                  {execOutput.length === 0 ? (
                    <div style={{ color: "var(--text-muted)", fontSize: "var(--text-sm)" }}>
                      Run commands inside '{container.Names}'. Output will appear here.
                    </div>
                  ) : (
                    execOutput.map((line, i) => (
                      <div key={i} style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: line.startsWith(container.Names + "$") ? "var(--accent-green)" : line.startsWith("Error") ? "var(--accent-red)" : "var(--text-secondary)", whiteSpace: "pre-wrap", padding: "1px 0" }}>
                        {line}
                      </div>
                    ))
                  )}
                </div>
                <div style={{ display: "flex", gap: 8 }}>
                  <input
                    className="input"
                    value={execCmd}
                    onChange={e => setExecCmd(e.target.value)}
                    placeholder={`${container.Names}$ Enter command...`}
                    style={{ flex: 1, fontFamily: "var(--font-mono)" }}
                    onKeyDown={e => e.key === "Enter" && handleExec()}
                    autoFocus
                  />
                  <button className="btn btn-primary" onClick={handleExec} disabled={execLoading || !execCmd.trim()}>
                    {execLoading ? "Running..." : "Run"}
                  </button>
                  <button className="btn btn-ghost" onClick={() => setExecOutput([])}>Clear</button>
                </div>
              </>
            ) : (
              <div style={{ textAlign: "center", padding: 40, color: "var(--text-muted)" }}>
                Container must be running to execute commands
              </div>
            )}
          </div>
        )}

        {tab === "inspect" && (
          <div>
            {inspectLoading ? (
              <div className="loading-screen" style={{ height: 200 }}><div className="spinner" /><span>Loading...</span></div>
            ) : (
              <div className="log-viewer" style={{ maxHeight: "55vh" }}>
                <pre style={{ margin: 0, color: "var(--text-secondary)" }}>{inspect}</pre>
              </div>
            )}
          </div>
        )}

        <div className="modal-footer">
          <button className="btn btn-primary" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

function StatCard({ label, value, sub, accent }: { label: string; value: string; sub?: string; accent: string }) {
  return (
    <div style={{ padding: 12, background: "var(--bg-primary)", borderRadius: 8, borderLeft: `3px solid ${accent}` }}>
      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: "var(--text-lg)", fontWeight: 700, fontFamily: "var(--font-mono)", color: accent }}>{value}</div>
      {sub && <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 2 }}>{sub}</div>}
    </div>
  );
}

function InfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <span style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", fontWeight: 500 }}>{label}</span>
      <span style={{
        fontSize: "var(--text-sm)", fontFamily: mono ? "var(--font-mono)" : "inherit",
        color: "var(--text-primary)", wordBreak: "break-all",
      }}>
        {value}
      </span>
    </div>
  );
}

/* ===== Run Container Modal ===== */
function RunContainerModal({ onClose, onSuccess }: { onClose: () => void; onSuccess: () => void }) {
  const [image, setImage] = useState("");
  const [name, setName] = useState("");
  const [ports, setPorts] = useState("");
  const [envVars, setEnvVars] = useState("");
  const [vols, setVols] = useState("");
  const [detach, setDetach] = useState(true);
  const [removeOnExit, setRemoveOnExit] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleRun = async () => {
    if (!image.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const portsList = ports.split("\n").map(s => s.trim()).filter(Boolean);
      const envList = envVars.split("\n").map(s => s.trim()).filter(Boolean);
      const volList = vols.split("\n").map(s => s.trim()).filter(Boolean);
      await dockerApi.runContainer(image.trim(), name.trim(), portsList, envList, volList, detach, removeOnExit);
      onSuccess();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()} style={{ width: "min(600px, 95vw)" }}>
        <div className="modal-header">
          <h2 className="modal-title">Run Container</h2>
          <button className="btn btn-icon btn-ghost" onClick={onClose}><CloseIcon size={16} /></button>
        </div>

        {error && (
          <div style={{ padding: 12, background: "rgba(248,81,73,0.1)", color: "var(--accent-red)", borderRadius: 8, marginBottom: 12, fontSize: "var(--text-sm)" }}>
            <WarningIcon size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> {error}
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <div>
            <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: 4 }}>Image *</label>
            <input className="input" value={image} onChange={e => setImage(e.target.value)} placeholder="nginx:latest, redis:alpine..." style={{ width: "100%" }} autoFocus />
          </div>
          <div>
            <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: 4 }}>Container Name</label>
            <input className="input" value={name} onChange={e => setName(e.target.value)} placeholder="my-container" style={{ width: "100%" }} />
          </div>
          <div>
            <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: 4 }}>Ports (one per line, host:container)</label>
            <textarea className="input" value={ports} onChange={e => setPorts(e.target.value)} placeholder="8080:80&#10;3000:3000" style={{ width: "100%", minHeight: 50, resize: "vertical", fontFamily: "var(--font-mono)" }} />
          </div>
          <div>
            <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: 4 }}>Environment Variables (KEY=VALUE, one per line)</label>
            <textarea className="input" value={envVars} onChange={e => setEnvVars(e.target.value)} placeholder="NODE_ENV=production&#10;PORT=3000" style={{ width: "100%", minHeight: 50, resize: "vertical", fontFamily: "var(--font-mono)" }} />
          </div>
          <div>
            <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: 4 }}>Volumes (host:container, one per line)</label>
            <textarea className="input" value={vols} onChange={e => setVols(e.target.value)} placeholder="/data:/app/data" style={{ width: "100%", minHeight: 50, resize: "vertical", fontFamily: "var(--font-mono)" }} />
          </div>
          <div style={{ display: "flex", gap: 16 }}>
            <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "var(--text-sm)", color: "var(--text-secondary)", cursor: "pointer" }}>
              <input type="checkbox" checked={detach} onChange={e => setDetach(e.target.checked)} style={{ accentColor: "var(--accent-blue)" }} />
              Detached mode
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "var(--text-sm)", color: "var(--text-secondary)", cursor: "pointer" }}>
              <input type="checkbox" checked={removeOnExit} onChange={e => setRemoveOnExit(e.target.checked)} style={{ accentColor: "var(--accent-blue)" }} />
              Remove on exit
            </label>
          </div>
        </div>

        <div className="modal-footer">
          <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" onClick={handleRun} disabled={loading || !image.trim()}>
            {loading ? "Starting..." : <><PlayIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> Run Container</>}
          </button>
        </div>
      </div>
    </div>
  );
}

/* ===== Virtualized Container Rows ===== */
function VirtualContainerRows({
  filtered,
  selected,
  actionLoading,
  gridCols,
  rowHeight,
  toggleSelect,
  setSelectedContainer,
  handleAction,
  onContextMenu,
}: {
  filtered: DockerContainer[];
  selected: Set<string>;
  actionLoading: string | null;
  gridCols: string;
  rowHeight: number;
  toggleSelect: (id: string) => void;
  setSelectedContainer: (c: DockerContainer) => void;
  handleAction: (id: string, name: string, action: string) => void;
  onContextMenu: (e: React.MouseEvent, c: DockerContainer) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });

  return (
    <div ref={scrollRef} className="vtable-scroll">
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((vRow) => {
          const c = filtered[vRow.index];
          const isRunning = c.State === 'running';
          const isPaused = c.Status.toLowerCase().includes('paused');
          const isLoading = actionLoading?.startsWith(c.Id);
          return (
            <div
              key={c.Id}
              className={`vtable-row${selected.has(c.Id) ? ' selected' : ''}`}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: vRow.size,
                transform: `translateY(${vRow.start}px)`,
                display: 'grid',
                gridTemplateColumns: gridCols,
                opacity: isLoading ? 0.6 : 1,
              }}
              onClick={() => setSelectedContainer(c)}
              onContextMenu={(e) => onContextMenu(e, c)}
            >
              <div className="vtable-cell" style={{ textAlign: 'center' }} onClick={(e) => e.stopPropagation()}>
                <input type="checkbox" checked={selected.has(c.Id)} onChange={() => toggleSelect(c.Id)}
                  style={{ accentColor: 'var(--accent-blue)', cursor: 'pointer' }} />
              </div>
              <div className="vtable-cell">
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <div style={{
                    width: 8, height: 8, borderRadius: '50%',
                    background: isPaused ? 'var(--accent-yellow)' : isRunning ? 'var(--status-running)' : 'var(--status-stopped)',
                    boxShadow: isRunning && !isPaused ? '0 0 6px var(--status-running)' : 'none',
                    flexShrink: 0,
                  }} />
                  <span style={{ fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {c.Names}
                  </span>
                </div>
              </div>
              <div className="vtable-cell" style={{ color: 'var(--text-secondary)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)' }}>
                {c.Image}
              </div>
              <div className="vtable-cell">
                <span className={`badge badge-${isPaused ? 'stopped' : isRunning ? 'running' : 'stopped'}`}>
                  <span className="badge-dot" />
                  {c.Status}
                </span>
              </div>
              <div className="vtable-cell" style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>
                {c.Ports || '—'}
              </div>
              <div className="vtable-cell" onClick={(e) => e.stopPropagation()}>
                <div className="table-actions" style={{ justifyContent: 'flex-end' }}>
                  {isPaused ? (
                    <button className="btn btn-ghost btn-icon" data-tooltip="Unpause" disabled={!!isLoading} onClick={() => handleAction(c.Id, c.Names, 'unpause')}>
                      <PlayIcon size={14} />
                    </button>
                  ) : isRunning ? (
                    <button className="btn btn-ghost btn-icon" data-tooltip="Stop" disabled={!!isLoading} onClick={() => handleAction(c.Id, c.Names, 'stop')}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1" /></svg>
                    </button>
                  ) : (
                    <button className="btn btn-ghost btn-icon" data-tooltip="Start" disabled={!!isLoading} onClick={() => handleAction(c.Id, c.Names, 'start')}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="6,4 20,12 6,20" /></svg>
                    </button>
                  )}
                  <button className="btn btn-ghost btn-icon" data-tooltip="Restart" disabled={!!isLoading} onClick={() => handleAction(c.Id, c.Names, 'restart')}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3" />
                    </svg>
                  </button>
                  <button className="btn btn-ghost btn-icon" data-tooltip="View Details" onClick={() => setSelectedContainer(c)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" /><circle cx="12" cy="12" r="3" />
                    </svg>
                  </button>
                  <button className="btn btn-ghost btn-icon" data-tooltip="Remove" disabled={!!isLoading}
                    onClick={() => handleAction(c.Id, c.Names, 'remove')} style={{ color: 'var(--accent-red)' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ===== Main Containers Page ===== */
export default function Containers() {
  const [containers, setContainers] = useAtom(containersAtom);
  const loading = useAtomValue(dockerLoadingAtom);

  const refreshContainers = useCallback(async () => {
    try {
      const list = await dockerApi.listContainers(true);
      setContainers(list);
    } catch { /* ignore */ }
  }, [setContainers]);
  const [filter, setFilter] = useState<"all" | "running" | "stopped">("all");
  const [searchTerm, setSearchTerm] = useState("");
  const deferredSearch = useDeferredValue(searchTerm);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [selectedContainer, setSelectedContainer] = useState<DockerContainer | null>(null);
  const [showRunModal, setShowRunModal] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [batchLoading, setBatchLoading] = useState(false);
  const { confirm, ConfirmDialogProps } = useConfirm();
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; container: DockerContainer } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // Hotkeys
  useHotkeys({
    "mod+k": () => searchRef.current?.focus(),
    "escape": () => { setSelectedContainer(null); setCtxMenu(null); },
    "delete": () => { if (selected.size > 0) handleBatchRemove(); },
    "backspace": () => { if (selected.size > 0) handleBatchRemove(); },
  });

  const openCtxMenu = (e: React.MouseEvent, c: DockerContainer) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY, container: c });
  };

  const getCtxItems = (c: DockerContainer): ContextMenuItem[] => {
    const isRunning = c.State === "running";
    const isPaused = c.Status.toLowerCase().includes("paused");
    return [
      ...(isPaused
        ? [{ label: "Unpause", icon: <PlayIcon size={14} />, action: () => handleAction(c.Id, c.Names, "unpause") }]
        : isRunning
        ? [{ label: "Stop", icon: <StopIcon size={14} />, action: () => handleAction(c.Id, c.Names, "stop") }]
        : [{ label: "Start", icon: <PlayIcon size={14} />, action: () => handleAction(c.Id, c.Names, "start") }]),
      { label: "Restart", icon: <RestartIcon size={14} />, action: () => handleAction(c.Id, c.Names, "restart") },
      { label: "View Details", action: () => setSelectedContainer(c) },
      { divider: true, label: "", action: () => {} },
      { label: "Copy ID", action: () => { navigator.clipboard.writeText(c.Id); globalToast("success", "Container ID copied"); } },
      { divider: true, label: "", action: () => {} },
      { label: "Remove", danger: true, action: () => handleAction(c.Id, c.Names, "remove") },
    ];
  };

  const handleAction = async (id: string, name: string, action: string) => {
    setActionLoading(`${id}-${action}`);
    try {
      switch (action) {
        case "start": await dockerApi.startContainer(id); break;
        case "stop": await dockerApi.stopContainer(id); break;
        case "restart": await dockerApi.restartContainer(id); break;
        case "remove": {
          const ok = await confirm({ title: "Remove Container", message: `Remove container "${name}"?\n\nThis will permanently delete the container and its data.`, confirmText: "Remove", variant: "danger" });
          if (!ok) { setActionLoading(null); return; }
          await dockerApi.removeContainer(id, true);
          // Always clear from selection (avoid stale closure from confirm dialog)
          setSelected(prev => { const next = new Set(prev); next.delete(id); return next; });
          break;
        }
        case "pause": await dockerApi.pauseContainer(id); break;
        case "unpause": await dockerApi.unpauseContainer(id); break;
      }
      const pastTense: Record<string, string> = {
        start: "started", stop: "stopped", restart: "restarted", remove: "removed", pause: "paused", unpause: "unpaused",
      };
      globalToast("success", `Container '${name}' ${pastTense[action] || action}`);
      await refreshContainers();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  // Batch actions
  const handleBatchStop = async () => {
    const names = filtered.filter(c => selected.has(c.Id) && c.State === "running").map(c => c.Names);
    if (names.length === 0) return;
    const ok = await confirm({ title: "Stop Selected", message: `Stop ${names.length} container${names.length > 1 ? "s" : ""}?\n\n${names.join(", ")}`, confirmText: "Stop All", variant: "warning" });
    if (!ok) return;
    setBatchLoading(true);
    let ok_count = 0;
    for (const c of filtered.filter(c => selected.has(c.Id) && c.State === "running")) {
      try { await dockerApi.stopContainer(c.Id); ok_count++; } catch { /* continue */ }
    }
    globalToast("success", `Stopped ${ok_count} container${ok_count > 1 ? "s" : ""}`);
    setSelected(new Set());
    setBatchLoading(false);
    await refreshContainers();
  };

  const handleBatchRemove = async () => {
    const names = filtered.filter(c => selected.has(c.Id)).map(c => c.Names);
    if (names.length === 0) return;
    const ok = await confirm({ title: "Remove Selected", message: `Remove ${names.length} container${names.length > 1 ? "s" : ""}?\n\n${names.join(", ")}\n\nThis cannot be undone.`, confirmText: `Remove ${names.length}`, variant: "danger" });
    if (!ok) return;
    setBatchLoading(true);
    let ok_count = 0;
    for (const c of filtered.filter(c => selected.has(c.Id))) {
      try { await dockerApi.removeContainer(c.Id, true); ok_count++; } catch { /* continue */ }
    }
    globalToast("success", `Removed ${ok_count} container${ok_count > 1 ? "s" : ""}`);
    setSelected(new Set());
    setBatchLoading(false);
    await refreshContainers();
  };

  const filtered = containers.filter((c) => {
    if (filter === "running") return c.State === "running";
    if (filter === "stopped") return c.State !== "running";
    return true;
  }).filter((c) => {
    if (!searchTerm) return true;
    const term = deferredSearch.toLowerCase();
    return c.Names.toLowerCase().includes(term) || c.Image.toLowerCase().includes(term) || c.Id.toLowerCase().includes(term);
  });

  // Clear selection when filter changes
  useEffect(() => { setSelected(new Set()); }, [filter, searchTerm]);

  // Auto-cleanup: remove stale selections when data changes
  useEffect(() => {
    setSelected(prev => {
      const validIds = new Set(containers.map(c => c.Id));
      const next = new Set([...prev].filter(id => validIds.has(id)));
      return next.size !== prev.size ? next : prev;
    });
  }, [containers]);

  const toggleSelect = (id: string) => setSelected(prev => {
    const next = new Set(prev);
    next.has(id) ? next.delete(id) : next.add(id);
    return next;
  });
  const toggleAll = () => {
    if (selected.size === filtered.length) setSelected(new Set());
    else setSelected(new Set(filtered.map(c => c.Id)));
  };

  const runningCount = containers.filter((c) => c.State === "running").length;

  if (loading) {
    return (
      <>
        <div className="content-header"><h1>Containers</h1></div>
        <div className="loading-screen"><div className="spinner" /><span>Loading containers...</span></div>
      </>
    );
  }

  return (
    <>
      <div className="content-header">
        <h1>
          Containers
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {runningCount} running · {containers.length} total
          </span>
        </h1>
        <div className="content-header-actions">
          <input ref={searchRef} className="input" placeholder="Search containers..." value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)} style={{ width: 180 }} />
          <div style={{ display: "flex", gap: 2, background: "var(--bg-card)", borderRadius: "var(--radius-md)", padding: 2 }}>
            {(["all", "running", "stopped"] as const).map((f) => (
              <button key={f} className="btn" style={{
                background: filter === f ? "var(--bg-card-hover)" : "transparent",
                color: filter === f ? "var(--text-primary)" : "var(--text-muted)",
                border: "none", fontSize: "var(--text-xs)", padding: "4px 10px", textTransform: "capitalize",
              }} onClick={() => setFilter(f)}>
                {f}
              </button>
            ))}
          </div>
          <button className="btn btn-primary" style={{ display: "flex", alignItems: "center", gap: 4 }} onClick={() => setShowRunModal(true)}><PlayIcon size={12} /> Run</button>
        </div>
      </div>

      <div className="content-body">

        {/* Batch action bar */}
        {selected.size > 0 && (
          <div style={{
            display: "flex", alignItems: "center", gap: 12, padding: "10px 16px", marginBottom: 12,
            background: "rgba(88,166,255,0.08)", border: "1px solid rgba(88,166,255,0.25)",
            borderRadius: "var(--radius-md)", animation: "slideUp 200ms ease",
          }}>
            <span style={{ fontSize: "var(--text-sm)", color: "var(--accent-blue)", fontWeight: 600 }}>
              {selected.size} selected
            </span>
            <div style={{ flex: 1 }} />
            <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-yellow)" }}
              onClick={handleBatchStop} disabled={batchLoading}>
              <StopIcon size={12} /> Stop Selected
            </button>
            <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)" }}
              onClick={handleBatchRemove} disabled={batchLoading}>
              {batchLoading ? "Removing..." : <><CloseIcon size={12} /> Remove Selected</>}
            </button>
            <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)" }}
              onClick={() => setSelected(new Set())}>
              Clear
            </button>
          </div>
        )}

        {filtered.length > 0 ? (() => {
          const ROW_H = 48;
          const COL_W = { check: 36, name: 'minmax(160px,1.5fr)', image: 'minmax(140px,1fr)', status: '140px', ports: 'minmax(120px,1fr)', actions: '180px' };
          const gridCols = `${COL_W.check}px ${COL_W.name} ${COL_W.image} ${COL_W.status} ${COL_W.ports} ${COL_W.actions}`;
          return (
          <div className="vtable">
            {/* Header */}
            <div className="vtable-header" style={{ display: 'grid', gridTemplateColumns: gridCols }}>
              <div className="vtable-header-cell" style={{ textAlign: 'center' }}>
                <input type="checkbox" checked={filtered.length > 0 && selected.size === filtered.length}
                  onChange={toggleAll} style={{ accentColor: 'var(--accent-blue)', cursor: 'pointer' }} />
              </div>
              <div className="vtable-header-cell">Name</div>
              <div className="vtable-header-cell">Image</div>
              <div className="vtable-header-cell">Status</div>
              <div className="vtable-header-cell">Ports</div>
              <div className="vtable-header-cell" style={{ textAlign: 'right' }}>Actions</div>
            </div>
            {/* Virtual Body */}
            <VirtualContainerRows
              filtered={filtered}
              selected={selected}
              actionLoading={actionLoading}
              gridCols={gridCols}
              rowHeight={ROW_H}
              toggleSelect={toggleSelect}
              setSelectedContainer={setSelectedContainer}
              handleAction={handleAction}
              onContextMenu={openCtxMenu}
            />
          </div>
          );
        })() : (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <path d="M22 8.35V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8.35A2 2 0 0 1 3.26 6.5l8-3.2a2 2 0 0 1 1.48 0l8 3.2A2 2 0 0 1 22 8.35Z"/>
              </svg>
            </div>
            <div className="empty-state-title">{filter === "all" && !searchTerm ? "No containers" : "No matching containers"}</div>
            <div className="empty-state-text">
              {searchTerm ? "Try a different search term." : "Click \"Run\" to start a container from an image."}
            </div>
          </div>
        )}
      </div>

      {selectedContainer && (
        <ContainerDetail container={selectedContainer} onClose={() => setSelectedContainer(null)} onAction={handleAction} />
      )}

      {showRunModal && (
        <RunContainerModal onClose={() => setShowRunModal(false)} onSuccess={() => { globalToast("success", "Container started!"); }} />
      )}
      <ConfirmDialog {...ConfirmDialogProps} />
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={getCtxItems(ctxMenu.container)} onClose={() => setCtxMenu(null)} />}
    </>
  );
}

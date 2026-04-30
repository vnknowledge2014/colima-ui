import { useState, useEffect, useCallback } from "react";
import { SystemInfo, dockerApi, aiApi } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { BroomIcon, SearchIcon, CheckIcon, ErrorIcon, RefreshIcon, GearIcon, RobotIcon, BoltIcon } from "../components/Icons";

const AI_PROVIDERS = [
  { id: "anthropic", label: "Anthropic" },
  { id: "openai", label: "OpenAI" },
  { id: "gemini", label: "Google Gemini" },
  { id: "ollama-local", label: "Ollama Local" },
  { id: "ollama-cloud", label: "Ollama Cloud" },
];

interface SettingsProps {
  systemInfo: SystemInfo | null;
}

interface DiskUsage {
  type: string;
  total: string;
  active: string;
  size: string;
  reclaimable: string;
}

export default function Settings({ systemInfo }: SettingsProps) {
  const [diskUsage, setDiskUsage] = useState<DiskUsage[]>([]);
  const [pruning, setPruning] = useState(false);
  const { confirm, ConfirmDialogProps } = useConfirm();

  // AI & Diagnostics state
  const [aiProvider, setAiProvider] = useState(() => localStorage.getItem("ai_provider") || "anthropic");
  const [aiModel, setAiModel] = useState(() => localStorage.getItem("ai_model") || "");
  const [aiApiKey, setAiApiKey] = useState(() => localStorage.getItem("ai_api_key") || "");
  const [aiEndpoint, setAiEndpoint] = useState(() => localStorage.getItem("ai_endpoint") || "");
  const [searxngInstances, setSearxngInstances] = useState(() => {
    try { return JSON.parse(localStorage.getItem("ai_searxng_instances") || '["http://localhost:8888/search","https://search.inetol.net/search","https://searx.be/search","https://search.brave4u.com/search","https://priv.au/search"]').join("\n"); }
    catch { return "http://localhost:8888/search\nhttps://search.inetol.net/search\nhttps://searx.be/search\nhttps://search.brave4u.com/search\nhttps://priv.au/search"; }
  });
  const [contentMode, setContentMode] = useState(() => localStorage.getItem("ai_diag_content_mode") || "full");
  const [maxPageSize, setMaxPageSize] = useState(() => localStorage.getItem("ai_diag_max_page_size") || "8000");
  const [autoTrigger, setAutoTrigger] = useState(() => localStorage.getItem("ai_diag_auto_trigger") !== "false");
  const [searxngTesting, setSearxngTesting] = useState(false);
  const [searxngStatus, setSearxngStatus] = useState<"ok" | "fail" | null>(null);
  const [searxngError, setSearxngError] = useState("");
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [modelsFetching, setModelsFetching] = useState(false);

  // Persist AI settings
  useEffect(() => { localStorage.setItem("ai_provider", aiProvider); }, [aiProvider]);
  useEffect(() => { localStorage.setItem("ai_model", aiModel); }, [aiModel]);
  useEffect(() => { localStorage.setItem("ai_api_key", aiApiKey); }, [aiApiKey]);
  useEffect(() => { localStorage.setItem("ai_endpoint", aiEndpoint); }, [aiEndpoint]);
  useEffect(() => {
    const arr = searxngInstances.split("\n").map((s: string) => s.trim()).filter(Boolean);
    localStorage.setItem("ai_searxng_instances", JSON.stringify(arr));
  }, [searxngInstances]);
  useEffect(() => { localStorage.setItem("ai_diag_content_mode", contentMode); }, [contentMode]);
  useEffect(() => { localStorage.setItem("ai_diag_max_page_size", maxPageSize); }, [maxPageSize]);
  useEffect(() => { localStorage.setItem("ai_diag_auto_trigger", String(autoTrigger)); }, [autoTrigger]);

  const fetchModels = useCallback(async () => {
    setModelsFetching(true);
    try {
      const raw = await aiApi.listModels(aiProvider, aiApiKey, aiEndpoint);
      const parsed: string[] = JSON.parse(typeof raw === "string" ? raw : "[]");
      setAvailableModels([...new Set(parsed)]);
    } catch {
      setAvailableModels([]);
    } finally {
      setModelsFetching(false);
    }
  }, [aiProvider, aiApiKey, aiEndpoint]);

  const testSearxng = async () => {
    setSearxngTesting(true);
    setSearxngStatus(null);
    setSearxngError("");
    try {
      const instances = searxngInstances.split("\n").map((s: string) => s.trim()).filter(Boolean);
      // Pass instances to backend; it will try SearXNG first, then DuckDuckGo fallback
      const results = await aiApi.search("colima docker", instances.length > 0 ? instances : undefined, 3);
      if (Array.isArray(results) && results.length > 0) {
        const engine = results[0]?.engine || "unknown";
        setSearxngStatus("ok");
        setSearxngError(engine === "duckduckgo" ? "via DuckDuckGo fallback" : `via ${engine}`);
      } else {
        setSearxngStatus("fail");
        setSearxngError("No results returned");
      }
    } catch (e) {
      setSearxngStatus("fail");
      const msg = String(e);
      if (msg.includes("429") || msg.includes("Too Many")) {
        setSearxngError("Rate limited (429) — all instances busy");
      } else if (msg.includes("Connection refused") || msg.includes("connection refused")) {
        setSearxngError("Connection refused — is SearXNG running?");
      } else if (msg.includes("timeout")) {
        setSearxngError("Connection timed out");
      } else {
        setSearxngError(msg.length > 100 ? msg.slice(0, 100) + "…" : msg);
      }
    } finally {
      setSearxngTesting(false);
    }
  };

  useEffect(() => {
    fetchDiskUsage();
  }, []);

  const fetchDiskUsage = async () => {
    try {
      const raw = await dockerApi.systemDf();
      if (!raw) return;
      const text = typeof raw === 'string' ? raw : String(raw);
      // Parse docker system df output
      const lines = text.split("\n").filter((l: string) => l.trim());
      const rows: DiskUsage[] = [];
      for (const line of lines) {
        if (line.startsWith("TYPE") || line.startsWith("---")) continue;
        const parts = line.split(/\s{2,}/);
        if (parts.length >= 4) {
          rows.push({
            type: parts[0],
            total: parts[1],
            active: parts[2],
            size: parts[3],
            reclaimable: parts[4] || "0B",
          });
        }
      }
      setDiskUsage(rows);
    } catch { /* ignore */ }
  };

  const handlePrune = async () => {
    const ok = await confirm({ title: "System Prune", message: "Remove all unused Docker data (stopped containers, unused networks, dangling images, build cache)?", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    setPruning(true);
    try {
      await dockerApi.systemPrune();
      globalToast("success", "System pruned successfully");
      fetchDiskUsage();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setPruning(false);
    }
  };

  const deps = [
    { name: "Colima", desc: "Container runtime manager", installed: systemInfo?.colima_installed, version: systemInfo?.colima_version },
    { name: "Docker", desc: "Container engine client", installed: systemInfo?.docker_installed, version: systemInfo?.docker_version },
    { name: "Lima", desc: "Linux virtual machine manager", installed: systemInfo?.lima_installed, version: systemInfo?.lima_version },
  ];

  return (
    <>
      <div className="content-header"><h1>Settings</h1></div>
      <div className="content-body">


        {/* System Dependencies */}
        <div className="card" style={{ marginBottom: 24 }}>
          <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600, marginBottom: 20 }}>System Dependencies</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: 0 }}>
            {deps.map((dep, i) => (
              <div key={dep.name} style={{
                display: "flex", justifyContent: "space-between", alignItems: "center", padding: "12px 0",
                borderBottom: i < deps.length - 1 ? "1px solid var(--border-subtle)" : "none",
              }}>
                <div>
                  <div style={{ fontWeight: 500 }}>{dep.name}</div>
                  <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>{dep.desc}</div>
                </div>
                <div style={{ textAlign: "right" }}>
                  <span className={`badge ${dep.installed ? "badge-running" : "badge-stopped"}`}>
                    {dep.installed ? "Installed" : "Not Found"}
                  </span>
                  {dep.version && (
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", fontFamily: "var(--font-mono)", marginTop: 4 }}>
                      {dep.version.split("\n")[0]}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Docker Disk Usage */}
        {diskUsage.length > 0 && (
          <div className="card" style={{ marginBottom: 24 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
              <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600 }}>Docker Disk Usage</h3>
              <button className="btn btn-ghost" style={{ color: "var(--accent-red)", fontSize: "var(--text-xs)" }}
                disabled={pruning} onClick={handlePrune}>
                {pruning ? "Pruning..." : <><BroomIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> System Prune</>}
              </button>
            </div>
            <table className="data-table">
              <thead>
                <tr><th>Type</th><th>Total</th><th>Active</th><th>Size</th><th>Reclaimable</th></tr>
              </thead>
              <tbody>
                {diskUsage.map(row => (
                  <tr key={row.type}>
                    <td style={{ fontWeight: 500, fontSize: "var(--text-sm)" }}>{row.type}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{row.total}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{row.active}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--accent-yellow)" }}>{row.size}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--accent-green)" }}>{row.reclaimable}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* AI & Diagnostics */}
        <div className="card" style={{ marginBottom: 24 }}>
          <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600, marginBottom: 20, display: "flex", alignItems: "center", gap: 8 }}>
            <RobotIcon size={18} /> AI & Diagnostics
          </h3>

          {/* AI Provider */}
          <div style={{ marginBottom: 16 }}>
            <div style={{ fontSize: "var(--text-xs)", fontWeight: 600, color: "var(--text-secondary)", marginBottom: 10, textTransform: "uppercase" as const, letterSpacing: "0.05em", display: "flex", alignItems: "center", gap: 6 }}>
              <GearIcon size={12} /> AI Provider
            </div>
            <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>Provider</label>
                <select value={aiProvider} onChange={e => { setAiProvider(e.target.value); setAiModel(""); }}
                  className="settings-select">
                  {AI_PROVIDERS.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}
                </select>
              </div>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>
                  Model {modelsFetching && <span className="spinner" style={{ width: 10, height: 10, borderWidth: 1.5, display: "inline-block", verticalAlign: "middle", marginLeft: 4 }} />}
                </label>
                <input type="text" list="settings-ai-models" value={aiModel} onChange={e => setAiModel(e.target.value)}
                  placeholder="Type or select..."
                  className="settings-input" />
                <datalist id="settings-ai-models">
                  {availableModels.map(m => <option key={m} value={m} />)}
                </datalist>
                <button className="btn btn-ghost" style={{ fontSize: "10px", padding: "2px 6px", marginTop: 4, display: "flex", alignItems: "center", gap: 3 }}
                  onClick={fetchModels} disabled={modelsFetching}>
                  <RefreshIcon size={10} /> {modelsFetching ? "Fetching..." : "Refresh models"}
                </button>
              </div>
            </div>
            {aiProvider !== "ollama-local" && (
              <div style={{ marginBottom: 8 }}>
                <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>API Key</label>
                <input type="password" value={aiApiKey} onChange={e => setAiApiKey(e.target.value)}
                  placeholder="Enter API key..."
                  className="settings-input" style={{ fontFamily: "var(--font-mono)" }} />
              </div>
            )}
            {aiProvider === "ollama-cloud" && (
              <div>
                <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>Endpoint URL</label>
                <input type="text" value={aiEndpoint} onChange={e => setAiEndpoint(e.target.value)}
                  placeholder="https://your-ollama-server.com"
                  className="settings-input" style={{ fontFamily: "var(--font-mono)" }} />
              </div>
            )}
          </div>

          <div style={{ borderTop: "1px solid var(--border-subtle)", margin: "0 0 16px" }} />

          {/* Web Search */}
          <div style={{ marginBottom: 16 }}>
            <div style={{ fontSize: "var(--text-xs)", fontWeight: 600, color: "var(--text-secondary)", marginBottom: 10, textTransform: "uppercase" as const, letterSpacing: "0.05em", display: "flex", alignItems: "center", gap: 6 }}>
              <SearchIcon size={12} /> Web Search
            </div>
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: 10, lineHeight: 1.6, padding: "8px 10px", background: "rgba(88,166,255,0.06)", borderRadius: "var(--radius-md)", border: "1px solid rgba(88,166,255,0.1)" }}>
              Search uses SearXNG instances first, then DuckDuckGo as fallback.
              Public SearXNG instances may rate-limit API access.
              For reliable results, run a local instance: <code style={{ fontSize: "10px", background: "rgba(255,255,255,0.06)", padding: "1px 4px", borderRadius: 3 }}>docker run -d -p 8888:8080 searxng/searxng</code>
            </div>
            <div style={{ marginBottom: 8 }}>
              <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>SearXNG Instances (one per line)</label>
              <textarea value={searxngInstances} onChange={e => setSearxngInstances(e.target.value)}
                rows={4} placeholder={"http://localhost:8888/search\nhttps://search.inetol.net/search"}
                className="settings-input" style={{ fontFamily: "var(--font-mono)", resize: "vertical" as const, lineHeight: 1.5 }} />
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }}
                onClick={testSearxng} disabled={searxngTesting}>
                {searxngTesting ? (
                  <><span className="spinner" style={{ width: 10, height: 10, borderWidth: 1.5 }} /> Testing...</>
                ) : (
                  <><SearchIcon size={12} /> Test Web Search</>
                )}
              </button>
              {searxngStatus === "ok" && (
                <span style={{ color: "var(--accent-green)", fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }}>
                  <CheckIcon size={12} color="var(--accent-green)" /> Connected{searxngError && ` — ${searxngError}`}
                </span>
              )}
              {searxngStatus === "fail" && (
                <span style={{ color: "var(--accent-red)", fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4 }}>
                  <ErrorIcon size={12} color="var(--accent-red)" /> Failed{searxngError && ` — ${searxngError}`}
                </span>
              )}
            </div>
          </div>

          <div style={{ borderTop: "1px solid var(--border-subtle)", margin: "0 0 16px" }} />

          {/* Content Processing */}
          <div style={{ marginBottom: 16 }}>
            <div style={{ fontSize: "var(--text-xs)", fontWeight: 600, color: "var(--text-secondary)", marginBottom: 10, textTransform: "uppercase" as const, letterSpacing: "0.05em", display: "flex", alignItems: "center", gap: 6 }}>
              <BoltIcon size={12} /> Content Processing
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>Content Mode</label>
                <select value={contentMode} onChange={e => setContentMode(e.target.value)}
                  className="settings-select">
                  <option value="full">Full — Keep images + links</option>
                  <option value="compact">Compact — Strip images only</option>
                  <option value="minimal">Minimal — Strip images + links</option>
                </select>
              </div>
              <div style={{ flex: 1 }}>
                <label style={{ fontSize: "11px", color: "var(--text-muted)", display: "block", marginBottom: 4 }}>Max Page Size (chars)</label>
                <input type="number" value={maxPageSize} onChange={e => setMaxPageSize(e.target.value)}
                  min={1000} max={50000} step={1000}
                  className="settings-input" style={{ fontFamily: "var(--font-mono)" }} />
              </div>
            </div>
          </div>

          <div style={{ borderTop: "1px solid var(--border-subtle)", margin: "0 0 16px" }} />

          {/* Behavior */}
          <div>
            <div style={{ fontSize: "var(--text-xs)", fontWeight: 600, color: "var(--text-secondary)", marginBottom: 10, textTransform: "uppercase" as const, letterSpacing: "0.05em" }}>
              Behavior
            </div>
            <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer", fontSize: "var(--text-sm)" }}>
              <input type="checkbox" checked={autoTrigger} onChange={e => setAutoTrigger(e.target.checked)}
                style={{ width: 16, height: 16, accentColor: "var(--accent-blue)" }} />
              <span>Auto-trigger on errors</span>
            </label>
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 4, marginLeft: 24 }}>
              When enabled, any application error automatically opens the AI diagnostics bubble and starts investigation.
            </div>
          </div>
        </div>

        {/* About */}
        <div className="card">
          <h3 style={{ fontSize: "var(--text-lg)", fontWeight: 600, marginBottom: 16 }}>About ColimaUI</h3>
          <p style={{ fontSize: "var(--text-sm)", color: "var(--text-secondary)", lineHeight: 1.7 }}>
            ColimaUI is a cross-platform graphical interface for managing Colima instances,
            Docker containers, Kubernetes clusters, and Linux VMs. Built with Tauri v2 and React.
          </p>
          <div style={{ marginTop: 16, display: "flex", gap: 12, flexWrap: "wrap" }}>
            <span className="badge" style={{ background: "rgba(88, 166, 255, 0.1)", color: "var(--accent-blue)" }}>v0.1.0</span>
            <span className="badge" style={{ background: "rgba(188, 140, 255, 0.1)", color: "var(--accent-purple)" }}>Tauri v2</span>
            <span className="badge" style={{ background: "rgba(57, 210, 192, 0.1)", color: "var(--accent-cyan)" }}>React</span>
            <span className="badge" style={{ background: "rgba(63,185,80,0.1)", color: "var(--accent-green)" }}>Rust</span>
          </div>
        </div>
      </div>
      <ConfirmDialog {...ConfirmDialogProps} />
    </>
  );
}

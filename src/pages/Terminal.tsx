import { useState, useEffect, useRef, useCallback } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { colimaApi, ColimaInstance } from "../lib/api";

const API_BASE = "http://127.0.0.1:11420";

// ===== Runtime Detection =====
const isTauri = (): boolean => !!(window as any).__TAURI_INTERNALS__;

interface TerminalTab {
  id: string;
  label: string;
  profile: string;
}

/* ===== Terminal Theme (shared) ===== */
const termTheme = {
  background: "#0D1117",
  foreground: "#E6EDF3",
  cursor: "#58A6FF",
  selectionBackground: "rgba(88, 166, 255, 0.3)",
  black: "#0D1117",
  red: "#F85149",
  green: "#3FB950",
  yellow: "#D29922",
  blue: "#58A6FF",
  magenta: "#BC8CFF",
  cyan: "#39D2C0",
  white: "#E6EDF3",
  brightBlack: "#6E7681",
  brightRed: "#F85149",
  brightGreen: "#3FB950",
  brightYellow: "#D29922",
  brightBlue: "#58A6FF",
  brightMagenta: "#BC8CFF",
  brightCyan: "#39D2C0",
  brightWhite: "#FFFFFF",
};

/* ===== Browser Mode Terminal (HTTP Polling) ===== */
function BrowserTerminalInstance({
  profile,
  active,
  sessionId,
}: {
  profile: string;
  active: boolean;
  sessionId: string;
}) {
  const termRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const mountedRef = useRef(true);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    // Generate unique session ID per mount to avoid React StrictMode race
    // (cleanup's async close from mount1 won't kill mount2's session)
    const actualSessionId = `${sessionId}-${Date.now()}`;
    if (!termRef.current || xtermRef.current) return;

    const xterm = new XTerm({
      cursorBlink: true,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      fontSize: 13,
      lineHeight: 1.4,
      theme: termTheme,
    });

    const fit = new FitAddon();
    fitRef.current = fit;
    xterm.loadAddon(fit);
    xterm.loadAddon(new WebLinksAddon());

    xterm.open(termRef.current);
    fit.fit();
    xtermRef.current = xterm;

    const resizeObserver = new ResizeObserver(() => {
      if (fitRef.current) {
        try { fitRef.current.fit(); } catch (_) { /* ignore */ }
      }
    });
    resizeObserver.observe(termRef.current);

    // Connect via HTTP API
    const connect = async () => {
      xterm.writeln("\x1b[36m● Connecting to " + profile + " (browser mode)...\x1b[0m\r\n");

      try {
        // Try to close any existing session with same ID first (handles React StrictMode double-mount)
        await fetch(`${API_BASE}/api/terminal/close`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ session_id: actualSessionId }),
        }).catch(() => {});

        const res = await fetch(`${API_BASE}/api/terminal/create`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ session_id: actualSessionId, profile }),
        });
        const data = await res.json();

        if (!mountedRef.current) return; // Component was unmounted during await

        if (!data.success) {
          throw new Error(data.error || "Failed to create session");
        }

        setConnected(true);

        // Send user input to backend
        xterm.onData(async (input) => {
          try {
            await fetch(`${API_BASE}/api/terminal/write`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ session_id: actualSessionId, data: input }),
            });
          } catch (_) { /* ignore write errors */ }
        });

        // Poll for output every 100ms
        pollingRef.current = setInterval(async () => {
          if (!mountedRef.current) return;
          try {
            const r = await fetch(`${API_BASE}/api/terminal/read?session_id=${encodeURIComponent(actualSessionId)}`);
            const d = await r.json();
            if (d.success && d.data) {
              // Normalize line endings: convert bare \n to \r\n for xterm.js
              const normalized = d.data.replace(/\r?\n/g, "\r\n");
              xterm.write(normalized);
            }
          } catch (_) { /* ignore read errors */ }
        }, 100);

      } catch (e) {
        if (!mountedRef.current) return;
        xterm.writeln(`\r\n\x1b[31m● Failed to connect: ${e}\x1b[0m`);
        xterm.writeln("\x1b[33m  Make sure the instance is running.\x1b[0m");
        setError(String(e));
      }
    };

    connect();

    return () => {
      mountedRef.current = false;
      resizeObserver.disconnect();
      // Cleanup: stop polling and close session
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
        pollingRef.current = null;
      }
      fetch(`${API_BASE}/api/terminal/close`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ session_id: actualSessionId }),
      }).catch(() => {});
      if (xtermRef.current) {
        xtermRef.current.dispose();
        xtermRef.current = null;
      }
    };
  }, [sessionId, profile]); // Only re-run on session/profile change, NOT on active change

  // Re-fit when tab becomes active
  useEffect(() => {
    if (active && fitRef.current) {
      setTimeout(() => {
        try { fitRef.current?.fit(); } catch (_) { /* ignore */ }
      }, 100);
    }
  }, [active]);

  return (
    <div style={{ position: "relative", height: "100%", display: active ? "block" : "none" }}>
      {error && !connected && (
        <div style={{
          position: "absolute", top: 12, right: 12, zIndex: 10,
          padding: "6px 12px", borderRadius: "var(--radius-md)",
          background: "rgba(248, 81, 73, 0.15)", border: "1px solid var(--accent-red)",
          color: "var(--accent-red)", fontSize: "var(--text-xs)",
        }}>
          Connection failed
        </div>
      )}
      <div ref={termRef} style={{ height: "100%", padding: 4 }} />
    </div>
  );
}

/* ===== Tauri Mode Terminal (Shell Plugin) ===== */
function TauriTerminalInstance({
  profile,
  active,
}: {
  profile: string;
  active: boolean;
}) {
  const termRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const childRef = useRef<any>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const initTerminal = useCallback(async () => {
    if (!termRef.current || xtermRef.current) return;

    const xterm = new XTerm({
      cursorBlink: true,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      fontSize: 13,
      lineHeight: 1.4,
      theme: termTheme,
    });

    const fit = new FitAddon();
    fitRef.current = fit;
    xterm.loadAddon(fit);
    xterm.loadAddon(new WebLinksAddon());

    xterm.open(termRef.current);
    fit.fit();
    xtermRef.current = xterm;

    const resizeObserver = new ResizeObserver(() => {
      if (fitRef.current && active) {
        try { fitRef.current.fit(); } catch (_) { /* ignore */ }
      }
    });
    resizeObserver.observe(termRef.current);

    try {
      xterm.writeln("\x1b[36m● Connecting to " + profile + "...\x1b[0m\r\n");

      const { Command } = await import("@tauri-apps/plugin-shell");
      const args = profile === "default" ? ["ssh"] : ["ssh", "--profile", profile];
      const cmd = Command.create("colima", args);

      cmd.on("close", (data: { code: number | null }) => {
        xterm.writeln(`\r\n\x1b[33m● Session ended (exit code: ${data.code})\x1b[0m`);
        setConnected(false);
      });

      cmd.on("error", (err: string) => {
        xterm.writeln(`\r\n\x1b[31m● Error: ${err}\x1b[0m`);
        setError(String(err));
      });

      cmd.stdout.on("data", (data: string) => xterm.write(data));
      cmd.stderr.on("data", (data: string) => xterm.write(data));

      const child = await cmd.spawn();
      childRef.current = child;
      setConnected(true);

      xterm.onData((data) => {
        if (childRef.current) childRef.current.write(data);
      });
    } catch (e) {
      xterm.writeln(`\r\n\x1b[31m● Failed to connect: ${e}\x1b[0m`);
      xterm.writeln("\x1b[33m  Make sure the instance is running.\x1b[0m");
      setError(String(e));
    }

    return () => resizeObserver.disconnect();
  }, [profile, active]);

  useEffect(() => {
    initTerminal();
    return () => {
      if (childRef.current) {
        childRef.current.kill();
        childRef.current = null;
      }
      if (xtermRef.current) {
        xtermRef.current.dispose();
        xtermRef.current = null;
      }
    };
  }, [initTerminal]);

  useEffect(() => {
    if (active && fitRef.current) {
      setTimeout(() => {
        try { fitRef.current?.fit(); } catch (_) { /* ignore */ }
      }, 100);
    }
  }, [active]);

  return (
    <div style={{ position: "relative", height: "100%", display: active ? "block" : "none" }}>
      {error && !connected && (
        <div style={{
          position: "absolute", top: 12, right: 12, zIndex: 10,
          padding: "6px 12px", borderRadius: "var(--radius-md)",
          background: "rgba(248, 81, 73, 0.15)", border: "1px solid var(--accent-red)",
          color: "var(--accent-red)", fontSize: "var(--text-xs)",
        }}>
          Connection failed
        </div>
      )}
      <div ref={termRef} style={{ height: "100%", padding: 4 }} />
    </div>
  );
}

/* ===== Main Terminal Page ===== */
export default function TerminalPage() {
  const [tabs, setTabs] = useState<TerminalTab[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [instances, setInstances] = useState<ColimaInstance[]>([]);
  const [showPicker, setShowPicker] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const list = await colimaApi.listInstances();
        setInstances(list.filter((i) => i.status === "Running"));
      } catch (_) { /* ignore */ }
    };
    load();
    const interval = setInterval(load, 10000);
    return () => clearInterval(interval);
  }, []);

  const addTab = (profile: string) => {
    const id = `term-${Date.now()}`;
    const label = profile === "default" ? "colima" : profile;
    setTabs((prev) => [...prev, { id, label, profile }]);
    setActiveTab(id);
    setShowPicker(false);
  };

  const removeTab = (id: string) => {
    setTabs((prev) => {
      const next = prev.filter((t) => t.id !== id);
      if (activeTab === id) {
        setActiveTab(next.length > 0 ? next[next.length - 1].id : null);
      }
      return next;
    });
  };

  const runningInstances = instances.filter((i) => i.status === "Running");

  return (
    <>
      <div className="content-header">
        <h1>Terminal</h1>
        <div className="content-header-actions">
          <button className="btn btn-primary" onClick={() => {
            if (runningInstances.length === 1) {
              const name = runningInstances[0].name;
              addTab(name === "colima" ? "default" : name.replace("colima-", ""));
            } else {
              setShowPicker(true);
            }
          }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
            </svg>
            New Session
          </button>
        </div>
      </div>

      {/* Tab Bar */}
      {tabs.length > 0 && (
        <div style={{
          display: "flex", alignItems: "center", gap: 0,
          borderBottom: "1px solid var(--border-primary)",
          background: "var(--bg-content)",
          paddingLeft: 12, flexShrink: 0, overflow: "auto",
        }}>
          {tabs.map((tab) => (
            <div
              key={tab.id}
              style={{
                display: "flex", alignItems: "center", gap: 6,
                padding: "8px 12px",
                borderBottom: activeTab === tab.id ? "2px solid var(--accent-blue)" : "2px solid transparent",
                color: activeTab === tab.id ? "var(--text-primary)" : "var(--text-secondary)",
                cursor: "pointer", fontSize: "var(--text-sm)",
                fontWeight: activeTab === tab.id ? 600 : 400,
                whiteSpace: "nowrap", transition: "all 150ms",
              }}
              onClick={() => setActiveTab(tab.id)}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
              </svg>
              {tab.label}
              <span
                style={{ marginLeft: 4, opacity: 0.5, cursor: "pointer", fontSize: "var(--text-xs)" }}
                onClick={(e) => { e.stopPropagation(); removeTab(tab.id); }}
              >
                ✕
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Terminal Content */}
      <div style={{ flex: 1, overflow: "hidden", background: "#0D1117" }}>
        {tabs.length === 0 ? (
          <div className="empty-state" style={{ height: "100%" }}>
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
              </svg>
            </div>
            <div className="empty-state-title">No terminal sessions</div>
            <div className="empty-state-text">
              Open a new SSH session to a running Colima instance.
            </div>
            {runningInstances.length > 0 ? (
              <button className="btn btn-primary" onClick={() => {
                if (runningInstances.length === 1) {
                  const name = runningInstances[0].name;
                  addTab(name === "colima" ? "default" : name.replace("colima-", ""));
                } else {
                  setShowPicker(true);
                }
              }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
                </svg>
                New Session
              </button>
            ) : (
              <p style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                No running instances. Start an instance first.
              </p>
            )}
          </div>
        ) : (
          tabs.map((tab) =>
            isTauri() ? (
              <TauriTerminalInstance key={tab.id} profile={tab.profile} active={activeTab === tab.id} />
            ) : (
              <BrowserTerminalInstance key={tab.id} profile={tab.profile} active={activeTab === tab.id} sessionId={tab.id} />
            )
          )
        )}
      </div>

      {/* Instance Picker Modal */}
      {showPicker && (
        <div className="modal-overlay" onClick={() => setShowPicker(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: "min(400px, 90vw)" }}>
            <div className="modal-header">
              <h2 className="modal-title">Select Instance</h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setShowPicker(false)}>✕</button>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {runningInstances.map((inst) => {
                const instProfile = inst.name === "colima" ? "default" : inst.name.replace("colima-", "");
                return (
                  <div
                    key={inst.name}
                    className="nav-item"
                    style={{ padding: "12px 16px", borderRadius: "var(--radius-md)" }}
                    onClick={() => addTab(instProfile)}
                  >
                    <div style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--status-running)", boxShadow: "0 0 6px var(--status-running)" }} />
                    <div>
                      <div style={{ fontWeight: 500 }}>{inst.name}</div>
                      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                        {inst.runtime} · {inst.arch} · {inst.cpus} CPU
                      </div>
                    </div>
                  </div>
                );
              })}
              {runningInstances.length === 0 && (
                <p style={{ textAlign: "center", color: "var(--text-muted)", padding: 20, fontSize: "var(--text-sm)" }}>
                  No running instances available.
                </p>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}

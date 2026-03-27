import React, { useState, useEffect, useCallback, Suspense } from "react";
import { colimaApi, dockerApi, systemApi, ColimaInstance, SystemInfo } from "./lib/api";
import { onToast, ToastMessage } from "./lib/globalToast";
import { WarningIcon } from "./components/Icons";
import { useSetAtom } from "jotai";
import { containersAtom, imagesAtom, dockerLoadingAtom } from "./store/dockerAtom";
import "./index.css";

// Lazy-loaded pages — each becomes a separate chunk
const Dashboard = React.lazy(() => import("./pages/Dashboard"));
const Instances = React.lazy(() => import("./pages/Instances"));
const Containers = React.lazy(() => import("./pages/Containers"));
const TerminalPage = React.lazy(() => import("./pages/Terminal"));
const Models = React.lazy(() => import("./pages/Models"));
const Images = React.lazy(() => import("./pages/Images"));
const Volumes = React.lazy(() => import("./pages/Volumes"));
const Networks = React.lazy(() => import("./pages/Networks"));
const Compose = React.lazy(() => import("./pages/Compose"));
const Kubernetes = React.lazy(() => import("./pages/Kubernetes"));
const LinuxVMs = React.lazy(() => import("./pages/LinuxVMs"));
const DockerfileGen = React.lazy(() => import("./pages/DockerfileGen"));
const Settings = React.lazy(() => import("./pages/Settings"));
const SetupWizard = React.lazy(() => import("./components/SetupWizard"));
const GettingStartedTour = React.lazy(() => import("./components/GettingStartedTour"));

type Page = "dashboard" | "instances" | "containers" | "images" | "volumes" | "networks" | "compose" | "kubernetes" | "linux-vms" | "dockerfile" | "terminal" | "models" | "settings";

const isTauri = !!(window as any).__TAURI_INTERNALS__;

// SVG icons as inline components
const Icons = {
  Dashboard: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>
    </svg>
  ),
  Server: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/>
    </svg>
  ),
  Container: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M22 8.35V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8.35A2 2 0 0 1 3.26 6.5l8-3.2a2 2 0 0 1 1.48 0l8 3.2A2 2 0 0 1 22 8.35Z"/>
      <path d="M6 18h12"/><path d="M6 14h12"/><path d="M6 10h12"/>
    </svg>
  ),
  Terminal: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
    </svg>
  ),
  Models: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
    </svg>
  ),
  Settings: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
    </svg>
  ),
  Volume: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
    </svg>
  ),
  Network: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="5" r="3"/><circle cx="5" cy="19" r="3"/><circle cx="19" cy="19" r="3"/>
      <line x1="12" y1="8" x2="5" y2="16"/><line x1="12" y1="8" x2="19" y2="16"/>
    </svg>
  ),
  Compose: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="9" height="9" rx="1"/><rect x="13" y="2" width="9" height="9" rx="1"/>
      <rect x="2" y="13" width="9" height="9" rx="1"/><rect x="13" y="13" width="9" height="9" rx="1"/>
      <line x1="6.5" y1="6.5" x2="17.5" y2="17.5"/>
    </svg>
  ),
  Kubernetes: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
    </svg>
  ),
  LinuxVM: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
    </svg>
  ),
  Dockerfile: () => (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
      <polyline points="14 2 14 8 20 8"/><line x1="12" y1="18" x2="12" y2="12"/><line x1="9" y1="15" x2="15" y2="15"/>
    </svg>
  ),
};

interface InstancesUpdatePayload {
  instances: ColimaInstance[];
  timestamp: number;
}

function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [instances, setInstances] = useState<ColimaInstance[]>([]);
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [currentTime, setCurrentTime] = useState(new Date());
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  // Global toast listener — persists across tab switches
  useEffect(() => {
    return onToast((toast) => {
      setToasts((prev) => [...prev, toast]);
      // Auto-dismiss after 5 seconds
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== toast.id));
      }, 5000);
    });
  }, []);

  // Onboarding state
  const [showWizard, setShowWizard] = useState(() => {
    return localStorage.getItem("colimaui_setup_complete") !== "true";
  });
  const [showTour, setShowTour] = useState(false);

  const refreshManual = useCallback(async () => {
    try {
      setError(null);
      const [instanceList, sysInfo] = await Promise.all([
        colimaApi.listInstances().catch(() => [] as ColimaInstance[]),
        systemApi.checkSystem().catch(() => null),
      ]);
      setInstances(instanceList);
      if (sysInfo) setSystemInfo(sysInfo);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial load
  useEffect(() => {
    refreshManual();
  }, [refreshManual]);

  // Tauri: listen to Rust poller events for real-time updates
  // Browser: connect to SSE stream for real-time updates (no polling!)
  useEffect(() => {
    if (isTauri) {
      let cleanup: (() => void) | undefined;
      import("@tauri-apps/api/event").then((mod) => {
        mod.listen<InstancesUpdatePayload>("instances-update", (event) => {
          setInstances(event.payload.instances);
          setLoading(false);
        }).then((unlisten) => {
          cleanup = unlisten;
        });
      }).catch(() => {
        // Fall back to polling if event import fails — no-op, polling will start on next render
      });
      return () => { if (cleanup) cleanup(); };
    } else {
      // Browser mode: try SSE, fall back to HTTP polling if unavailable
      let sseWorking = false;
      let pollInterval: ReturnType<typeof setInterval> | null = null;
      const es = new EventSource("http://127.0.0.1:11420/api/events");
      es.addEventListener("instances-update", (e) => {
        try {
          sseWorking = true;
          const data = JSON.parse((e as MessageEvent).data);
          setInstances(data.instances);
          setLoading(false);
        } catch { /* ignore parse errors */ }
      });
      es.onerror = () => {
        if (!sseWorking && !pollInterval) {
          es.close();
          // Fall back to polling
          pollInterval = setInterval(refreshManual, 5000);
        }
      };
      // Also refresh manually on first mount
      refreshManual();
      return () => {
        es.close();
        if (pollInterval) clearInterval(pollInterval);
      };
    }
  }, [refreshManual]);

  const setContainers = useSetAtom(containersAtom);
  const setImages = useSetAtom(imagesAtom);
  const setDockerLoading = useSetAtom(dockerLoadingAtom);

  // Global Docker State Effect
  useEffect(() => {
    if (isTauri) {
      let unlisten: (() => void) | undefined;

      // Immediate fetch via normalized API (handles field-name mapping)
      Promise.all([
        dockerApi.listContainers(true).catch(() => []),
        dockerApi.listImages().catch(() => []),
      ]).then(([c, i]) => {
        setContainers(c);
        setImages(i);
        setDockerLoading(false);
      });

      // Also subscribe to push updates for real-time sync
      import("@tauri-apps/api/event").then((mod) => {
        mod.listen<{ containers: any[], images: any[] }>("docker-state-updated", (event) => {
          // Normalize field names from bollard format
          const containers = (event.payload.containers || []).map((v: any) => ({
            Id: v.Id || v.id || v.ID || "",
            Names: v.Names || v.names || "",
            Image: v.Image || v.image || "",
            Status: v.Status || v.status || "",
            State: v.State || v.state || "",
            Ports: v.Ports || v.ports || "",
            CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
            Size: v.Size || v.size || "",
            Command: v.Command || v.command || "",
          }));
          const images = (event.payload.images || []).map((v: any) => ({
            Id: v.Id || v.id || v.ID || "",
            Repository: v.Repository || v.repository || "",
            Tag: v.Tag || v.tag || "",
            Size: v.Size || v.size || "",
            CreatedAt: v.CreatedAt || v.created_at || v.createdAt || "",
          }));
          setContainers(containers);
          setImages(images);
          setDockerLoading(false);
        }).then((fn) => { unlisten = fn; });
      });
      return () => { if (unlisten) unlisten(); };
    } else {
      // Browser mode: immediate fetch + SSE for real-time updates
      let pollInterval: ReturnType<typeof setInterval> | null = null;

      // Immediate HTTP fetch — don't wait for SSE push
      const fetchDocker = async () => {
        try {
          const [containers, images] = await Promise.all([
            dockerApi.listContainers(true),
            dockerApi.listImages(),
          ]);
          setContainers(containers);
          setImages(images);
          setDockerLoading(false);
        } catch { /* ignore */ }
      };
      fetchDocker();

      // SSE for real-time push updates (container start/stop/etc.)
      const es = new EventSource("http://127.0.0.1:11420/api/events");
      es.addEventListener("docker-state-updated", (e) => {
        try {
          const data = JSON.parse((e as MessageEvent).data);
          setContainers(data.containers);
          setImages(data.images);
          setDockerLoading(false);
        } catch { /* ignore parse errors */ }
      });
      es.onerror = () => {
        // SSE unavailable — fall back to periodic polling
        if (!pollInterval) {
          es.close();
          pollInterval = setInterval(fetchDocker, 3000);
        }
      };
      return () => {
        es.close();
        if (pollInterval) clearInterval(pollInterval);
      };
    }
  }, [setContainers, setImages, setDockerLoading]);

  // Smooth 1-second clock
  useEffect(() => {
    const clockInterval = setInterval(() => setCurrentTime(new Date()), 1000);
    return () => clearInterval(clockInterval);
  }, []);

  // Refresh system info less frequently (every 30s)
  useEffect(() => {
    const sysInterval = setInterval(async () => {
      try {
        const sysInfo = await systemApi.checkSystem();
        setSystemInfo(sysInfo);
      } catch (_) { /* ignore */ }
    }, 30000);
    return () => clearInterval(sysInterval);
  }, []);

  const navGroups: { label: string; items: { id: Page; label: string; icon: React.FC }[] }[] = [
    {
      label: "Overview",
      items: [
        { id: "dashboard", label: "Dashboard", icon: Icons.Dashboard },
        { id: "instances", label: "Instances", icon: Icons.Server },
      ],
    },
    {
      label: "Docker",
      items: [
        { id: "containers", label: "Containers", icon: Icons.Container },
        { id: "images", label: "Images", icon: Icons.Container },
        { id: "volumes", label: "Volumes", icon: Icons.Volume },
        { id: "networks", label: "Networks", icon: Icons.Network },
        { id: "compose", label: "Compose", icon: Icons.Compose },
        { id: "dockerfile", label: "Dockerfile", icon: Icons.Dockerfile },
      ],
    },
    {
      label: "Infrastructure",
      items: [
        { id: "kubernetes", label: "Kubernetes", icon: Icons.Kubernetes },
        { id: "linux-vms", label: "Linux VMs", icon: Icons.LinuxVM },
      ],
    },
    {
      label: "Tools",
      items: [
        { id: "terminal", label: "Terminal", icon: Icons.Terminal },
        { id: "models", label: "AI Models", icon: Icons.Models },
        { id: "settings", label: "Settings", icon: Icons.Settings },
      ],
    },
  ];

  const renderPage = () => {
    switch (page) {
      case "dashboard":
        return <Dashboard instances={instances} systemInfo={systemInfo} loading={loading} onNavigate={setPage} />;
      case "instances":
        return <Instances instances={instances} onRefresh={refreshManual} />;
      case "containers":
        return <Containers />;
      case "images":
        return <Images />;
      case "volumes":
        return <Volumes />;
      case "networks":
        return <Networks />;
      case "compose":
        return <Compose />;
      case "dockerfile":
        return <DockerfileGen />;
      case "kubernetes":
        return <Kubernetes />;
      case "linux-vms":
        return <LinuxVMs />;
      case "terminal":
        return <TerminalPage />;
      case "models":
        return <Models />;
      case "settings":
        return <Settings systemInfo={systemInfo} />;
      default:
        return <Dashboard instances={instances} systemInfo={systemInfo} loading={loading} onNavigate={setPage} />;
    }
  };

  const formatTime = () => currentTime.toLocaleTimeString();

  return (
    <div className={`app-layout${isTauri ? " tauri-app" : ""}`}>
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <img src="/colima_icon.png" alt="ColimaUI" className="sidebar-logo" />
          <h1 className="sidebar-title">ColimaUI</h1>
        </div>

        <nav className="sidebar-nav" data-tour-id="sidebar-nav">
          {navGroups.map((group) => (
            <div key={group.label} className="nav-section">
              <div className="nav-section-label">{group.label}</div>
              {group.items.map((item) => (
                <div
                  key={item.id}
                  className={`nav-item ${page === item.id ? "active" : ""}`}
                  onClick={() => setPage(item.id)}
                  data-tour-id={`nav-${item.id}`}
                >
                  <item.icon />
                  <span>{item.label}</span>
                </div>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div
            className="nav-item"
            style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", cursor: "pointer" }}
            onClick={() => {
              setShowTour(true);
              localStorage.removeItem("colimaui_tour_complete");
            }}
            data-tooltip="Restart tour"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M2 3h6a4 4 0 0 1 4 4v14"/>
              <path d="M22 3h-6a4 4 0 0 0-4 4v14"/>
              <polyline points="6 7 2 3 6 -1"/>
            </svg>
            <span>Tour Guide</span>
          </div>
          <div
            className="nav-item"
            style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", cursor: "default" }}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
            </svg>
            <span>{formatTime()}</span>
          </div>
          <div
            className="nav-item"
            style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", cursor: "default" }}
          >
            {systemInfo?.colima_version
              ? `Colima v${systemInfo.colima_version.split("\n")[0].replace(/.*version\s*/i, "")}`
              : "Colima not detected"}
          </div>
        </div>
      </aside>

      {/* Main Content */}
      <main className="main-content">
        {error && (
          <div
            style={{
              padding: "8px 24px",
              background: "rgba(248, 81, 73, 0.1)",
              color: "var(--accent-red)",
              fontSize: "var(--text-sm)",
              borderBottom: "1px solid var(--border-primary)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
            }}
          >
            <span style={{ display: "flex", alignItems: "center", gap: 6 }}><WarningIcon size={14} /> {error}</span>
            <button
              className="btn btn-ghost"
              style={{ fontSize: "var(--text-xs)", padding: "2px 8px" }}
              onClick={() => setError(null)}
            >
              Dismiss
            </button>
          </div>
        )}
        <Suspense fallback={<div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "50vh" }}><div className="spinner" /></div>}>
          {renderPage()}
        </Suspense>
      </main>

      {/* Global Toast Notifications */}
      {toasts.length > 0 && (
        <div style={{ position: "fixed", top: 16, right: 16, zIndex: 99999, display: "flex", flexDirection: "column", gap: 8, maxWidth: 420 }}>
          {toasts.map((toast) => (
            <div key={toast.id} style={{
              padding: "12px 20px",
              borderRadius: "var(--radius-md)",
              background: toast.type === "success" ? "#1a2e1a" : toast.type === "error" ? "#2e1a1a" : "#1a1a2e",
              border: `1px solid ${toast.type === "success" ? "var(--accent-green)" : toast.type === "error" ? "var(--accent-red)" : "var(--accent-blue)"}`,
              color: toast.type === "success" ? "var(--accent-green)" : toast.type === "error" ? "var(--accent-red)" : "var(--accent-blue)",
              fontSize: "var(--text-sm)", fontWeight: 500,
              boxShadow: "0 8px 32px rgba(0,0,0,0.6)",
              backdropFilter: "blur(12px)",
              display: "flex", alignItems: "center", gap: 8,
              animation: "fadeInSlide 0.3s ease",
              cursor: "pointer",
            }} onClick={() => setToasts((prev) => prev.filter((t) => t.id !== toast.id))}>
              {toast.type === "success" ? "✓" : toast.type === "error" ? "✕" : "ℹ"} {toast.text}
            </div>
          ))}
        </div>
      )}

      {/* Setup Wizard — shows on first launch */}
      {showWizard && (
        <SetupWizard
          systemInfo={systemInfo}
          onComplete={() => {
            setShowWizard(false);
            localStorage.setItem("colimaui_setup_complete", "true");
            // Start tour after wizard
            if (localStorage.getItem("colimaui_tour_complete") !== "true") {
              setTimeout(() => setShowTour(true), 300);
            }
          }}
          onSkip={() => {
            setShowWizard(false);
            localStorage.setItem("colimaui_setup_complete", "true");
            if (localStorage.getItem("colimaui_tour_complete") !== "true") {
              setTimeout(() => setShowTour(true), 300);
            }
          }}
        />
      )}

      {/* Getting Started Tour — shows after wizard */}
      {showTour && (
        <GettingStartedTour
          onComplete={() => {
            setShowTour(false);
            localStorage.setItem("colimaui_tour_complete", "true");
          }}
        />
      )}
    </div>
  );
}

export default App;

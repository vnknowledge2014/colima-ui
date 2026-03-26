import { useState, useEffect, useCallback } from "react";
import { systemApi, colimaApi, SystemInfo, PlatformInfo } from "../lib/api";
import { SnowflakeIcon, GearIcon, PackageIcon, MonitorIcon, RocketIcon, HomebrewIcon, WarningIcon } from "./Icons";

interface SetupWizardProps {
  systemInfo: SystemInfo | null;
  onComplete: () => void;
  onSkip: () => void;
}

type DepName = "homebrew" | "colima" | "docker" | "lima";
type DepStatus = "checking" | "installed" | "missing" | "installing" | "failed";
type InstallMethod = "brew" | "apt" | "nix" | "wsl-brew" | "manual";

interface DepState {
  name: DepName;
  label: string;
  desc: string;
  icon: React.ReactNode;
  status: DepStatus;
  version: string;
}

const STEPS = ["Welcome", "Dependencies", "Quick Setup", "Complete"];

const OS_LABELS: Record<string, { label: string; icon: React.ReactNode }> = {
  macos: { label: "macOS", icon: <MonitorIcon size={16} /> },
  linux: { label: "Linux", icon: <MonitorIcon size={16} /> },
  windows: { label: "Windows", icon: <MonitorIcon size={16} /> },
};

const METHOD_LABELS: Record<string, { label: string; icon: React.ReactNode; desc: string }> = {
  brew: { label: "Homebrew", icon: <HomebrewIcon size={16} />, desc: "Recommended for macOS & Linux" },
  apt: { label: "APT", icon: <PackageIcon size={16} />, desc: "Debian/Ubuntu package manager" },
  nix: { label: "Nix", icon: <SnowflakeIcon size={16} />, desc: "Reproducible package manager" },
  "wsl-brew": { label: "WSL + Homebrew", icon: <MonitorIcon size={16} />, desc: "Install via Windows Subsystem for Linux" },
  manual: { label: "Manual", icon: <GearIcon size={16} />, desc: "Download and install manually" },
};

export default function SetupWizard({ systemInfo, onComplete, onSkip }: SetupWizardProps) {
  const [step, setStep] = useState(0);
  const [platform, setPlatform] = useState<PlatformInfo | null>(null);
  const [installMethod, setInstallMethod] = useState<InstallMethod>("brew");
  const [deps, setDeps] = useState<DepState[]>([
    { name: "homebrew", label: "Homebrew", desc: "Package manager for macOS", icon: <HomebrewIcon size={16} />, status: "checking", version: "" },
    { name: "colima", label: "Colima", desc: "Container runtime manager", icon: <PackageIcon size={16} />, status: "checking", version: "" },
    { name: "docker", label: "Docker CLI", desc: "Container engine client", icon: <PackageIcon size={16} />, status: "checking", version: "" },
    { name: "lima", label: "Lima", desc: "Linux virtual machine manager", icon: <MonitorIcon size={16} />, status: "checking", version: "" },
  ]);
  const [autostart, setAutostart] = useState(true);
  const [createInstance, setCreateInstance] = useState(true);
  const [settingUp, setSettingUp] = useState(false);
  const [setupLog, setSetupLog] = useState("");

  // Fetch platform info on mount
  useEffect(() => {
    (async () => {
      try {
        const p = await systemApi.getPlatform();
        setPlatform(p);
        // Auto-select best install method
        if (p.os === "windows" && p.wsl_available) {
          setInstallMethod("wsl-brew");
        } else if (p.os === "linux") {
          const hasBrew = p.package_managers.find(pm => pm.name === "brew")?.available;
          const hasApt = p.package_managers.find(pm => pm.name === "apt")?.available;
          if (hasBrew) setInstallMethod("brew");
          else if (hasApt) setInstallMethod("apt");
          else setInstallMethod("manual");
        } else {
          setInstallMethod("brew");
        }
      } catch {
        // Fallback — assume macOS
        setPlatform({ os: "macos", arch: "aarch64", wsl: false, wsl_available: false, package_managers: [] });
      }
    })();
  }, []);

  // Check dependencies on mount / re-check
  const checkDeps = useCallback(async () => {
    const updated = [...deps];

    // Check Homebrew / package manager
    try {
      const brew = await systemApi.checkHomebrew();
      updated[0] = { ...updated[0], status: brew.installed ? "installed" : "missing", version: brew.version };
    } catch {
      if (systemInfo?.colima_installed || systemInfo?.lima_installed) {
        updated[0] = { ...updated[0], status: "installed", version: "" };
      } else {
        updated[0] = { ...updated[0], status: "missing" };
      }
    }

    // Use systemInfo for deps
    if (systemInfo) {
      updated[1] = {
        ...updated[1],
        status: systemInfo.colima_installed ? "installed" : "missing",
        version: systemInfo.colima_version ? systemInfo.colima_version.split("\n")[0] : "",
      };
      updated[2] = {
        ...updated[2],
        status: systemInfo.docker_installed ? "installed" : "missing",
        version: systemInfo.docker_version ? systemInfo.docker_version.split("\n")[0] : "",
      };
      updated[3] = {
        ...updated[3],
        status: systemInfo.lima_installed ? "installed" : "missing",
        version: systemInfo.lima_version ? systemInfo.lima_version.split("\n")[0] : "",
      };
    } else {
      for (let i = 1; i <= 3; i++) {
        updated[i] = { ...updated[i], status: "missing" };
      }
    }

    // Update first row to reflect selected package manager status
    if (platform) {
      const pm = platform.package_managers.find(p => p.name === (installMethod === "wsl-brew" ? "brew" : installMethod));
      if (pm) {
        const methodInfo = METHOD_LABELS[installMethod];
        updated[0] = {
          ...updated[0],
          label: methodInfo?.label || "Package Manager",
          desc: methodInfo?.desc || "Package manager",
          icon: methodInfo?.icon || <PackageIcon size={16} />,
          status: pm.available ? "installed" : "missing",
          version: pm.version,
        };
      }
    } else if (installMethod === "manual") {
      updated[0] = { ...updated[0], label: "Manual", icon: <GearIcon size={16} />, status: "installed", desc: "Download and install manually" };
    }

    setDeps(updated);
  }, [systemInfo, installMethod]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    checkDeps();
  }, [checkDeps]);

  const missingDeps = deps.filter(d => d.status === "missing");
  const allInstalled = deps.every(d => d.status === "installed");
  const isInstalling = deps.some(d => d.status === "installing");

  // Get available methods for current platform
  const getAvailableMethods = (): InstallMethod[] => {
    if (!platform) return ["brew", "manual"];
    const methods: InstallMethod[] = [];

    if (platform.os === "macos") {
      methods.push("brew", "nix", "manual");
    } else if (platform.os === "linux") {
      methods.push("brew");
      if (platform.package_managers.find(pm => pm.name === "apt")?.available) {
        methods.push("apt");
      }
      methods.push("nix", "manual");
    } else if (platform.os === "windows") {
      if (platform.wsl_available) methods.push("wsl-brew");
      methods.push("manual");
    }
    return methods;
  };

  const handleInstallAll = async () => {
    if (installMethod === "manual") return;

    for (let i = 1; i < deps.length; i++) {
      if (deps[i].status !== "missing") continue;
      const depName = deps[i].name as "colima" | "docker" | "lima";

      setDeps(prev => {
        const updated = [...prev];
        updated[i] = { ...updated[i], status: "installing" };
        return updated;
      });

      try {
        const result = await systemApi.installDep(depName, installMethod);
        setDeps(prev => {
          const updated = [...prev];
          updated[i] = { ...updated[i], status: result.success ? "installed" : "failed" };
          return updated;
        });
      } catch {
        setDeps(prev => {
          const updated = [...prev];
          updated[i] = { ...updated[i], status: "failed" };
          return updated;
        });
      }
    }
  };

  const handleInstallSingle = async (index: number) => {
    if (index === 0 || installMethod === "manual") return;
    const depName = deps[index].name as "colima" | "docker" | "lima";

    setDeps(prev => {
      const updated = [...prev];
      updated[index] = { ...updated[index], status: "installing" };
      return updated;
    });

    try {
      const result = await systemApi.installDep(depName, installMethod);
      setDeps(prev => {
        const updated = [...prev];
        updated[index] = { ...updated[index], status: result.success ? "installed" : "failed" };
        return updated;
      });
    } catch {
      setDeps(prev => {
        const updated = [...prev];
        updated[index] = { ...updated[index], status: "failed" };
        return updated;
      });
    }
  };

  const handleQuickSetup = async () => {
    setSettingUp(true);
    setSetupLog("Configuring...");

    try {
      if (autostart) {
        setSetupLog("Setting up auto-start on boot...");
        try {
          await systemApi.configureAutostart(true);
          setSetupLog("✓ Auto-start configured");
        } catch {
          setSetupLog("⚠ Could not configure auto-start (will need manual setup)");
        }
      }

      if (createInstance) {
        setSetupLog(l => l + "\nCreating default Colima instance...");
        try {
          await colimaApi.startInstance({
            profile: "default",
            runtime: "docker",
            vm_type: "qemu",
            cpus: 2,
            memory: 4,
            disk: 60,
            kubernetes: false,
            kubernetes_version: "",
            arch: "aarch64",
            mount_type: "",
            mounts: [],
            dns: [],
            network_address: false,
          });
          setSetupLog(l => l + "\n✓ Default instance created and starting");
        } catch {
          setSetupLog(l => l + "\n⚠ Instance may already exist or Colima is not available yet");
        }
      }

      setSetupLog(l => l + "\n\n✓ Setup complete!");
      setTimeout(() => setStep(3), 1000);
    } finally {
      setSettingUp(false);
    }
  };

  const osInfo = OS_LABELS[platform?.os || "macos"] || OS_LABELS.macos;
  const availableMethods = getAvailableMethods();

  // Manual install instructions
  const getManualInstructions = () => {
    if (platform?.os === "windows") {
      return (
        <div style={{
          padding: 12, background: "var(--bg-content)", borderRadius: "var(--radius-md)",
          fontSize: "var(--text-xs)", color: "var(--text-secondary)", marginBottom: 16,
          fontFamily: "var(--font-mono)", lineHeight: 1.8,
        }}>
          <div style={{ color: "var(--accent-blue)", fontWeight: 600, marginBottom: 4 }}>Windows + WSL Setup:</div>
          1. Install WSL: <code>wsl --install -d Ubuntu</code><br/>
          2. Open Ubuntu terminal<br/>
          3. Install Homebrew: <code>/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"</code><br/>
          4. Install deps: <code>brew install colima docker lima</code>
        </div>
      );
    }
    return (
      <div style={{
        padding: 12, background: "var(--bg-content)", borderRadius: "var(--radius-md)",
        fontSize: "var(--text-xs)", color: "var(--text-secondary)", marginBottom: 16,
        fontFamily: "var(--font-mono)", lineHeight: 1.8,
      }}>
        <div style={{ color: "var(--accent-blue)", fontWeight: 600, marginBottom: 4 }}>Manual Installation:</div>
        • Colima: <code>curl -LO https://github.com/abiosoft/colima/releases/latest</code><br/>
        • Docker: <code>https://docs.docker.com/engine/install/</code><br/>
        • Lima: <code>https://lima-vm.io/docs/installation/</code>
      </div>
    );
  };

  const renderStep = () => {
    switch (step) {
      case 0: // Welcome
        return (
          <>
            <div className="wizard-logo">C</div>
            <div className="wizard-title">Welcome to ColimaUI</div>
            <div className="wizard-subtitle">
              Your cross-platform graphical interface for managing Colima instances,
              Docker containers, Kubernetes clusters, and Linux VMs.
              <br /><br />
              {platform && (
                <span style={{
                  display: "inline-flex", alignItems: "center", gap: 6,
                  padding: "4px 12px", background: "rgba(88,166,255,0.1)",
                  borderRadius: 20, border: "1px solid rgba(88,166,255,0.2)",
                  fontSize: "var(--text-xs)",
                }}>
                  <span>{osInfo.icon}</span>
                  <span>Detected: <strong>{osInfo.label}</strong> ({platform.arch})</span>
                  {platform.wsl && <span style={{ color: "var(--accent-yellow)" }}>• WSL</span>}
                </span>
              )}
            </div>
            <div className="wizard-actions" style={{ justifyContent: "center" }}>
              <button className="btn btn-ghost" onClick={onSkip}>Skip Setup</button>
              <button className="btn btn-primary" onClick={() => setStep(1)} style={{ padding: "10px 32px", fontSize: "var(--text-base)" }}>
                Get Started →
              </button>
            </div>
          </>
        );

      case 1: // Dependencies
        return (
          <>
            <h2 style={{ fontSize: "var(--text-xl)", fontWeight: 700, marginBottom: 8 }}>
              System Dependencies
            </h2>
            <p style={{ color: "var(--text-muted)", fontSize: "var(--text-sm)", marginBottom: 16 }}>
              ColimaUI requires these tools. Choose your preferred installation method.
            </p>

            {/* Install method selector */}
            {availableMethods.length > 1 && (
              <div style={{
                display: "flex", gap: 6, marginBottom: 20, flexWrap: "wrap",
              }}>
                {availableMethods.map(m => {
                  const info = METHOD_LABELS[m];
                  const isActive = installMethod === m;
                  const pmInfo = platform?.package_managers.find(pm => pm.name === m);
                  const isManual = (m as string) === "manual";
                  const isAvailable = isManual || pmInfo?.available;
                  return (
                    <button
                      key={m}
                      className={`btn ${isActive ? "btn-primary" : "btn-ghost"}`}
                      style={{
                        fontSize: "var(--text-xs)", padding: "6px 12px",
                        opacity: isAvailable ? 1 : 0.5,
                        display: "flex", alignItems: "center", gap: 4,
                      }}
                      onClick={() => setInstallMethod(m)}
                      disabled={!isAvailable && !isManual}
                      data-tooltip={info.desc}
                    >
                      <span>{info.icon}</span>
                      <span>{info.label}</span>
                      {!isAvailable && !isManual && (
                        <span style={{ fontSize: 9, opacity: 0.6 }}>(not found)</span>
                      )}
                    </button>
                  );
                })}
              </div>
            )}

            {/* WSL notice for Windows */}
            {platform?.os === "windows" && !platform.wsl_available && (
              <div style={{
                padding: 12, background: "rgba(248, 81, 73, 0.1)", borderRadius: "var(--radius-md)",
                border: "1px solid rgba(248, 81, 73, 0.3)", marginBottom: 16,
                fontSize: "var(--text-xs)", color: "var(--accent-red)",
              }}>
                <WarningIcon size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> WSL is required to run Colima on Windows. Install it with:
                <code style={{ display: "block", marginTop: 4, padding: "4px 8px", background: "var(--bg-content)", borderRadius: 4 }}>
                  wsl --install -d Ubuntu
                </code>
              </div>
            )}

            {/* Dependency rows */}
            <div className="dep-list">
              {deps.map((dep, i) => (
                <div key={dep.name} className="dep-row">
                  <div className="dep-info">
                    <div className={`dep-icon ${dep.status === "installed" ? "installed" : dep.status === "installing" ? "installing" : "missing"}`}>
                      {dep.icon}
                    </div>
                    <div>
                      <div className="dep-name">{dep.label}</div>
                      <div className="dep-desc">{dep.desc}</div>
                    </div>
                  </div>
                  <div className="dep-status">
                    {dep.version && <span className="dep-version">{dep.version}</span>}
                    {dep.status === "installed" && (
                      <span className="badge badge-running">
                        <span className="badge-dot" style={{ animation: "none" }} />
                        Installed
                      </span>
                    )}
                    {dep.status === "missing" && (
                      <>
                        <span className="badge badge-stopped">Missing</span>
                        {i === 0 ? (
                          installMethod === "brew" || installMethod === "wsl-brew" ? (
                            <a href="https://brew.sh" target="_blank" rel="noopener noreferrer"
                              className="btn btn-ghost"
                              style={{ fontSize: "var(--text-xs)", padding: "2px 8px", textDecoration: "none" }}>
                              Install ↗
                            </a>
                          ) : null
                        ) : installMethod !== "manual" ? (
                          <button
                            className="btn btn-primary"
                            style={{ fontSize: "var(--text-xs)", padding: "4px 10px" }}
                            onClick={() => handleInstallSingle(i)}>
                            Install
                          </button>
                        ) : null}
                      </>
                    )}
                    {dep.status === "installing" && (
                      <span className="badge" style={{ background: "rgba(88,166,255,0.15)", color: "var(--accent-blue)" }}>
                        <div className="spinner" style={{ width: 10, height: 10, borderWidth: 1.5 }} />
                        Installing...
                      </span>
                    )}
                    {dep.status === "checking" && (
                      <span className="badge" style={{ background: "rgba(139,148,158,0.15)", color: "var(--text-muted)" }}>
                        <div className="spinner" style={{ width: 10, height: 10, borderWidth: 1.5 }} />
                        Checking
                      </span>
                    )}
                    {dep.status === "failed" && (
                      <span className="badge badge-stopped">Failed</span>
                    )}
                  </div>
                </div>
              ))}
            </div>

            {/* Install all button */}
            {missingDeps.length > 0 && missingDeps.some(d => d.name !== "homebrew") && installMethod !== "manual" && (
              <button
                className="btn btn-primary"
                style={{ width: "100%", padding: "10px", marginBottom: 16 }}
                onClick={handleInstallAll}
                disabled={isInstalling}
              >
                {isInstalling ? (
                  <><div className="spinner" style={{ width: 14, height: 14 }} /> Installing via {METHOD_LABELS[installMethod]?.label}...</>
                ) : (
                  `Install All Missing via ${METHOD_LABELS[installMethod]?.label} (${missingDeps.filter(d => d.name !== "homebrew").length})`
                )}
              </button>
            )}

            {/* Manual instructions */}
            {installMethod === "manual" && getManualInstructions()}

            <div className="wizard-actions">
              <button className="btn btn-ghost" onClick={() => setStep(0)}>← Back</button>
              <button className="btn btn-ghost" onClick={checkDeps} disabled={isInstalling}>↻ Re-check</button>
              <button className="btn btn-primary" onClick={() => setStep(2)} disabled={isInstalling}>
                {allInstalled ? "Next →" : "Continue Anyway →"}
              </button>
            </div>
          </>
        );

      case 2: // Quick Setup
        return (
          <>
            <h2 style={{ fontSize: "var(--text-xl)", fontWeight: 700, marginBottom: 8 }}>
              Quick Setup
            </h2>
            <p style={{ color: "var(--text-muted)", fontSize: "var(--text-sm)", marginBottom: 24 }}>
              Configure your environment for the best experience.
            </p>

            <div className="wizard-setup-grid">
              <div className="wizard-option">
                <div className="wizard-option-info">
                  <span className="wizard-option-label"><RocketIcon size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> Auto-start Colima on boot</span>
                  <span className="wizard-option-desc">
                    {platform?.os === "macos" && "Uses macOS LaunchAgent to start Colima automatically"}
                    {platform?.os === "linux" && (platform?.wsl ? "Starts Colima when WSL boots" : "Uses systemd service to start Colima automatically")}
                    {platform?.os === "windows" && "Uses Task Scheduler + WSL to start Colima automatically"}
                    {!platform && "Colima will start automatically when your system restarts"}
                  </span>
                </div>
                <label className="toggle-switch">
                  <input type="checkbox" checked={autostart} onChange={e => setAutostart(e.target.checked)} />
                  <span className="toggle-slider" />
                </label>
              </div>

              <div className="wizard-option">
                <div className="wizard-option-info">
                  <span className="wizard-option-label"><PackageIcon size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> Create default instance</span>
                  <span className="wizard-option-desc">
                    2 CPUs · 4 GB RAM · 60 GB Disk · Docker runtime
                  </span>
                </div>
                <label className="toggle-switch">
                  <input type="checkbox" checked={createInstance} onChange={e => setCreateInstance(e.target.checked)} />
                  <span className="toggle-slider" />
                </label>
              </div>
            </div>

            {setupLog && (
              <div style={{
                padding: 12, background: "var(--bg-content)", borderRadius: "var(--radius-md)",
                fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)",
                color: "var(--text-secondary)", marginBottom: 16, whiteSpace: "pre-wrap",
                maxHeight: 120, overflow: "auto",
              }}>
                {setupLog}
              </div>
            )}

            <div className="wizard-actions">
              <button className="btn btn-ghost" onClick={() => setStep(1)} disabled={settingUp}>← Back</button>
              <button className="btn btn-ghost" onClick={() => setStep(3)} disabled={settingUp}>Skip</button>
              <button
                className="btn btn-primary"
                onClick={handleQuickSetup}
                disabled={settingUp}
                style={{ padding: "10px 24px" }}
              >
                {settingUp ? (
                  <><div className="spinner" style={{ width: 14, height: 14 }} /> Setting up...</>
                ) : (
                  "Apply & Continue →"
                )}
              </button>
            </div>
          </>
        );

      case 3: // Complete
        return (
          <>
            <div className="wizard-success-icon">
              <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </div>
            <div className="wizard-title" style={{ fontSize: "var(--text-xl)" }}>You're All Set!</div>
            <div className="wizard-subtitle">
              ColimaUI is ready to use on <strong>{osInfo.label}</strong>. You can manage your
              Colima instances, Docker containers, and much more from the dashboard.
            </div>
            <div className="wizard-actions" style={{ justifyContent: "center" }}>
              <button
                className="btn btn-primary"
                onClick={onComplete}
                style={{ padding: "12px 40px", fontSize: "var(--text-base)" }}
              >
                Enter ColimaUI →
              </button>
            </div>
          </>
        );

      default:
        return null;
    }
  };

  return (
    <div className="wizard-overlay">
      <div className="wizard-card">
        <div className="wizard-progress">
          {STEPS.map((_, i) => (
            <div
              key={i}
              className={`wizard-progress-dot ${i === step ? "active" : i < step ? "done" : ""}`}
            />
          ))}
        </div>
        {renderStep()}
      </div>
    </div>
  );
}

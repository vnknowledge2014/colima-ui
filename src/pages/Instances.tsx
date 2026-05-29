import { useState, useCallback, useEffect } from "react";
import { colimaApi, ColimaInstance, StartConfig, kindApi, systemApi, HostSpecs, aiApi } from "../lib/api";
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

/* ===== Instance Profile Presets ===== */
interface InstancePreset {
  id: string;
  label: string;
  description: string;
  icon: string;
  cpus: number;
  memory: number;
  disk: number;
  runtime: string;
  color: string;
  kubernetes?: boolean;
  network_address?: boolean;
}

// ===== SVG Icons for Presets =====
const PresetIcons: Record<string, React.FC<{ size?: number; color?: string }>> = {
  minimal: ({ size = 18, color = "currentColor" }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
    </svg>
  ),
  development: ({ size = 18, color = "currentColor" }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
    </svg>
  ),
  standard: ({ size = 18, color = "currentColor" }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
    </svg>
  ),
  power: ({ size = 18, color = "currentColor" }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/>
    </svg>
  ),
  kubernetes: ({ size = 18, color = "currentColor" }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10"/>
      <line x1="12" y1="2" x2="12" y2="6"/><line x1="12" y1="18" x2="12" y2="22"/>
      <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"/><line x1="16.24" y1="16.24" x2="19.07" y2="19.07"/>
      <line x1="2" y1="12" x2="6" y2="12"/><line x1="18" y1="12" x2="22" y2="12"/>
      <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"/><line x1="16.24" y1="7.76" x2="19.07" y2="4.93"/>
    </svg>
  ),
  custom: ({ size = 18, color = "currentColor" }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
      <circle cx="12" cy="12" r="3"/>
    </svg>
  ),
};

const CUSTOM_PRESETS_KEY = "colima-ui-custom-presets";
const DETECTED_PRESETS_KEY = "colima-ui-detected-presets";
const DETECTED_HOST_KEY = "colima-ui-detected-host";
const LAST_PROFILE_KEY_PREFIX = "colima-ui-last-profile-";

function loadCustomPresets(): InstancePreset[] {
  try {
    const raw = localStorage.getItem(CUSTOM_PRESETS_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch { return []; }
}

function saveCustomPresets(presets: InstancePreset[]) {
  localStorage.setItem(CUSTOM_PRESETS_KEY, JSON.stringify(presets));
}

const CUSTOM_COLORS = [
  "#e06c75", "#e5c07b", "#61afef", "#c678dd", "#56b6c2",
  "#d19a66", "#98c379", "#be5046", "#61afef", "#abb2bf",
];

function loadDetectedPresets(): InstancePreset[] | null {
  try {
    const raw = localStorage.getItem(DETECTED_PRESETS_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}
function saveDetectedPresets(p: InstancePreset[]) {
  localStorage.setItem(DETECTED_PRESETS_KEY, JSON.stringify(p));
}
function loadDetectedHostInfo(): HostSpecs | null {
  try {
    const raw = localStorage.getItem(DETECTED_HOST_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}
function saveDetectedHostInfo(h: HostSpecs) {
  localStorage.setItem(DETECTED_HOST_KEY, JSON.stringify(h));
}

function loadLastUsedProfile(instanceId: string): string | null {
  try { return localStorage.getItem(LAST_PROFILE_KEY_PREFIX + instanceId) ?? null; } catch { return null; }
}
function saveLastUsedProfile(instanceId: string, presetId: string) {
  try { localStorage.setItem(LAST_PROFILE_KEY_PREFIX + instanceId, presetId); } catch { /* noop */ }
}

const DEFAULT_PRESETS: InstancePreset[] = [
  { id: "minimal", label: "Minimal", description: "Nhẹ, tiết kiệm tài nguyên", icon: "minimal", cpus: 1, memory: 1, disk: 20, runtime: "docker", color: "var(--accent-green)" },
  { id: "development", label: "Development", description: "Phát triển hàng ngày", icon: "development", cpus: 2, memory: 4, disk: 60, runtime: "docker", color: "var(--accent-blue)" },
  { id: "standard", label: "Standard", description: "Dùng chung, cân bằng", icon: "standard", cpus: 4, memory: 8, disk: 100, runtime: "docker", color: "var(--accent-purple)" },
  { id: "power", label: "Power", description: "Build & CI nặng", icon: "power", cpus: 8, memory: 16, disk: 200, runtime: "docker", color: "var(--accent-orange)" },
  { id: "kubernetes", label: "Kubernetes", description: "K3s cluster local", icon: "kubernetes", cpus: 4, memory: 8, disk: 100, runtime: "docker", color: "#a78bfa" },
];

/** Generate optimal presets based on host hardware specs */
function generateOptimalPresets(specs: HostSpecs): InstancePreset[] {
  const { cpu_cores: cpus, memory_gib: mem, disk_free_gib: diskFree } = specs;
  // Use at most 80% of free disk for the largest profile
  const diskBudget = Math.max(diskFree > 0 ? Math.floor(diskFree * 0.8) : 200, 20);

  // Clamp helper
  const clamp = (v: number, min: number, max: number) => Math.max(min, Math.min(max, v));
  // Round to nearest even number for memory
  const roundMem = (v: number) => Math.max(1, Math.round(v));

  return [
    {
      id: "minimal", label: "Minimal", description: "Nhẹ, tiết kiệm tài nguyên", icon: "minimal",
      cpus: clamp(Math.floor(cpus * 0.15), 1, 4),
      memory: roundMem(mem * 0.1),
      disk: clamp(Math.floor(diskBudget * 0.1), 10, 30),
      runtime: "docker", color: "var(--accent-green)",
    },
    {
      id: "development", label: "Development", description: "Phát triển hàng ngày", icon: "development",
      cpus: clamp(Math.floor(cpus * 0.25), 1, 8),
      memory: roundMem(mem * 0.25),
      disk: clamp(Math.floor(diskBudget * 0.25), 20, 100),
      runtime: "docker", color: "var(--accent-blue)",
    },
    {
      id: "standard", label: "Standard", description: "Dùng chung, cân bằng", icon: "standard",
      cpus: clamp(Math.floor(cpus * 0.5), 2, 16),
      memory: roundMem(mem * 0.5),
      disk: clamp(Math.floor(diskBudget * 0.4), 40, 200),
      runtime: "docker", color: "var(--accent-purple)",
    },
    {
      id: "power", label: "Power", description: "Build & CI nặng", icon: "power",
      cpus: clamp(Math.floor(cpus * 0.75), 4, 32),
      memory: roundMem(mem * 0.75),
      disk: clamp(Math.floor(diskBudget * 0.6), 60, 500),
      runtime: "docker", color: "var(--accent-orange)",
    },
    {
      id: "kubernetes", label: "Kubernetes", description: "K3s cluster local", icon: "kubernetes",
      cpus: clamp(Math.floor(cpus * 0.5), 2, 16),
      memory: roundMem(mem * 0.5),
      disk: clamp(Math.floor(diskBudget * 0.4), 40, 200),
      runtime: "docker", color: "#a78bfa",
    },
  ];
}

/* ===== Create Instance Dialog ===== */
function CreateInstanceDialog({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null);
  const [presets, setPresets] = useState<InstancePreset[]>(() => loadDetectedPresets() ?? DEFAULT_PRESETS);
  const [customPresets, setCustomPresets] = useState<InstancePreset[]>(loadCustomPresets);
  const [hostInfo, setHostInfo] = useState<HostSpecs | null>(loadDetectedHostInfo);
  const [detecting, setDetecting] = useState(false);
  const [aiOptimizing, setAiOptimizing] = useState(false);
  const [aiOptimized, setAiOptimized] = useState(false);
  const [showSaveForm, setShowSaveForm] = useState(false);
  const [saveLabel, setSaveLabel] = useState("");
  const [config, setConfig] = useState<StartConfig>({
    profile: "default", runtime: "docker", cpus: 2, memory: 2, disk: 60, vm_type: "vz",
    kubernetes: false, kubernetes_version: "", arch: "", mount_type: "", mounts: [], dns: [], network_address: false,
  });
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Persist presets/hostInfo so they survive dialog re-open
  const updatePresets = (p: InstancePreset[]) => { setPresets(p); saveDetectedPresets(p); };
  const updateHostInfo = (h: HostSpecs) => { setHostInfo(h); saveDetectedHostInfo(h); };

  const handleAutoDetect = async () => {
    setDetecting(true);
    setAiOptimized(false);
    try {
      const specs = await systemApi.hostSpecs();
      updateHostInfo(specs);

      // Step 1: Apply math-based presets immediately
      const mathPresets = generateOptimalPresets(specs);
      updatePresets(mathPresets);
      globalToast("success", `Detected: ${specs.cpu_cores} CPUs · ${specs.memory_gib} GiB RAM · ${specs.disk_free_gib} GiB free`);
      setDetecting(false);

      // Step 2: Ask AI to optimize (if configured)
      const apiKey = localStorage.getItem("ai_api_key") || "";
      const provider = localStorage.getItem("ai_provider") || "anthropic";
      const model = localStorage.getItem("ai_model") || "";
      const endpoint = localStorage.getItem("ai_endpoint") || "";
      const hasAi = !!(apiKey || provider === "ollama-local");

      if (hasAi && model) {
        setAiOptimizing(true);
        try {
          const prompt = `You are a macOS VM resource allocation expert. Given these host specs:
- CPU cores: ${specs.cpu_cores}
- Total RAM: ${specs.memory_gib} GiB
- Free disk: ${specs.disk_free_gib} GiB (total: ${specs.disk_total_gib} GiB)
- Architecture: ${specs.arch}
- Model: ${specs.model || "unknown"}

Return ONLY a valid JSON object (no markdown, no explanation) with optimized Colima VM settings for 5 profiles.
Rules:
- Never exceed 80% of host CPU/RAM
- Disk allocation from free disk only, never exceed 70% of free disk per profile
- Minimal must be lightweight (development/testing)
- Development is for daily coding
- Standard is balanced general-purpose
- Power is for heavy builds/CI
- Kubernetes needs at least 2 CPUs and 4 GiB RAM for k3s
- All values must be integers

JSON format:
{
  "minimal":     { "cpus": N, "memory": N, "disk": N },
  "development": { "cpus": N, "memory": N, "disk": N },
  "standard":    { "cpus": N, "memory": N, "disk": N },
  "power":       { "cpus": N, "memory": N, "disk": N },
  "kubernetes":  { "cpus": N, "memory": N, "disk": N }
}`;

          const raw = await aiApi.chat(provider, model, apiKey,
            [{ role: "user", content: prompt }], endpoint);
          const text = typeof raw === "string" ? raw : String(raw);

          // Extract JSON from response
          const jsonMatch = text.match(/\{[\s\S]*\}/);
          if (jsonMatch) {
            const data = JSON.parse(jsonMatch[0]);
            setPresets(prev => {
              const updated = prev.map(p => {
                const override = data[p.id];
                if (!override) return p;
                return {
                  ...p,
                  cpus: Math.max(1, Math.min(Math.floor(specs.cpu_cores * 0.8), parseInt(override.cpus) || p.cpus)),
                  memory: Math.max(1, Math.min(Math.floor(specs.memory_gib * 0.8), parseInt(override.memory) || p.memory)),
                  disk: Math.max(10, Math.min(Math.floor(specs.disk_free_gib * 0.7), parseInt(override.disk) || p.disk)),
                };
              });
              saveDetectedPresets(updated);
              return updated;
            });
            setAiOptimized(true);
            globalToast("success", "AI optimized profiles for your hardware");
          }
        } catch (e) {
          // AI failed — math presets already applied, silently continue
          console.warn("AI optimization failed, using math presets:", e);
        } finally {
          setAiOptimizing(false);
        }
      }
    } catch (e) {
      globalToast("error", `Failed to detect host specs: ${e}`);
      setDetecting(false);
    }
  };

  const applyPreset = (preset: InstancePreset) => {
    setSelectedPreset(preset.id);
    setConfig(prev => ({
      ...prev,
      cpus: preset.cpus,
      memory: preset.memory,
      disk: preset.disk,
      runtime: preset.runtime,
      kubernetes: preset.kubernetes ?? preset.id === "kubernetes",
      network_address: preset.network_address ?? false,
    }));
  };

  const handleSaveCustomPreset = () => {
    if (!saveLabel.trim()) return;
    const flags = [];
    if (config.kubernetes) flags.push("K8s");
    if (config.network_address) flags.push("Net");
    const newPreset: InstancePreset = {
      id: `custom-${Date.now()}`,
      label: saveLabel.trim(),
      icon: "custom",
      description: `${config.cpus}C · ${config.memory}G · ${config.disk}G${flags.length ? " · " + flags.join("+") : ""}`,
      cpus: config.cpus,
      memory: config.memory,
      disk: config.disk,
      runtime: config.runtime,
      kubernetes: config.kubernetes,
      network_address: config.network_address,
      color: CUSTOM_COLORS[customPresets.length % CUSTOM_COLORS.length],
    };
    const updated = [...customPresets, newPreset];
    setCustomPresets(updated);
    saveCustomPresets(updated);
    setShowSaveForm(false);
    setSaveLabel("");
    setSelectedPreset(newPreset.id);
    globalToast("success", `Profile "${newPreset.label}" saved`);
  };

  const handleDeleteCustomPreset = (id: string) => {
    const updated = customPresets.filter(p => p.id !== id);
    setCustomPresets(updated);
    saveCustomPresets(updated);
    if (selectedPreset === id) setSelectedPreset(null);
  };

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
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 600, width: "min(600px, 95vw)" }}>
        <div className="modal-header">
          <h2 className="modal-title">Create Instance</h2>
          <button className="btn btn-icon btn-ghost" onClick={onClose}><CloseIcon size={16} /></button>
        </div>
        {error && <div style={{ padding: "8px 12px", background: "rgba(248, 81, 73, 0.1)", borderRadius: "var(--radius-md)", color: "var(--accent-red)", fontSize: "var(--text-sm)", marginBottom: 16 }}>{error}</div>}

        {/* Host Specs Info Banner */}
        {hostInfo && (
          <div style={{
            display: "flex", alignItems: "center", gap: 12, padding: "10px 14px", marginBottom: 16,
            borderRadius: 10, background: "rgba(88,166,255,0.06)",
            border: "1px solid rgba(88,166,255,0.15)",
          }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="1.5" style={{ flexShrink: 0 }}>
              <rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>
            </svg>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)", marginBottom: 2 }}>
                {hostInfo.model || `${hostInfo.arch} Host`}
              </div>
              <div style={{ fontSize: "10px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                {hostInfo.cpu_cores} cores · {hostInfo.memory_gib} GiB RAM · {hostInfo.disk_free_gib} GiB free / {hostInfo.disk_total_gib} GiB total
              </div>
            </div>
            <span style={{ padding: "2px 8px", borderRadius: 10, fontSize: "9px", fontWeight: 600,
              background: aiOptimized ? "rgba(167,139,250,0.12)" : "rgba(63,185,80,0.1)",
              color: aiOptimized ? "#a78bfa" : "var(--accent-green)",
              whiteSpace: "nowrap" }}>
              {aiOptimized ? "✦ AI Optimized" : "✓ Detected"}
            </span>
          </div>
        )}

        {/* Quick Start Profiles */}
        <div style={{ marginBottom: 20 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
            <div style={{ fontSize: "11px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)" }}>
              Quick Start Profile
            </div>
            <button
              className="btn btn-ghost"
              onClick={handleAutoDetect}
              disabled={detecting}
              style={{
                fontSize: "10px", padding: "3px 10px", display: "flex", alignItems: "center", gap: 5,
                borderRadius: 8,
                background: hostInfo ? "rgba(63,185,80,0.06)" : "rgba(88,166,255,0.06)",
                border: `1px solid ${hostInfo ? "rgba(63,185,80,0.2)" : "rgba(88,166,255,0.2)"}`,
                color: hostInfo ? "var(--accent-green)" : "var(--accent-blue)",
              }}
            >
              {detecting ? (
                <><div className="spinner" style={{ width: 10, height: 10 }} /> Detecting...</>
              ) : aiOptimizing ? (
                <><div className="spinner" style={{ width: 10, height: 10, borderColor: "#a78bfa", borderTopColor: "transparent" }} /> AI Optimizing...</>
              ) : hostInfo ? (
                <>
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>
                  Re-detect
                  {aiOptimized && <span style={{ fontSize: "9px", background: "rgba(167,139,250,0.15)", color: "#a78bfa", padding: "0 4px", borderRadius: 4 }}>✦ AI</span>}
                </>
              ) : (
                <><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg> Auto-detect Host</>
              )}
            </button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 8 }}>
            {presets.map(preset => {
              const isActive = selectedPreset === preset.id;
              return (
                <button
                  key={preset.id}
                  onClick={() => applyPreset(preset)}
                  style={{
                    background: isActive ? `color-mix(in srgb, ${preset.color} 12%, var(--bg-card))` : "var(--bg-primary)",
                    border: `1.5px solid ${isActive ? preset.color : "var(--border-primary)"}`,
                    borderRadius: 10,
                    padding: "10px 8px",
                    cursor: "pointer",
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    gap: 4,
                    transition: "all 150ms ease",
                    boxShadow: isActive ? `0 0 0 2px color-mix(in srgb, ${preset.color} 25%, transparent)` : "none",
                    outline: "none",
                    textAlign: "center",
                  }}
                >
                  {(() => { const Icon = PresetIcons[preset.icon] ?? PresetIcons["custom"]; return <Icon size={20} color={isActive ? preset.color : "var(--text-muted)"} />; })()}
                  <span style={{ fontSize: "11px", fontWeight: 700, color: isActive ? preset.color : "var(--text-primary)", whiteSpace: "nowrap" }}>{preset.label}</span>
                  <span style={{ fontSize: "10px", color: "var(--text-muted)", lineHeight: 1.3 }}>{preset.description}</span>
                  <span style={{ fontSize: "9px", fontFamily: "var(--font-mono)", color: isActive ? preset.color : "var(--text-muted)", marginTop: 2, opacity: 0.85 }}>
                    {preset.cpus}C · {preset.memory}G · {preset.disk}G
                  </span>
                </button>
              );
            })}
          </div>

          {/* Custom Profiles */}
          {customPresets.length > 0 && (
            <>
              <div style={{ fontSize: "10px", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)", marginTop: 10, marginBottom: 6 }}>
                My Profiles
              </div>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                {customPresets.map(preset => {
                  const isActive = selectedPreset === preset.id;
                  return (
                    <div key={preset.id} style={{ position: "relative" }}>
                      <button
                        onClick={() => applyPreset(preset)}
                        style={{
                          background: isActive ? `color-mix(in srgb, ${preset.color} 12%, var(--bg-card))` : "var(--bg-primary)",
                          border: `1.5px solid ${isActive ? preset.color : "var(--border-primary)"}`,
                          borderRadius: 10, padding: "8px 14px 8px 10px", cursor: "pointer",
                          display: "flex", alignItems: "center", gap: 6,
                          transition: "all 150ms ease",
                          boxShadow: isActive ? `0 0 0 2px color-mix(in srgb, ${preset.color} 25%, transparent)` : "none",
                          outline: "none",
                        }}
                      >
                        <div style={{ width: 28, height: 28, display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0,
                          background: `color-mix(in srgb, ${preset.color} 10%, transparent)`, borderRadius: 8 }}>
                          {(() => { const Icon = PresetIcons[preset.icon] ?? PresetIcons["custom"]; return <Icon size={14} color={isActive ? preset.color : "var(--text-muted)"} />; })()}
                        </div>
                        <div style={{ textAlign: "left" }}>
                          <div style={{ fontSize: "11px", fontWeight: 700, color: isActive ? preset.color : "var(--text-primary)", whiteSpace: "nowrap" }}>{preset.label}</div>
                          <div style={{ fontSize: "9px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
                            {preset.cpus}C · {preset.memory}G · {preset.disk}G
                          </div>
                        </div>
                      </button>
                      <button
                        onClick={(e) => { e.stopPropagation(); handleDeleteCustomPreset(preset.id); }}
                        style={{
                          position: "absolute", top: -5, right: -5,
                          width: 16, height: 16, borderRadius: "50%",
                          background: "var(--bg-secondary)", border: "1px solid var(--border-primary)",
                          display: "flex", alignItems: "center", justifyContent: "center",
                          cursor: "pointer", padding: 0, lineHeight: 1,
                          color: "var(--text-muted)", fontSize: "10px",
                        }}
                        title="Delete profile"
                      >✕</button>
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>

        <div style={{ borderTop: "1px solid var(--border-subtle)", paddingTop: 16 }}>
          <div className="form-group">
            <label className="form-label">Profile Name</label>
            <input className="input" value={config.profile} onChange={(e) => setConfig({ ...config, profile: e.target.value })} placeholder="default" />
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
            <div className="form-group">
              <label className="form-label">Runtime</label>
              <select className="input select" value={config.runtime} onChange={(e) => { setSelectedPreset(null); setConfig({ ...config, runtime: e.target.value }); }}>
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
              <input className="input" type="number" min={1} max={32} value={config.cpus} onChange={(e) => { setSelectedPreset(null); setConfig({ ...config, cpus: Number(e.target.value) }); }} />
            </div>
            <div className="form-group">
              <label className="form-label">Memory (GiB)</label>
              <input className="input" type="number" min={1} max={128} value={config.memory} onChange={(e) => { setSelectedPreset(null); setConfig({ ...config, memory: Number(e.target.value) }); }} />
            </div>
            <div className="form-group">
              <label className="form-label">Disk (GiB)</label>
              <input className="input" type="number" min={10} max={1000} value={config.disk} onChange={(e) => { setSelectedPreset(null); setConfig({ ...config, disk: Number(e.target.value) }); }} />
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
              <input type="checkbox" className="checkbox" id="k8s-check" checked={config.kubernetes} onChange={(e) => setConfig({ ...config, kubernetes: e.target.checked })} />
              <label htmlFor="k8s-check" className="form-label" style={{ marginBottom: 0 }}>Enable Kubernetes (K3s)</label>
            </div>
            <div className="form-group" style={{ flexDirection: "row", alignItems: "center", gap: 10 }}>
              <input type="checkbox" className="checkbox" id="net-addr" checked={config.network_address} onChange={(e) => setConfig({ ...config, network_address: e.target.checked })} />
              <label htmlFor="net-addr" className="form-label" style={{ marginBottom: 0 }}>Reachable Network Address</label>
            </div>
          </div>
        </div>
        {/* Save Profile Form */}
        {showSaveForm && (
          <div style={{
            padding: "12px 16px", margin: "0 0 4px",
            background: "rgba(88,166,255,0.04)",
            border: "1px solid rgba(88,166,255,0.15)",
            borderRadius: 10,
          }}>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)", marginBottom: 8 }}>Save Current Config as Profile</div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <div style={{ width: 36, height: 36, display: "flex", alignItems: "center", justifyContent: "center",
                background: "rgba(88,166,255,0.08)", borderRadius: 8, flexShrink: 0, border: "1px solid rgba(88,166,255,0.2)" }}>
                <PresetIcons.custom size={16} color="var(--accent-blue)" />
              </div>
              <input
                className="input" value={saveLabel}
                onChange={(e) => setSaveLabel(e.target.value)}
                placeholder="Profile name..." autoFocus
                style={{ flex: 1 }}
                onKeyDown={(e) => e.key === "Enter" && handleSaveCustomPreset()}
              />
              <button className="btn btn-primary" onClick={handleSaveCustomPreset} disabled={!saveLabel.trim()}
                style={{ fontSize: "var(--text-xs)", padding: "6px 12px", whiteSpace: "nowrap" }}>
                Save
              </button>
              <button className="btn btn-ghost" onClick={() => setShowSaveForm(false)}
                style={{ fontSize: "var(--text-xs)", padding: "6px 8px" }}>
                Cancel
              </button>
            </div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)", marginTop: 6 }}>
              Saves: {config.cpus} CPUs · {config.memory} GiB RAM · {config.disk} GiB Disk · {config.runtime}
            </div>
          </div>
        )}

        <div className="modal-footer">
          {!showSaveForm && (
            <button className="btn btn-ghost" onClick={() => setShowSaveForm(true)}
              style={{ marginRight: "auto", fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 4, color: "var(--accent-blue)" }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/>
                <polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/>
              </svg>
              Save as Profile
            </button>
          )}
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


  const handleAction = async (profile: string, action: "start" | "stop" | "restart" | "delete", config?: StartConfig) => {
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
    globalToast("success", `${labels[action]} instance '${profile}'${config ? ` with ${config.cpus}C/${config.memory}G` : ""}...`);
    setActionLoading(`${profile}-${action}`);
    pendingOps.set("instance-action", `${profile}-${action}`);

    const startConfig: StartConfig = config ?? { profile, runtime: "docker", cpus: 2, memory: 2, disk: 60, vm_type: "", kubernetes: false, kubernetes_version: "", arch: "", mount_type: "", mounts: [], dns: [], network_address: false };
    const runAction = async () => {
      switch (action) {
        case "start": await colimaApi.startInstance({ ...startConfig, profile }); break;
        case "stop": await colimaApi.stopInstance(profile); break;
        case "restart": await colimaApi.stopInstance(profile); await colimaApi.startInstance({ ...startConfig, profile }); break;
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

/* ===== Profile Dropdown Component (SOTA) ===== */
function ProfileDropdown({
  myProfiles, quickPresets, lastUsedId, onSelect,
}: {
  myProfiles: InstancePreset[];
  quickPresets: InstancePreset[];
  lastUsedId: string | null;
  onSelect: (p: InstancePreset) => void;
}) {
  const sectionLabel = (text: string) => (
    <div style={{
      padding: "6px 12px 3px",
      fontSize: "9px",
      fontWeight: 700,
      textTransform: "uppercase",
      letterSpacing: "0.07em",
      color: "var(--text-muted)",
      borderBottom: "1px solid var(--border-primary)",
      marginBottom: 2,
    }}>{text}</div>
  );

  const renderItem = (p: InstancePreset) => {
    const Icon = PresetIcons[p.icon] ?? PresetIcons["custom"];
    const isLast = p.id === lastUsedId;
    return (
      <button
        key={p.id}
        onClick={() => onSelect(p)}
        style={{
          display: "flex", alignItems: "center", gap: 9,
          width: "100%", padding: "7px 12px",
          background: isLast ? "color-mix(in srgb, var(--accent-blue) 8%, transparent)" : "none",
          border: "none", cursor: "pointer",
          color: "var(--text-primary)", fontSize: "var(--text-xs)",
          transition: "background 0.1s",
        }}
        onMouseEnter={e => (e.currentTarget.style.background = `color-mix(in srgb, ${p.color} 10%, transparent)`)}
        onMouseLeave={e => (e.currentTarget.style.background = isLast ? "color-mix(in srgb, var(--accent-blue) 8%, transparent)" : "none")}
      >
        {/* Icon */}
        <div style={{
          width: 24, height: 24, flexShrink: 0,
          display: "flex", alignItems: "center", justifyContent: "center",
          background: `color-mix(in srgb, ${p.color} 14%, transparent)`,
          borderRadius: 7,
        }}>
          <Icon size={13} color={p.color} />
        </div>
        {/* Label + specs */}
        <div style={{ flex: 1, textAlign: "left", minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
            <span style={{ fontWeight: 600, fontSize: "11px", color: isLast ? "var(--accent-blue)" : "var(--text-primary)" }}>{p.label}</span>
            {isLast && (
              <span style={{
                fontSize: "8px", fontWeight: 700, padding: "1px 5px",
                background: "color-mix(in srgb, var(--accent-blue) 20%, transparent)",
                color: "var(--accent-blue)", borderRadius: 4, letterSpacing: "0.04em",
              }}>last</span>
            )}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 4, marginTop: 1 }}>
            <span style={{ fontSize: "9px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
              {p.cpus}C · {p.memory}G · {p.disk}G
            </span>
            {(p.kubernetes ?? p.id === "kubernetes") && (
              <span style={{
                fontSize: "8px", padding: "1px 4px",
                background: "color-mix(in srgb, #a78bfa 18%, transparent)",
                color: "#a78bfa", borderRadius: 3, fontWeight: 600,
              }}>K8s</span>
            )}
            {p.network_address && (
              <span style={{
                fontSize: "8px", padding: "1px 4px",
                background: "color-mix(in srgb, var(--accent-green) 18%, transparent)",
                color: "var(--accent-green)", borderRadius: 3, fontWeight: 600,
              }}>Net</span>
            )}
          </div>
        </div>
      </button>
    );
  };

  return (
    <div style={{
      position: "absolute", top: "calc(100% + 4px)", right: 0, zIndex: 200,
      minWidth: 210,
      background: "var(--bg-card)",
      border: "1px solid var(--border-primary)",
      borderRadius: 12,
      boxShadow: "0 12px 32px rgba(0,0,0,0.35), 0 2px 8px rgba(0,0,0,0.2)",
      overflow: "hidden",
    }}>
      {myProfiles.length > 0 && (
        <>
          {sectionLabel("My Profiles")}
          {myProfiles.map(renderItem)}
        </>
      )}
      {quickPresets.length > 0 && (
        <>
          {myProfiles.length > 0 && <div style={{ height: 1, background: "var(--border-primary)", margin: "3px 0" }} />}
          {sectionLabel("Quick Presets")}
          {quickPresets.map(renderItem)}
        </>
      )}
    </div>
  );
}

/* ===== Colima Detail Panel ===== */
function ColimaDetail({ inst, actionLoading, onAction, onRefresh }: { inst: ColimaInstance; actionLoading: string | null; onAction: (profile: string, action: "start" | "stop" | "restart" | "delete", config?: StartConfig) => void; onRefresh: () => void }) {
  const profileId = inst.name === "colima" ? "default" : inst.name.replace("colima-", "");
  const isRunning = inst.status === "Running";
  const isLoading = actionLoading?.startsWith(profileId);
  const [k8sLoading, setK8sLoading] = useState<string | null>(pendingOps.get(`k8s-${profileId}`) || null);
  const [k8sNotice, setK8sNotice] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [showProfileMenu, setShowProfileMenu] = useState<"start" | "restart" | null>(null);

  // Tiered profile lists: custom ("My Profiles") first, then quick presets
  const myProfiles: InstancePreset[] = loadCustomPresets();
  const quickPresets: InstancePreset[] = loadDetectedPresets() ?? DEFAULT_PRESETS;
  const allProfiles: InstancePreset[] = [...myProfiles, ...quickPresets];

  // Last-used profile per instance
  const lastUsedId = loadLastUsedProfile(profileId);
  const lastUsedPreset = lastUsedId ? allProfiles.find(p => p.id === lastUsedId) ?? null : null;

  // Helper: build StartConfig from preset
  const presetToConfig = (p: InstancePreset): StartConfig => ({
    profile: profileId,
    runtime: p.runtime,
    cpus: p.cpus,
    memory: p.memory,
    disk: p.disk,
    vm_type: "vz",
    kubernetes: p.kubernetes ?? p.id === "kubernetes",
    kubernetes_version: "",
    arch: "",
    mount_type: "",
    mounts: [],
    dns: [],
    network_address: p.network_address ?? false,
  });

  const handleProfileAction = (p: InstancePreset, action: "start" | "restart") => {
    saveLastUsedProfile(profileId, p.id);
    setShowProfileMenu(null);
    onAction(profileId, action, presetToConfig(p));
  };

  // Close dropdown on outside click
  useEffect(() => {
    if (!showProfileMenu) return;
    const close = () => setShowProfileMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [showProfileMenu]);

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
              {/* Restart split-button — SOTA */}
              <div style={{ position: "relative", display: "flex" }} onClick={e => e.stopPropagation()}>
                <button
                  className="btn btn-ghost"
                  disabled={!!isLoading}
                  onClick={() => lastUsedPreset ? handleProfileAction(lastUsedPreset, "restart") : onAction(profileId, "restart")}
                  style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 5, borderRadius: "6px 0 0 6px", borderRight: "1px solid var(--border-primary)", maxWidth: 130, overflow: "hidden" }}
                >
                  {actionLoading === `${profileId}-restart`
                    ? <div className="spinner" style={{ width: 12, height: 12, flexShrink: 0 }} />
                    : <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ flexShrink: 0 }}><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/></svg>}
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    Restart{lastUsedPreset ? ` · ${lastUsedPreset.label}` : ""}
                  </span>
                </button>
                <button
                  className="btn btn-ghost"
                  disabled={!!isLoading}
                  onClick={() => setShowProfileMenu(showProfileMenu === "restart" ? null : "restart")}
                  style={{ fontSize: "10px", padding: "4px 7px", borderRadius: "0 6px 6px 0", display: "flex", alignItems: "center" }}
                >
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
                </button>
                {showProfileMenu === "restart" && (
                  <ProfileDropdown
                    myProfiles={myProfiles}
                    quickPresets={quickPresets}
                    lastUsedId={lastUsedId}
                    onSelect={p => handleProfileAction(p, "restart")}
                  />
                )}
              </div>
            </>
          ) : (
            /* Start split-button — SOTA */
            <div style={{ position: "relative", display: "flex" }} onClick={e => e.stopPropagation()}>
              <button
                className="btn btn-primary"
                disabled={!!isLoading}
                onClick={() => lastUsedPreset ? handleProfileAction(lastUsedPreset, "start") : onAction(profileId, "start")}
                style={{ fontSize: "var(--text-xs)", display: "flex", alignItems: "center", gap: 5, borderRadius: "6px 0 0 6px", borderRight: "1px solid rgba(255,255,255,0.15)", maxWidth: 140, overflow: "hidden" }}
              >
                {actionLoading === `${profileId}-start`
                  ? <div className="spinner" style={{ width: 12, height: 12, flexShrink: 0 }} />
                  : <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" style={{ flexShrink: 0 }}><polygon points="6,4 20,12 6,20"/></svg>}
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  Start{lastUsedPreset ? ` · ${lastUsedPreset.label}` : ""}
                </span>
              </button>
              <button
                className="btn btn-primary"
                disabled={!!isLoading}
                onClick={() => setShowProfileMenu(showProfileMenu === "start" ? null : "start")}
                style={{ fontSize: "10px", padding: "4px 8px", borderRadius: "0 6px 6px 0", display: "flex", alignItems: "center" }}
              >
                <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
              </button>
              {showProfileMenu === "start" && (
                <ProfileDropdown
                  myProfiles={myProfiles}
                  quickPresets={quickPresets}
                  lastUsedId={lastUsedId}
                  onSelect={p => handleProfileAction(p, "start")}
                />
              )}
            </div>
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

import { useState, useEffect } from "react";
import { SystemInfo, dockerApi } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { BroomIcon } from "../components/Icons";

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

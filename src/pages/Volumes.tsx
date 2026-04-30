import { useState, useEffect, useCallback } from "react";
import { volumesApi, DockerVolume } from "../lib/api";

interface VolumesProps {}

export default function Volumes(_props: VolumesProps) {
  const [volumes, setVolumes] = useState<DockerVolume[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDriver, setNewDriver] = useState("local");
  const [inspecting, setInspecting] = useState<string | null>(null);
  const [inspectData, setInspectData] = useState<string>("");
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const list = await volumesApi.listVolumes();
      setVolumes(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    setActionLoading("create");
    try {
      await volumesApi.createVolume(newName.trim(), newDriver);
      setNewName("");
      setShowCreate(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleRemove = async (name: string) => {
    if (!confirm(`Remove volume "${name}"?`)) return;
    setActionLoading(name);
    try {
      await volumesApi.removeVolume(name, true);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handlePrune = async () => {
    if (!confirm("Remove all unused volumes? This cannot be undone.")) return;
    setActionLoading("prune");
    try {
      await volumesApi.pruneVolumes();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleInspect = async (name: string) => {
    if (inspecting === name) { setInspecting(null); return; }
    try {
      const data = await volumesApi.inspectVolume(name);
      setInspectData(data);
      setInspecting(name);
    } catch (e) {
      setError(String(e));
    }
  };

  const filtered = volumes.filter(v =>
    v.Name.toLowerCase().includes(search.toLowerCase()) ||
    v.Driver.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div style={{ padding: "24px" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "20px" }}>
        <div>
          <h2 style={{ margin: 0, fontSize: "var(--text-xl)", fontWeight: 600 }}>Volumes</h2>
          <p style={{ margin: "4px 0 0", color: "var(--text-muted)", fontSize: "var(--text-sm)" }}>
            {volumes.length} volume{volumes.length !== 1 ? "s" : ""}
          </p>
        </div>
        <div style={{ display: "flex", gap: "8px" }}>
          <button className="btn btn-ghost" onClick={handlePrune} disabled={actionLoading === "prune"}>
            {actionLoading === "prune" ? "Pruning..." : "🗑 Prune"}
          </button>
          <button className="btn btn-primary" onClick={() => setShowCreate(!showCreate)}>
            + Create Volume
          </button>
          <button className="btn btn-ghost" onClick={refresh}>↻ Refresh</button>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div style={{ padding: "12px", background: "rgba(248,81,73,0.1)", color: "var(--accent-red)", borderRadius: "8px", marginBottom: "16px", fontSize: "var(--text-sm)" }}>
          ⚠ {error}
          <button className="btn btn-ghost" style={{ marginLeft: "8px", fontSize: "var(--text-xs)" }} onClick={() => setError(null)}>Dismiss</button>
        </div>
      )}

      {/* Create form */}
      {showCreate && (
        <div style={{ padding: "16px", background: "var(--bg-secondary)", borderRadius: "12px", marginBottom: "16px", border: "1px solid var(--border-primary)" }}>
          <h3 style={{ margin: "0 0 12px", fontSize: "var(--text-base)" }}>Create Volume</h3>
          <div style={{ display: "flex", gap: "12px", alignItems: "flex-end" }}>
            <div style={{ flex: 1 }}>
              <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: "4px" }}>Name</label>
              <input
                type="text"
                value={newName}
                onChange={e => setNewName(e.target.value)}
                placeholder="my-volume"
                style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: "6px", color: "var(--text-primary)", fontSize: "var(--text-sm)" }}
                onKeyDown={e => e.key === "Enter" && handleCreate()}
              />
            </div>
            <div>
              <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: "4px" }}>Driver</label>
              <select
                value={newDriver}
                onChange={e => setNewDriver(e.target.value)}
                style={{ padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: "6px", color: "var(--text-primary)", fontSize: "var(--text-sm)" }}
              >
                <option value="local">local</option>
              </select>
            </div>
            <button className="btn btn-primary" onClick={handleCreate} disabled={actionLoading === "create" || !newName.trim()}>
              {actionLoading === "create" ? "Creating..." : "Create"}
            </button>
            <button className="btn btn-ghost" onClick={() => setShowCreate(false)}>Cancel</button>
          </div>
        </div>
      )}

      {/* Search */}
      <div style={{ marginBottom: "16px" }}>
        <input
          type="text"
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="🔍 Search volumes..."
          style={{ width: "100%", padding: "8px 12px", background: "var(--bg-secondary)", border: "1px solid var(--border-primary)", borderRadius: "8px", color: "var(--text-primary)", fontSize: "var(--text-sm)" }}
        />
      </div>

      {/* Volume list */}
      {loading ? (
        <div style={{ textAlign: "center", padding: "40px", color: "var(--text-muted)" }}>Loading volumes...</div>
      ) : filtered.length === 0 ? (
        <div style={{ textAlign: "center", padding: "40px", color: "var(--text-muted)" }}>
          {search ? "No volumes match your search" : "No volumes found"}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          {filtered.map(vol => (
            <div key={vol.Name} style={{ padding: "16px", background: "var(--bg-secondary)", borderRadius: "12px", border: "1px solid var(--border-primary)" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: "var(--text-base)" }}>{vol.Name}</div>
                  <div style={{ color: "var(--text-muted)", fontSize: "var(--text-sm)", marginTop: "4px" }}>
                    Driver: <span style={{ color: "var(--accent-blue)" }}>{vol.Driver}</span>
                    {vol.Scope && <> · Scope: {vol.Scope}</>}
                    {vol.Mountpoint && <> · {vol.Mountpoint}</>}
                  </div>
                </div>
                <div style={{ display: "flex", gap: "6px" }}>
                  <button className="btn btn-ghost" onClick={() => handleInspect(vol.Name)} style={{ fontSize: "var(--text-xs)", padding: "4px 10px" }}>
                    {inspecting === vol.Name ? "Hide" : "Inspect"}
                  </button>
                  <button
                    className="btn btn-ghost"
                    onClick={() => handleRemove(vol.Name)}
                    disabled={actionLoading === vol.Name}
                    style={{ fontSize: "var(--text-xs)", padding: "4px 10px", color: "var(--accent-red)" }}
                  >
                    {actionLoading === vol.Name ? "..." : "Remove"}
                  </button>
                </div>
              </div>
              {inspecting === vol.Name && (
                <pre style={{ marginTop: "12px", padding: "12px", background: "var(--bg-primary)", borderRadius: "8px", fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "300px", color: "var(--text-secondary)" }}>
                  {inspectData}
                </pre>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

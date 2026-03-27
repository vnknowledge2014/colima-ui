import { useState, useEffect, useCallback, useDeferredValue } from "react";
import { volumesApi } from "../lib/api";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { BroomIcon, WarningIcon } from "../components/Icons";
import { useAtom } from "jotai";
import { volumesAtom, volumesLoadingAtom } from "../store/resourceAtom";

interface VolumesProps {}

export default function Volumes(_props: VolumesProps) {
  const [volumes, setVolumes] = useAtom(volumesAtom);
  const [loading, setLoading] = useAtom(volumesLoadingAtom);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDriver, setNewDriver] = useState("local");
  const [inspecting, setInspecting] = useState<string | null>(null);
  const [inspectData, setInspectData] = useState<string>("");
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [batchLoading, setBatchLoading] = useState(false);
  const { confirm, ConfirmDialogProps } = useConfirm();

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
    const ok = await confirm({ title: "Remove Volume", message: `Remove volume "${name}"?`, confirmText: "Remove", variant: "danger" });
    if (!ok) return;
    setActionLoading(name);
    try {
      await volumesApi.removeVolume(name, true);
      // Clear from selection if it was selected
      if (selected.has(name)) {
        setSelected(prev => { const next = new Set(prev); next.delete(name); return next; });
      }
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handlePrune = async () => {
    const ok = await confirm({ title: "Prune Volumes", message: "Remove all unused volumes? This cannot be undone.", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    setActionLoading("prune");
    try {
      await volumesApi.pruneVolumes();
      setSelected(new Set());
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
    v.Name.toLowerCase().includes(deferredSearch.toLowerCase()) ||
    v.Driver.toLowerCase().includes(deferredSearch.toLowerCase())
  );

  useEffect(() => { setSelected(new Set()); }, [search]);

  const toggleSelect = (name: string) => setSelected(prev => {
    const next = new Set(prev);
    next.has(name) ? next.delete(name) : next.add(name);
    return next;
  });


  const handleBatchRemove = async () => {
    const names = filtered.filter(v => selected.has(v.Name)).map(v => v.Name);
    if (names.length === 0) return;
    const ok = await confirm({ title: "Remove Selected Volumes", message: `Remove ${names.length} volume${names.length > 1 ? "s" : ""}?\n\n${names.join(", ")}\n\nThis cannot be undone.`, confirmText: `Remove ${names.length}`, variant: "danger" });
    if (!ok) return;
    setBatchLoading(true);
    let ok_count = 0;
    for (const name of names) {
      try { await volumesApi.removeVolume(name, true); ok_count++; } catch { /* continue */ }
    }
    setSelected(new Set());
    setBatchLoading(false);
    refresh();
  };

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
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          {selected.size > 0 && (
            <button className="btn btn-ghost" style={{ color: "var(--accent-red)", fontSize: "var(--text-sm)" }}
              onClick={handleBatchRemove} disabled={batchLoading}>
              {batchLoading ? "Removing..." : `Remove ${selected.size} Selected`}
            </button>
          )}
          <button className="btn btn-ghost" onClick={handlePrune} disabled={actionLoading === "prune"}
            style={{ display: "flex", alignItems: "center", gap: 4 }}>
            {actionLoading === "prune" ? "Pruning..." : <><BroomIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> Prune</>}
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
          <WarningIcon size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> {error}
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
          placeholder="Search volumes..."
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
            <div key={vol.Name} style={{ padding: "16px", background: selected.has(vol.Name) ? "rgba(88,166,255,0.06)" : "var(--bg-secondary)", borderRadius: "12px", border: selected.has(vol.Name) ? "1px solid rgba(88,166,255,0.25)" : "1px solid var(--border-primary)", transition: "all 150ms" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <input type="checkbox" checked={selected.has(vol.Name)} onChange={() => toggleSelect(vol.Name)}
                    style={{ accentColor: "var(--accent-blue)", cursor: "pointer" }} />
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
      <ConfirmDialog {...ConfirmDialogProps} />
    </div>
  );
}

import { useState, useEffect, useCallback, useDeferredValue } from "react";
import { networksApi } from "../lib/api";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { BroomIcon, WarningIcon } from "../components/Icons";
import { useAtom } from "jotai";
import { networksAtom, networksLoadingAtom } from "../store/resourceAtom";

interface NetworksProps {}

export default function Networks(_props: NetworksProps) {
  const [networks, setNetworks] = useAtom(networksAtom);
  const [loading, setLoading] = useAtom(networksLoadingAtom);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDriver, setNewDriver] = useState("bridge");
  const [newSubnet, setNewSubnet] = useState("");
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
      const list = await networksApi.listNetworks();
      setNetworks(list);
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
      await networksApi.createNetwork(newName.trim(), newDriver, newSubnet.trim());
      setNewName("");
      setNewSubnet("");
      setShowCreate(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleRemove = async (name: string) => {
    const ok = await confirm({ title: "Remove Network", message: `Remove network "${name}"?`, confirmText: "Remove", variant: "danger" });
    if (!ok) return;
    setActionLoading(name);
    try {
      await networksApi.removeNetwork(name);
      // Always clear from selection (avoid stale closure from confirm dialog)
      const net = networks.find(n => n.Name === name);
      if (net) {
        setSelected(prev => { const next = new Set(prev); next.delete(net.Id); return next; });
      }
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handlePrune = async () => {
    const ok = await confirm({ title: "Prune Networks", message: "Remove all unused networks?", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    setActionLoading("prune");
    try {
      await networksApi.pruneNetworks();
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
      const data = await networksApi.inspectNetwork(name);
      setInspectData(data);
      setInspecting(name);
    } catch (e) {
      setError(String(e));
    }
  };

  const isSystemNetwork = (name: string) => ["bridge", "host", "none"].includes(name);

  const filtered = networks.filter(n =>
    n.Name.toLowerCase().includes(deferredSearch.toLowerCase()) ||
    n.Driver.toLowerCase().includes(deferredSearch.toLowerCase())
  );

  const driverColor = (driver: string) => {
    switch (driver) {
      case "bridge": return "var(--accent-blue)";
      case "host": return "var(--accent-orange)";
      case "overlay": return "var(--accent-purple)";
      case "null": return "var(--text-muted)";
      default: return "var(--accent-green)";
    }
  };

  useEffect(() => { setSelected(new Set()); }, [search]);

  // Auto-cleanup: remove stale selections when data changes
  useEffect(() => {
    setSelected(prev => {
      const validIds = new Set(networks.map(n => n.Id));
      const next = new Set([...prev].filter(id => validIds.has(id)));
      return next.size !== prev.size ? next : prev;
    });
  }, [networks]);

  const toggleSelect = (id: string) => setSelected(prev => {
    const next = new Set(prev);
    next.has(id) ? next.delete(id) : next.add(id);
    return next;
  });

  const handleBatchRemove = async () => {
    const names = filtered.filter(n => selected.has(n.Id) && !isSystemNetwork(n.Name)).map(n => n.Name);
    if (names.length === 0) return;
    const ok = await confirm({ title: "Remove Selected Networks", message: `Remove ${names.length} network${names.length > 1 ? "s" : ""}?\n\n${names.join(", ")}\n\nThis cannot be undone.`, confirmText: `Remove ${names.length}`, variant: "danger" });
    if (!ok) return;
    setBatchLoading(true);
    let ok_count = 0;
    for (const name of names) {
      try { await networksApi.removeNetwork(name); ok_count++; } catch { /* continue */ }
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
          <h2 style={{ margin: 0, fontSize: "var(--text-xl)", fontWeight: 600 }}>Networks</h2>
          <p style={{ margin: "4px 0 0", color: "var(--text-muted)", fontSize: "var(--text-sm)" }}>
            {networks.length} network{networks.length !== 1 ? "s" : ""}
          </p>
        </div>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          {selected.size > 0 && (
            <button className="btn btn-ghost" style={{ color: "var(--accent-red)", fontSize: "var(--text-sm)" }}
              onClick={handleBatchRemove} disabled={batchLoading}>
              {batchLoading ? "Removing..." : `Remove ${selected.size} Selected`}
            </button>
          )}
          <button className="btn btn-ghost" onClick={handlePrune} disabled={actionLoading === "prune"}>
            {actionLoading === "prune" ? "Pruning..." : <><BroomIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> Prune</>}
          </button>
          <button className="btn btn-primary" onClick={() => setShowCreate(!showCreate)}>
            + Create Network
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
          <h3 style={{ margin: "0 0 12px", fontSize: "var(--text-base)" }}>Create Network</h3>
          <div style={{ display: "flex", gap: "12px", alignItems: "flex-end", flexWrap: "wrap" }}>
            <div style={{ flex: 1, minWidth: "180px" }}>
              <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: "4px" }}>Name</label>
              <input
                type="text"
                value={newName}
                onChange={e => setNewName(e.target.value)}
                placeholder="my-network"
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
                <option value="bridge">bridge</option>
                <option value="host">host</option>
                <option value="overlay">overlay</option>
                <option value="macvlan">macvlan</option>
                <option value="ipvlan">ipvlan</option>
              </select>
            </div>
            <div style={{ flex: 1, minWidth: "180px" }}>
              <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: "4px" }}>Subnet (optional)</label>
              <input
                type="text"
                value={newSubnet}
                onChange={e => setNewSubnet(e.target.value)}
                placeholder="172.28.0.0/16"
                style={{ width: "100%", padding: "8px 12px", background: "var(--bg-primary)", border: "1px solid var(--border-primary)", borderRadius: "6px", color: "var(--text-primary)", fontSize: "var(--text-sm)" }}
              />
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
          placeholder="Search networks..."
          style={{ width: "100%", padding: "8px 12px", background: "var(--bg-secondary)", border: "1px solid var(--border-primary)", borderRadius: "8px", color: "var(--text-primary)", fontSize: "var(--text-sm)" }}
        />
      </div>

      {/* Network list */}
      {loading ? (
        <div style={{ textAlign: "center", padding: "40px", color: "var(--text-muted)" }}>Loading networks...</div>
      ) : filtered.length === 0 ? (
        <div style={{ textAlign: "center", padding: "40px", color: "var(--text-muted)" }}>
          {search ? "No networks match your search" : "No networks found"}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          {filtered.map(net => (
            <div key={net.Id} style={{ padding: "16px", background: selected.has(net.Id) ? "rgba(88,166,255,0.06)" : "var(--bg-secondary)", borderRadius: "12px", border: selected.has(net.Id) ? "1px solid rgba(88,166,255,0.25)" : "1px solid var(--border-primary)", transition: "all 150ms" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  {isSystemNetwork(net.Name) ? (
                    <input type="checkbox" disabled checked={false}
                      title="System network — cannot be removed"
                      style={{ opacity: 0.3, cursor: "not-allowed" }} />
                  ) : (
                    <input type="checkbox" checked={selected.has(net.Id)} onChange={() => toggleSelect(net.Id)}
                      style={{ accentColor: "var(--accent-blue)", cursor: "pointer" }} />
                  )}
                  <div>
                    <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                    <span style={{ fontWeight: 600, fontSize: "var(--text-base)" }}>{net.Name}</span>
                    {isSystemNetwork(net.Name) && (
                      <span style={{ fontSize: "var(--text-xs)", padding: "2px 6px", borderRadius: "4px", background: "rgba(139,148,158,0.2)", color: "var(--text-muted)" }}>
                        system
                      </span>
                    )}
                  </div>
                  <div style={{ color: "var(--text-muted)", fontSize: "var(--text-sm)", marginTop: "4px" }}>
                    Driver: <span style={{ color: driverColor(net.Driver) }}>{net.Driver}</span>
                    {net.Scope && <> · Scope: {net.Scope}</>}
                    <> · ID: {net.Id.substring(0, 12)}</>
                  </div>
                </div>
                <div style={{ display: "flex", gap: "6px" }}>
                  <button className="btn btn-ghost" onClick={() => handleInspect(net.Name)} style={{ fontSize: "var(--text-xs)", padding: "4px 10px" }}>
                    {inspecting === net.Name ? "Hide" : "Inspect"}
                  </button>
                  {!isSystemNetwork(net.Name) && (
                    <button
                      className="btn btn-ghost"
                      onClick={() => handleRemove(net.Name)}
                      disabled={actionLoading === net.Name}
                      style={{ fontSize: "var(--text-xs)", padding: "4px 10px", color: "var(--accent-red)" }}
                    >
                      {actionLoading === net.Name ? "..." : "Remove"}
                    </button>
                  )}
                </div>
              </div>
              </div>
              {inspecting === net.Name && (
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

import { useState, useEffect, useCallback } from "react";
import { modelsApi, colimaApi, AiModel, ColimaInstance } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { WarningIcon, CloseIcon } from "../components/Icons";

export default function Models() {
  const [models, setModels] = useState<AiModel[]>([]);
  const [instances, setInstances] = useState<ColimaInstance[]>([]);
  const [selectedProfile, setSelectedProfile] = useState("default");
  const [selectedRunner, setSelectedRunner] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [showPull, setShowPull] = useState(false);
  const [pullName, setPullName] = useState("");


  const fetchModels = useCallback(async () => {
    try {
      setError(null);
      const list = await modelsApi.listModels(selectedProfile, selectedRunner);
      setModels(list);
    } catch (e) {
      setError(String(e));
      setModels([]);
    } finally {
      setLoading(false);
    }
  }, [selectedProfile, selectedRunner]);

  const fetchInstances = useCallback(async () => {
    try {
      const list = await colimaApi.listInstances();
      setInstances(list.filter((i) => i.status === "Running"));
      if (list.length > 0 && !list.find((i) => {
        const p = i.name === "colima" ? "default" : i.name.replace("colima-", "");
        return p === selectedProfile;
      })) {
        const firstName = list[0].name;
        setSelectedProfile(firstName === "colima" ? "default" : firstName.replace("colima-", ""));
      }
    } catch (_) { /* ignore */ }
  }, [selectedProfile]);

  useEffect(() => {
    fetchInstances();
  }, [fetchInstances]);

  useEffect(() => {
    fetchModels();
  }, [fetchModels]);

  const handlePull = async () => {
    if (!pullName.trim()) return;
    const name = pullName.trim();
    // Fire-and-forget: close dialog, long operation runs in background
    globalToast("success", `Pulling model '${name}'... This may take a while.`);
    setPullName("");
    setShowPull(false);
    modelsApi.pullModel(selectedProfile, name, selectedRunner)
      .then(() => { globalToast("success", `Model '${name}' pulled successfully`); fetchModels(); })
      .catch((e) => globalToast("error", `Pull failed: ${e}`));
  };

  const handleDelete = async (name: string) => {
    setActionLoading(`${name}-delete`);
    try {
      await modelsApi.deleteModel(selectedProfile, name, selectedRunner);
      globalToast("success", `Model '${name}' deleted`);
      fetchModels();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleServe = async (name: string) => {
    setActionLoading(`${name}-serve`);
    try {
      await modelsApi.serveModel(selectedProfile, name, 11434, selectedRunner);
      globalToast("success", `Model '${name}' serving on port 11434`);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const dockerModels = [
    { name: "ai/smollm2", desc: "Small Language Model 2", size: "~1.7 GB" },
    { name: "ai/gemma3", desc: "Google Gemma 3", size: "~5.4 GB" },
    { name: "ai/llama3.2", desc: "Meta Llama 3.2", size: "~2.0 GB" },
    { name: "ai/phi4-mini", desc: "Microsoft Phi-4 Mini", size: "~2.4 GB" },
    { name: "ai/deepseek-r1", desc: "DeepSeek R1 (distill)", size: "~4.7 GB" },
    { name: "ai/mistral-small", desc: "Mistral Small 3.1", size: "~15 GB" },
  ];

  const ramalamaModels = [
    { name: "llama3.3", desc: "Meta's latest Llama 3.3", size: "~4.7 GB" },
    { name: "gemma2", desc: "Google Gemma 2", size: "~5.4 GB" },
    { name: "qwen2.5", desc: "Alibaba Qwen 2.5", size: "~4.7 GB" },
    { name: "phi4", desc: "Microsoft Phi-4", size: "~9.1 GB" },
    { name: "deepseek-r1", desc: "DeepSeek R1", size: "~4.7 GB" },
    { name: "mistral", desc: "Mistral 7B", size: "~4.1 GB" },
  ];

  const popularModels = selectedRunner === "ramalama" ? ramalamaModels : dockerModels;

  return (
    <>
      <div className="content-header">
        <h1>
          AI Models
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {models.length} model{models.length !== 1 ? "s" : ""}
          </span>
        </h1>
        <div className="content-header-actions">
          {instances.length > 1 && (
            <select
              className="input select"
              style={{ width: 160 }}
              value={selectedProfile}
              onChange={(e) => setSelectedProfile(e.target.value)}
            >
              {instances.map((inst) => {
                const p = inst.name === "colima" ? "default" : inst.name.replace("colima-", "");
                return <option key={p} value={p}>{inst.name}</option>;
              })}
            </select>
          )}
          <select
            className="input select"
            style={{ width: 180 }}
            value={selectedRunner}
            onChange={(e) => setSelectedRunner(e.target.value)}
          >
            <option value="">Docker Model Runner</option>
            <option value="ramalama">Ramalama</option>
          </select>
          <button className="btn btn-ghost" onClick={fetchModels}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
          <button className="btn btn-primary" onClick={() => setShowPull(true)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
            Pull Model
          </button>
        </div>
      </div>

      <div className="content-body">


        {error && (
          <div className="card" style={{ borderColor: "var(--accent-yellow)", marginBottom: 16 }}>
            <p style={{ color: "var(--accent-yellow)", fontSize: "var(--text-sm)" }}>
              <WarningIcon size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> AI model support not available
            </p>
            <p style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)", marginTop: 4 }}>
              {error.includes("krunkit") || error.includes("vm-type") ? (
                <>
                  GPU support requires krunkit. Install it first:
                  <code style={{ display: "block", margin: "8px 0", padding: "6px 10px", background: "var(--bg-primary)", borderRadius: 6, fontFamily: "var(--font-mono)" }}>
                    brew tap slp/krunkit && brew install krunkit
                  </code>
                  Then restart Colima:
                  <code style={{ display: "block", margin: "8px 0", padding: "6px 10px", background: "var(--bg-primary)", borderRadius: 6, fontFamily: "var(--font-mono)" }}>
                    colima start --runtime docker --vm-type krunkit
                  </code>
                </>
              ) : error.includes("not installed")
                ? "Required tools are not installed. Make sure Colima is available."
                : "Model management requires Colima started with krunkit VM type for GPU access."}
            </p>
          </div>
        )}

        {loading ? (
          <div className="loading-screen"><div className="spinner" /><span>Loading models...</span></div>
        ) : models.length > 0 ? (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: 12 }}>
            {models.map((model) => {
              const isLoading = actionLoading?.startsWith(model.name);
              return (
                <div key={model.name} className="card" style={{ opacity: isLoading ? 0.6 : 1 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 12 }}>
                    <div>
                      <div style={{ fontWeight: 600, fontSize: "var(--text-base)" }}>{model.name}</div>
                      <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 2 }}>
                        {model.family && <span>{model.family} · </span>}
                        {model.parameters && <span>{model.parameters} · </span>}
                        {model.size}
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 4 }}>
                      <button className="btn btn-ghost btn-icon" data-tooltip="Serve" disabled={!!isLoading} onClick={() => handleServe(model.name)}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="6,4 20,12 6,20"/></svg>
                      </button>
                      <button className="btn btn-ghost btn-icon" data-tooltip="Delete" disabled={!!isLoading} onClick={() => handleDelete(model.name)} style={{ color: "var(--accent-red)" }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                        </svg>
                      </button>
                    </div>
                  </div>
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                    {model.format && (
                      <span style={{ padding: "2px 8px", borderRadius: "var(--radius-sm)", background: "rgba(88, 166, 255, 0.1)", color: "var(--accent-blue)", fontSize: "var(--text-xs)" }}>
                        {model.format}
                      </span>
                    )}
                    {model.quantization && (
                      <span style={{ padding: "2px 8px", borderRadius: "var(--radius-sm)", background: "rgba(188, 140, 255, 0.1)", color: "var(--accent-purple)", fontSize: "var(--text-xs)" }}>
                        Q{model.quantization}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
              </svg>
            </div>
            <div className="empty-state-title">No models installed</div>
            <div className="empty-state-text">
              Pull a model to get started with AI inference.
            </div>

            {/* Popular Models Quick-Add */}
            <div style={{ width: "100%", maxWidth: 500, marginTop: 16 }}>
              <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: 8, textAlign: "left" }}>Popular Models</div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                {popularModels.map((m) => (
                  <div
                    key={m.name}
                    className="card"
                    style={{ cursor: "pointer", padding: 12, transition: "border-color 200ms" }}
                    onClick={() => { setPullName(m.name); setShowPull(true); }}
                  >
                    <div style={{ fontWeight: 500, fontSize: "var(--text-sm)" }}>{m.name}</div>
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>{m.desc}</div>
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--accent-blue)", marginTop: 4 }}>{m.size}</div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Pull Model Dialog */}
      {showPull && (
        <div className="modal-overlay" onClick={() => setShowPull(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: "min(450px, 90vw)" }}>
            <div className="modal-header">
              <h2 className="modal-title">Pull Model</h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setShowPull(false)}><CloseIcon size={16} /></button>
            </div>
            <div className="form-group">
              <label className="form-label">Model Name</label>
              <input
                className="input"
                placeholder="e.g. llama3.3, gemma2, phi4:14b"
                value={pullName}
                onChange={(e) => setPullName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handlePull()}
                autoFocus
              />
              <p style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 4 }}>
                Use model name from Ollama registry. Append :tag for specific variants.
              </p>
            </div>
            <div className="modal-footer">
              <button className="btn btn-ghost" onClick={() => setShowPull(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handlePull} disabled={!pullName.trim()}>
                Pull
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

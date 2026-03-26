import { useState, useEffect, useCallback, Fragment } from "react";
import { dockerApi, DockerImage } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { TrashIcon, DownloadIcon, WarningIcon, InspectIcon, BroomIcon, TagIcon } from "../components/Icons";

export default function Images() {
  const [images, setImages] = useState<DockerImage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");

  const [showPull, setShowPull] = useState(false);
  const [pullName, setPullName] = useState("");
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [inspecting, setInspecting] = useState<string | null>(null);
  const [inspectData, setInspectData] = useState<string>("");
  const [showTag, setShowTag] = useState<string | null>(null);
  const [tagTarget, setTagTarget] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [batchLoading, setBatchLoading] = useState(false);
  const { confirm, ConfirmDialogProps } = useConfirm();



  const fetchImages = useCallback(async () => {
    try {
      setError(null);
      const list = await dockerApi.listImages();
      setImages(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchImages();
    const interval = setInterval(fetchImages, 15000);
    return () => clearInterval(interval);
  }, [fetchImages]);

  const handlePull = async () => {
    if (!pullName.trim()) return;
    const name = pullName.trim();
    // Fire-and-forget: close form immediately
    globalToast("success", `Pulling image "${name}"... This may take a moment.`);
    setPullName("");
    setShowPull(false);
    dockerApi.pullImage(name)
      .then(() => { globalToast("success", `Image "${name}" pulled successfully`); fetchImages(); })
      .catch((e) => globalToast("error", `Pull failed: ${e}`));
  };

  const handleRemove = async (imageId: string, name: string) => {
    const ok = await confirm({ title: "Remove Image", message: `Remove image "${name}"?\n\nIf the image is used by running containers, they will be stopped and removed automatically.`, confirmText: "Remove", variant: "danger" });
    if (!ok) return;
    setActionLoading(imageId);
    try {
      await dockerApi.removeImage(imageId, true);
      globalToast("success", `Image "${name}" removed`);
      await fetchImages();
    } catch (e) {
      const msg = String(e);
      if (msg.includes("being used by running container")) {
        globalToast("error", `Cannot remove "${name}" — it is used by a running container. Stop the container first.`);
      } else if (msg.includes("cannot be forced")) {
        globalToast("error", `Cannot force-remove "${name}" — stop related containers first.`);
      } else {
        globalToast("error", msg);
      }
    } finally {
      setActionLoading(null);
    }
  };

  const handlePrune = async () => {
    const ok = await confirm({ title: "Prune Images", message: "Remove all unused images? This cannot be undone.", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    setActionLoading("prune");
    try {
      await dockerApi.pruneImages();
      globalToast("success", "Unused images pruned");
      await fetchImages();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const handleInspect = async (imageId: string) => {
    if (inspecting === imageId) { setInspecting(null); return; }
    try {
      const data = await dockerApi.inspectImage(imageId);
      setInspectData(data);
      setInspecting(imageId);
    } catch (e) {
      globalToast("error", String(e));
    }
  };

  const handleTag = async (source: string) => {
    if (!tagTarget.trim()) return;
    setActionLoading("tag");
    try {
      await dockerApi.tagImage(source, tagTarget.trim());
      globalToast("success", `Image tagged as "${tagTarget}"`);
      setShowTag(null);
      setTagTarget("");
      await fetchImages();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const filteredImages = images.filter((img) => {
    if (!searchTerm) return true;
    const term = searchTerm.toLowerCase();
    return (
      img.Repository.toLowerCase().includes(term) ||
      img.Tag.toLowerCase().includes(term) ||
      img.Id.toLowerCase().includes(term)
    );
  });

  const totalSize = images.reduce((sum, img) => {
    const match = img.Size.match(/([\d.]+)\s*(GB|MB|KB)/i);
    if (match) {
      const val = parseFloat(match[1]);
      const unit = match[2].toUpperCase();
      if (unit === "GB") return sum + val * 1024;
      if (unit === "MB") return sum + val;
      if (unit === "KB") return sum + val / 1024;
    }
    return sum;
  }, 0);

  // Clear selection on search change
  useEffect(() => { setSelected(new Set()); }, [searchTerm]);

  const toggleSelect = (id: string) => setSelected(prev => {
    const next = new Set(prev);
    next.has(id) ? next.delete(id) : next.add(id);
    return next;
  });
  const toggleAll = () => {
    if (selected.size === filteredImages.length) setSelected(new Set());
    else setSelected(new Set(filteredImages.map(i => i.Id)));
  };

  const handleBatchRemove = async () => {
    const names = filteredImages.filter(i => selected.has(i.Id)).map(i => `${i.Repository}:${i.Tag}`);
    if (names.length === 0) return;
    const ok = await confirm({ title: "Remove Selected Images", message: `Remove ${names.length} image${names.length > 1 ? "s" : ""}?\n\n${names.join("\n")}\n\nThis cannot be undone.`, confirmText: `Remove ${names.length}`, variant: "danger" });
    if (!ok) return;
    setBatchLoading(true);
    let ok_count = 0;
    for (const img of filteredImages.filter(i => selected.has(i.Id))) {
      try { await dockerApi.removeImage(img.Id, true); ok_count++; } catch { /* continue */ }
    }
    globalToast("success", `Removed ${ok_count} image${ok_count > 1 ? "s" : ""}`);
    setSelected(new Set());
    setBatchLoading(false);
    fetchImages();
  };

  if (loading) {
    return (
      <>
        <div className="content-header"><h1>Docker Images</h1></div>
        <div className="loading-screen"><div className="spinner" /><span>Loading images...</span></div>
      </>
    );
  }

  return (
    <>
      <div className="content-header">
        <h1>
          Docker Images
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)", fontWeight: 400, marginLeft: 12 }}>
            {images.length} image{images.length !== 1 ? "s" : ""} · {totalSize > 1024 ? `${(totalSize / 1024).toFixed(1)} GB` : `${totalSize.toFixed(0)} MB`}
          </span>
        </h1>
        <div className="content-header-actions">
          <input
            className="input"
            placeholder="Search images..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            style={{ width: 200 }}
          />
          <button className="btn btn-ghost" onClick={handlePrune} disabled={actionLoading === "prune"}>
            {actionLoading === "prune" ? "Pruning..." : <><BroomIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> Prune</>}
          </button>
          <button className="btn btn-primary" onClick={() => setShowPull(!showPull)}>
            <DownloadIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> Pull Image
          </button>
          <button className="btn btn-ghost" onClick={fetchImages}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
        </div>
      </div>

      <div className="content-body">


        {/* Pull form */}
        {showPull && (
          <div className="card" style={{ marginBottom: 16, padding: 16 }}>
            <h3 style={{ margin: "0 0 12px", fontSize: "var(--text-base)" }}>Pull Docker Image</h3>
            <div style={{ display: "flex", gap: "12px", alignItems: "flex-end" }}>
              <div style={{ flex: 1 }}>
                <label style={{ display: "block", fontSize: "var(--text-sm)", color: "var(--text-secondary)", marginBottom: "4px" }}>Image name</label>
                <input
                  className="input"
                  value={pullName}
                  onChange={e => setPullName(e.target.value)}
                  placeholder="nginx:latest, ubuntu:22.04, docker.io/library/redis..."
                  style={{ width: "100%" }}
                  onKeyDown={e => e.key === "Enter" && handlePull()}
                  autoFocus
                />
              </div>
              <button className="btn btn-primary" onClick={handlePull} disabled={!pullName.trim()}>
                Pull
              </button>
              <button className="btn btn-ghost" onClick={() => setShowPull(false)}>Cancel</button>
            </div>
          </div>
        )}

        {error && (
          <div className="card" style={{ borderColor: "var(--accent-yellow)", marginBottom: 16 }}>
            <p style={{ color: "var(--accent-yellow)", fontSize: "var(--text-sm)", display: "flex", alignItems: "center", gap: 6 }}><WarningIcon size={14} /> Could not connect to Docker: {error}</p>
          </div>
        )}

        {/* Batch action bar */}
        {selected.size > 0 && (
          <div style={{
            display: "flex", alignItems: "center", gap: 12, padding: "10px 16px", marginBottom: 12,
            background: "rgba(88,166,255,0.08)", border: "1px solid rgba(88,166,255,0.25)",
            borderRadius: "var(--radius-md)", animation: "slideUp 200ms ease",
          }}>
            <span style={{ fontSize: "var(--text-sm)", color: "var(--accent-blue)", fontWeight: 600 }}>
              {selected.size} selected
            </span>
            <div style={{ flex: 1 }} />
            <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)" }}
              onClick={handleBatchRemove} disabled={batchLoading}>
              {batchLoading ? "Removing..." : <><TrashIcon size={12} /> Remove Selected</>}
            </button>
            <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)" }}
              onClick={() => setSelected(new Set())}>
              Clear
            </button>
          </div>
        )}

        {filteredImages.length > 0 ? (
          <table className="data-table">
            <thead>
              <tr>
                <th style={{ width: 36, textAlign: "center" }}>
                  <input type="checkbox" checked={filteredImages.length > 0 && selected.size === filteredImages.length}
                    onChange={toggleAll} style={{ accentColor: "var(--accent-blue)", cursor: "pointer" }} />
                </th>
                <th>Repository</th>
                <th>Tag</th>
                <th>Image ID</th>
                <th>Created</th>
                <th>Size</th>
                <th style={{ width: "160px" }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredImages.map((img) => (
                <Fragment key={`${img.Repository}:${img.Tag}:${img.Id}`}>
                <tr style={{ background: selected.has(img.Id) ? "rgba(88,166,255,0.06)" : undefined }}>
                    <td style={{ textAlign: "center" }} onClick={e => e.stopPropagation()}>
                      <input type="checkbox" checked={selected.has(img.Id)} onChange={() => toggleSelect(img.Id)}
                        style={{ accentColor: "var(--accent-blue)", cursor: "pointer" }} />
                    </td>
                    <td style={{ fontWeight: 500 }}>{img.Repository}</td>
                    <td>
                      <span style={{
                        padding: "2px 8px", borderRadius: "var(--radius-sm)",
                        background: img.Tag === "latest" ? "rgba(63, 185, 80, 0.1)" : "rgba(88, 166, 255, 0.1)",
                        color: img.Tag === "latest" ? "var(--accent-green)" : "var(--accent-blue)",
                        fontSize: "var(--text-xs)", fontWeight: 500,
                      }}>
                        {img.Tag}
                      </span>
                    </td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                      {img.Id.replace("sha256:", "").substring(0, 12)}
                    </td>
                    <td style={{ color: "var(--text-secondary)", fontSize: "var(--text-sm)" }}>{img.CreatedAt}</td>
                    <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-sm)" }}>{img.Size}</td>
                    <td>
                      <div style={{ display: "flex", gap: "4px" }}>
                        <button
                          className="btn btn-ghost"
                          style={{ fontSize: "var(--text-xs)", padding: "2px 8px" }}
                          onClick={() => handleInspect(img.Id)}
                        >
                          {inspecting === img.Id ? "Hide" : <InspectIcon size={12} />}
                        </button>
                        <button
                          className="btn btn-ghost"
                          style={{ fontSize: "var(--text-xs)", padding: "2px 8px" }}
                          onClick={() => { setShowTag(showTag === img.Id ? null : img.Id); setTagTarget(""); }}
                        >
                          <TagIcon size={12} />
                        </button>
                        <button
                          className="btn btn-ghost"
                          style={{ fontSize: "var(--text-xs)", padding: "2px 8px", color: "var(--accent-red)" }}
                          onClick={() => handleRemove(img.Id, `${img.Repository}:${img.Tag}`)}
                          disabled={actionLoading === img.Id}
                        >
                          {actionLoading === img.Id ? "..." : <TrashIcon size={12} />}
                        </button>
                      </div>
                    </td>
                </tr>
                {/* Tag form inline */}
                {showTag === img.Id && (
                  <tr>
                    <td colSpan={6} style={{ padding: "8px 16px", background: "var(--bg-secondary)" }}>
                      <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                        <span style={{ fontSize: "var(--text-sm)", color: "var(--text-secondary)" }}>
                          Tag {img.Repository}:{img.Tag} as:
                        </span>
                        <input
                          className="input"
                          value={tagTarget}
                          onChange={e => setTagTarget(e.target.value)}
                          placeholder="myrepo/myimage:v1.0"
                          style={{ flex: 1 }}
                          onKeyDown={e => e.key === "Enter" && handleTag(`${img.Repository}:${img.Tag}`)}
                          autoFocus
                        />
                        <button className="btn btn-primary" onClick={() => handleTag(`${img.Repository}:${img.Tag}`)} disabled={actionLoading === "tag" || !tagTarget.trim()} style={{ fontSize: "var(--text-sm)" }}>
                          {actionLoading === "tag" ? "..." : "Tag"}
                        </button>
                        <button className="btn btn-ghost" onClick={() => setShowTag(null)} style={{ fontSize: "var(--text-sm)" }}>Cancel</button>
                      </div>
                    </td>
                  </tr>
                )}
                {/* Inspect data */}
                {inspecting === img.Id && (
                  <tr>
                    <td colSpan={6} style={{ padding: 0 }}>
                      <pre style={{ margin: 0, padding: "12px 16px", background: "var(--bg-secondary)", fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "300px", color: "var(--text-secondary)" }}>
                        {inspectData}
                      </pre>
                    </td>
                  </tr>
                )}
                </Fragment>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ color: "var(--text-muted)" }}>
                <rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18"/><path d="M9 3v18"/>
              </svg>
            </div>
            <div className="empty-state-title">{searchTerm ? "No matching images" : "No Docker images"}</div>
            <div className="empty-state-text">
              {searchTerm ? "Try a different search term." : "Click \"Pull Image\" to download your first image."}
            </div>
          </div>
        )}
      </div>
      <ConfirmDialog {...ConfirmDialogProps} />
    </>
  );
}

import { useState, useEffect, useRef, useDeferredValue, useCallback } from "react";
import { dockerApi, DockerImage } from "../lib/api";
import { globalToast } from "../lib/globalToast";
import { ConfirmDialog, useConfirm } from "../components/ConfirmDialog";
import { useAtom, useAtomValue } from "jotai";
import { imagesAtom, dockerLoadingAtom } from "../store/dockerAtom";
import { TrashIcon, DownloadIcon, InspectIcon, BroomIcon, TagIcon } from "../components/Icons";
import { useVirtualizer } from "@tanstack/react-virtual";
import ContextMenu, { ContextMenuItem } from "../components/ContextMenu";
import { useHotkeys } from "../hooks/useHotkeys";

/* ===== Virtualized Image Rows ===== */
function VirtualImageRows({
  filteredImages, selected, actionLoading, gridCols, rowHeight, toggleSelect,
  handleInspect, handleRemove, inspecting, inspectData,
  showTag, setShowTag, tagTarget, setTagTarget, handleTag, onContextMenu,
}: {
  filteredImages: DockerImage[];
  selected: Set<string>;
  actionLoading: string | null;
  gridCols: string;
  rowHeight: number;
  toggleSelect: (id: string) => void;
  handleInspect: (id: string) => void;
  handleRemove: (id: string, name: string) => void;
  inspecting: string | null;
  inspectData: string;
  showTag: string | null;
  setShowTag: (id: string | null) => void;
  tagTarget: string;
  setTagTarget: (v: string) => void;
  handleTag: (source: string) => void;
  onContextMenu: (e: React.MouseEvent, img: DockerImage) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filteredImages.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });

  return (
    <div ref={scrollRef} className="vtable-scroll">
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((vRow) => {
          const img = filteredImages[vRow.index];
          return (
            <div key={img.Id} style={{ position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${vRow.start}px)` }}>
              <div
                className={`vtable-row${selected.has(img.Id) ? ' selected' : ''}`}
                style={{ display: 'grid', gridTemplateColumns: gridCols, height: rowHeight }}
                onContextMenu={(e) => onContextMenu(e, img)}
              >
                <div className="vtable-cell" style={{ textAlign: 'center' }}>
                  <input type="checkbox" checked={selected.has(img.Id)} onChange={() => toggleSelect(img.Id)}
                    style={{ accentColor: 'var(--accent-blue)', cursor: 'pointer' }} />
                </div>
                <div className="vtable-cell" style={{ fontWeight: 500 }}>{img.Repository}</div>
                <div className="vtable-cell">
                  <span style={{
                    padding: '2px 8px', borderRadius: 'var(--radius-sm)',
                    background: img.Tag === 'latest' ? 'rgba(63,185,80,0.1)' : 'rgba(88,166,255,0.1)',
                    color: img.Tag === 'latest' ? 'var(--accent-green)' : 'var(--accent-blue)',
                    fontSize: 'var(--text-xs)', fontWeight: 500,
                  }}>{img.Tag}</span>
                </div>
                <div className="vtable-cell" style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--text-muted)' }}>
                  {img.Id.replace('sha256:', '').substring(0, 12)}
                </div>
                <div className="vtable-cell" style={{ color: 'var(--text-secondary)', fontSize: 'var(--text-sm)' }}>{img.CreatedAt}</div>
                <div className="vtable-cell" style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>{img.Size}</div>
                <div className="vtable-cell">
                  <div style={{ display: 'flex', gap: 4 }}>
                    <button className="btn btn-ghost" style={{ fontSize: 'var(--text-xs)', padding: '2px 8px' }}
                      onClick={() => handleInspect(img.Id)}>
                      {inspecting === img.Id ? 'Hide' : <InspectIcon size={12} />}
                    </button>
                    <button className="btn btn-ghost" style={{ fontSize: 'var(--text-xs)', padding: '2px 8px' }}
                      onClick={() => { setShowTag(showTag === img.Id ? null : img.Id); setTagTarget(''); }}>
                      <TagIcon size={12} />
                    </button>
                    <button className="btn btn-ghost" style={{ fontSize: 'var(--text-xs)', padding: '2px 8px', color: 'var(--accent-red)' }}
                      onClick={() => handleRemove(img.Id, `${img.Repository}:${img.Tag}`)} disabled={actionLoading === img.Id}>
                      {actionLoading === img.Id ? '...' : <TrashIcon size={12} />}
                    </button>
                  </div>
                </div>
              </div>
              {/* Expandable panels */}
              {showTag === img.Id && (
                <div style={{ padding: '8px 16px', background: 'var(--bg-secondary)', borderBottom: '1px solid var(--border-subtle)' }}>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    <span style={{ fontSize: 'var(--text-sm)', color: 'var(--text-secondary)' }}>Tag {img.Repository}:{img.Tag} as:</span>
                    <input className="input" value={tagTarget} onChange={e => setTagTarget(e.target.value)}
                      placeholder="myrepo/myimage:v1.0" style={{ flex: 1 }}
                      onKeyDown={e => e.key === 'Enter' && handleTag(`${img.Repository}:${img.Tag}`)} autoFocus />
                    <button className="btn btn-primary" onClick={() => handleTag(`${img.Repository}:${img.Tag}`)} disabled={actionLoading === 'tag' || !tagTarget.trim()} style={{ fontSize: 'var(--text-sm)' }}>
                      {actionLoading === 'tag' ? '...' : 'Tag'}
                    </button>
                    <button className="btn btn-ghost" onClick={() => setShowTag(null)} style={{ fontSize: 'var(--text-sm)' }}>Cancel</button>
                  </div>
                </div>
              )}
              {inspecting === img.Id && (
                <pre style={{ margin: 0, padding: '12px 16px', background: 'var(--bg-secondary)', fontSize: 'var(--text-xs)', overflow: 'auto', maxHeight: 300, color: 'var(--text-secondary)', borderBottom: '1px solid var(--border-subtle)' }}>
                  {inspectData}
                </pre>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default function Images() {
  const [images, setImages] = useAtom(imagesAtom);
  const loading = useAtomValue(dockerLoadingAtom);

  const refreshImages = useCallback(async () => {
    try {
      const list = await dockerApi.listImages();
      setImages(list);
    } catch { /* ignore */ }
  }, [setImages]);
  const [searchTerm, setSearchTerm] = useState("");
  const deferredSearch = useDeferredValue(searchTerm);

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
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; image: DockerImage } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  useHotkeys({
    "mod+k": () => searchRef.current?.focus(),
    "escape": () => { setInspecting(null); setShowTag(null); setCtxMenu(null); },
    "delete": () => { if (selected.size > 0) handleBatchRemove(); },
    "backspace": () => { if (selected.size > 0) handleBatchRemove(); },
  });

  const openCtxMenu = (e: React.MouseEvent, img: DockerImage) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY, image: img });
  };

  // Auto-cleanup: remove stale selections when data changes
  useEffect(() => {
    setSelected(prev => {
      const validIds = new Set(images.map(i => i.Id));
      const next = new Set([...prev].filter(id => validIds.has(id)));
      return next.size !== prev.size ? next : prev;
    });
  }, [images]);

  const getCtxItems = (img: DockerImage): ContextMenuItem[] => [
    { label: "Inspect", icon: <InspectIcon size={14} />, action: () => handleInspect(img.Id) },
    { label: "Tag", icon: <TagIcon size={14} />, action: () => { setShowTag(img.Id); setTagTarget(""); } },
    { divider: true, label: "", action: () => {} },
    { label: "Copy ID", action: () => { navigator.clipboard.writeText(img.Id); globalToast("success", "Image ID copied"); } },
    { divider: true, label: "", action: () => {} },
    { label: "Remove", danger: true, action: () => handleRemove(img.Id, `${img.Repository}:${img.Tag}`) },
  ];

  const handlePull = async () => {
    if (!pullName.trim()) return;
    const name = pullName.trim();
    // Fire-and-forget: close form immediately
    globalToast("success", `Pulling image "${name}"... This may take a moment.`);
    setPullName("");
    setShowPull(false);
    dockerApi.pullImage(name)
      .then(() => { globalToast("success", `Image "${name}" pulled successfully`); })
      .catch((e) => globalToast("error", `Pull failed: ${e}`));
  };

  const handleRemove = async (imageId: string, name: string) => {
    const ok = await confirm({ title: "Remove Image", message: `Remove image "${name}"?\n\nIf the image is used by running containers, they will be stopped and removed automatically.`, confirmText: "Remove", variant: "danger" });
    if (!ok) return;
    setActionLoading(imageId);
    try {
      await dockerApi.removeImage(imageId, true);
      // Always clear from selection (avoid stale closure from confirm dialog)
      setSelected(prev => { const next = new Set(prev); next.delete(imageId); return next; });
      globalToast("success", `Image "${name}" removed`);
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
    await refreshImages();
  };

  const handlePrune = async () => {
    const ok = await confirm({ title: "Prune Images", message: "Remove all unused images? This cannot be undone.", confirmText: "Prune All", variant: "warning" });
    if (!ok) return;
    setActionLoading("prune");
    try {
      await dockerApi.pruneImages();
      setSelected(new Set());
      globalToast("success", "Unused images pruned");
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
    await refreshImages();
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
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      setActionLoading(null);
    }
  };

  const filteredImages = images.filter((img) => {
    if (!searchTerm) return true;
    const term = deferredSearch.toLowerCase();
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
    await refreshImages();
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
            ref={searchRef}
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

        {filteredImages.length > 0 ? (() => {
          const ROW_H = 48;
          const gridCols = '36px minmax(160px,1.5fr) 100px 120px minmax(100px,0.8fr) 80px 160px';
          return (
          <div className="vtable">
            <div className="vtable-header" style={{ display: 'grid', gridTemplateColumns: gridCols }}>
              <div className="vtable-header-cell" style={{ textAlign: 'center' }}>
                <input type="checkbox" checked={filteredImages.length > 0 && selected.size === filteredImages.length}
                  onChange={toggleAll} style={{ accentColor: 'var(--accent-blue)', cursor: 'pointer' }} />
              </div>
              <div className="vtable-header-cell">Repository</div>
              <div className="vtable-header-cell">Tag</div>
              <div className="vtable-header-cell">Image ID</div>
              <div className="vtable-header-cell">Created</div>
              <div className="vtable-header-cell">Size</div>
              <div className="vtable-header-cell">Actions</div>
            </div>
            <VirtualImageRows
              filteredImages={filteredImages}
              selected={selected}
              actionLoading={actionLoading}
              gridCols={gridCols}
              rowHeight={ROW_H}
              toggleSelect={toggleSelect}
              handleInspect={handleInspect}
              handleRemove={handleRemove}
              inspecting={inspecting}
              inspectData={inspectData}
              showTag={showTag}
              setShowTag={setShowTag}
              tagTarget={tagTarget}
              setTagTarget={setTagTarget}
              handleTag={handleTag}
              onContextMenu={openCtxMenu}
            />
          </div>
          );
        })() : (
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
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={getCtxItems(ctxMenu.image)} onClose={() => setCtxMenu(null)} />}
    </>
  );
}

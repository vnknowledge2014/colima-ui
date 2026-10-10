<script lang="ts">
  /**
   * SVG renderer for the Docker topology graph.
   *
   * Layout is delegated to ELK via `lib/topology-elk.ts`; this component owns
   * only viewport (pan/zoom), pointer interaction, and drawing.
   */
  import { untrack } from "svelte";
  import type { TopologyNode, TopologyEdge } from "../../lib/api/topology";
  import * as Icons from "../Icons.svelte";
  import { t } from "../../lib/i18n.svelte";
  import {
    layoutTopology,
    emptyLayout,
    displayLabel,
    titleLabel,
    GROUP_TITLE_LEFT,
    GROUP_COUNT_RESERVE,
    type TopologyLayout,
  } from "../../lib/topology-elk";

  interface Props {
    nodes: TopologyNode[];
    edges: TopologyEdge[];
    selectedId: string | null;
    onSelect: (id: string | null) => void;
  }

  let { nodes, edges, selectedId, onSelect }: Props = $props();

  /**
   * Icon glyphs, stripped of their `<svg>` wrapper so they can live in a
   * `<symbol>` and be stamped with `<use>`. Fifty containers referencing one
   * symbol is one copy of the path data; inlining the markup per node would be
   * fifty. The wrapper also carries React-style `strokeWidth`, which is not a
   * real SVG attribute — the stroke properties are re-declared on the symbol.
   */
  const ICON_INNER = (svg: string) =>
    svg.replace(/^<svg[^>]*>/, "").replace(/<\/svg>\s*$/, "");

  const KIND_ICON: Record<string, string> = {
    container: ICON_INNER(Icons.Container),
    network: ICON_INNER(Icons.Network),
    volume: ICON_INNER(Icons.Volume),
    project: ICON_INNER(Icons.Compose),
    image: ICON_INNER(Icons.Image),
    // A declared-but-not-created service is a container-shaped hole, so it uses
    // the container glyph; the dashed chip and muted colour carry the "does not
    // exist yet" part.
    service: ICON_INNER(Icons.Container),
  };

  const KIND_ORDER = ["container", "network", "volume", "project", "image", "service"];

  /** Chip geometry, mirroring what `topology-elk` reserved for each box. */
  const ICON_SIZE = 14;
  const ICON_INSET = 8;
  const TEXT_LEFT = 28;
  const MARKER_INSET = 13;

  /**
   * `$state.raw` because the layout holds Maps: a deep reactive proxy would wrap
   * every lookup, and nothing here mutates the object in place — each ELK run
   * replaces it wholesale.
   */
  let layout = $state.raw<TopologyLayout>(emptyLayout());
  /**
   * Positions the user dragged a node to, kept apart from the ELK result so a
   * refresh re-lays out the graph without discarding hand placement.
   */
  let overrides = $state<Record<string, { x: number; y: number }>>({});
  let laidOut = $state(false);

  let view = $state({ x: 0, y: 0, scale: 1 });
  let hoveredId = $state<string | null>(null);
  let svgEl = $state<SVGSVGElement | null>(null);
  let dragging: { id: string } | null = null;
  let panning: { startX: number; startY: number; originX: number; originY: number } | null = null;
  /** Guards against an earlier ELK run resolving after a later one. */
  let runId = 0;

  /**
   * Identity of the graph's *shape*. Status flips and label changes must not
   * appear here: a poll that only changes a container's health would otherwise
   * re-lay-out the graph under the user's cursor.
   */
  const graphKey = $derived(
    nodes.map((n) => n.id).join("|") + "##" + edges.map((e) => `${e.from}>${e.to}>${e.kind}`).join("|")
  );

  /** Shape the current `layout` was built from, so identical refreshes are free. */
  let builtKey: string | null = null;

  $effect(() => {
    const key = graphKey;
    // The body reads props; untracking it keeps the key as the only dependency.
    untrack(() => {
      if (key === builtKey) return;
      builtKey = key;
      void rebuild();
    });
  });

  /**
   * Re-run ELK for the current graph.
   *
   * ELK is deterministic, so nothing needs carrying over between runs the way
   * the old force simulation did — the same graph lands in the same place. Only
   * hand-dragged positions survive, and those live in `overrides`.
   */
  async function rebuild() {
    const id = ++runId;
    const next = await layoutTopology(nodes, edges);
    // A newer run started while this one was in the Worker; its result is the
    // current one and this stale layout must not overwrite it.
    if (id !== runId) return;
    const first = !laidOut;
    layout = next;
    laidOut = true;
    // Only re-frame the first time. Refitting on every poll would throw away a
    // viewport the user had panned and zoomed.
    if (first) fitToView();
  }

  /** Edges ELK actually laid out — already deduped and filtered by the adapter. */
  const renderEdges = $derived(layout.edges);

  /** ELK coordinates with any hand-dragged position layered on top. */
  const positions = $derived.by(() => {
    const map = new Map(layout.positions);
    for (const [id, pos] of Object.entries(overrides)) {
      if (map.has(id)) map.set(id, pos);
    }
    return map;
  });

  /** Where an edge should attach: a node's centre, or a group box's top edge. */
  function anchorOf(id: string) {
    return positions.get(id) ?? layout.groupAnchors.get(id);
  }

  /** Neighbours of the hovered node, used to dim everything else. */
  const highlighted = $derived.by(() => {
    const focus = hoveredId ?? selectedId;
    if (!focus) return null;
    const set = new Set<string>([focus]);
    for (const e of edges) {
      if (e.from === focus) set.add(e.to);
      else if (e.to === focus) set.add(e.from);
    }
    return set;
  });

  /** Zoom a step about the canvas centre, matching what the buttons imply. */
  function zoomBy(factor: number) {
    const rect = svgEl?.getBoundingClientRect();
    if (!rect) return;
    const next = Math.min(Math.max(view.scale * factor, 0.2), 4);
    const cx = rect.width / 2;
    const cy = rect.height / 2;
    view = {
      scale: next,
      x: cx - ((cx - view.x) / view.scale) * next,
      y: cy - ((cy - view.y) / view.scale) * next,
    };
  }

  export function fitToView() {
    if (layout.width === 0 || layout.height === 0 || !svgEl) {
      view = { x: 0, y: 0, scale: 1 };
      return;
    }
    const pad = 60;
    const rect = svgEl.getBoundingClientRect();
    // ELK lays out from the origin, so the drawing spans 0..width/height.
    const w = layout.width + pad * 2;
    const h = layout.height + pad * 2;
    const scale = Math.min(rect.width / w, rect.height / h, 1.5);
    view = {
      scale,
      x: rect.width / 2 - (layout.width / 2) * scale,
      y: rect.height / 2 - (layout.height / 2) * scale,
    };
  }

  /** Discard hand-placed positions, re-run the layout, then re-frame. */
  export function resetLayout() {
    overrides = {};
    laidOut = false;
    void rebuild();
  }

  function toGraphCoords(clientX: number, clientY: number) {
    const rect = svgEl?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    return {
      x: (clientX - rect.left - view.x) / view.scale,
      y: (clientY - rect.top - view.y) / view.scale,
    };
  }

  function onWheel(event: WheelEvent) {
    event.preventDefault();
    const rect = svgEl?.getBoundingClientRect();
    if (!rect) return;
    const factor = event.deltaY < 0 ? 1.12 : 1 / 1.12;
    const nextScale = Math.min(Math.max(view.scale * factor, 0.2), 4);
    // Zoom about the cursor so the point under the pointer stays put.
    const px = event.clientX - rect.left;
    const py = event.clientY - rect.top;
    view = {
      scale: nextScale,
      x: px - ((px - view.x) / view.scale) * nextScale,
      y: py - ((py - view.y) / view.scale) * nextScale,
    };
  }

  function onNodePointerDown(event: PointerEvent, id: string) {
    if (!positions.has(id)) return;
    event.stopPropagation();
    dragging = { id };
    (event.target as Element).setPointerCapture?.(event.pointerId);
    onSelect(id);
  }

  function onBackgroundPointerDown(event: PointerEvent) {
    panning = { startX: event.clientX, startY: event.clientY, originX: view.x, originY: view.y };
  }

  function onPointerMove(event: PointerEvent) {
    if (dragging) {
      // Only the dragged node moves. The old force layout relaxed neighbours
      // around it, which meant every drag quietly rearranged the graph; with a
      // deterministic layout the rest of the drawing is a fixed reference the
      // user is navigating by, so it must stay put.
      overrides = { ...overrides, [dragging.id]: toGraphCoords(event.clientX, event.clientY) };
      return;
    }
    if (panning) {
      view = {
        ...view,
        x: panning.originX + (event.clientX - panning.startX),
        y: panning.originY + (event.clientY - panning.startY),
      };
    }
  }

  function onPointerUp() {
    // The dragged position stays in `overrides`: the user placed it
    // deliberately, so the next refresh must not pull it back.
    dragging = null;
    panning = null;
  }
</script>

<svg
  bind:this={svgEl}
  class="topology-canvas"
  role="presentation"
  onwheel={onWheel}
  onpointerdown={onBackgroundPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointerleave={onPointerUp}
>
  <defs>
    <!-- One definition per kind, stamped by every node of that kind. The stroke
         properties live here so `<use>` only has to supply position and colour. -->
    {#each KIND_ORDER as kind (kind)}
      <symbol
        id="topo-icon-{kind}"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        {@html KIND_ICON[kind]}
      </symbol>
    {/each}
  </defs>

  <g transform="translate({view.x} {view.y}) scale({view.scale})">
    <!-- Network boxes sit behind everything: they are the ground the contained
         containers are drawn on, not shapes competing with them. -->
    {#each layout.groups as group (group.id)}
      <g
        class="group"
        class:dimmed={highlighted && !highlighted.has(group.id)}
        class:selected={selectedId === group.id}
        role="button"
        tabindex="0"
        onpointerdown={(e) => {
          e.stopPropagation();
          onSelect(group.id);
        }}
        onmouseenter={() => (hoveredId = group.id)}
        onmouseleave={() => (hoveredId = null)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect(group.id);
          }
        }}
      >
        <rect x={group.x} y={group.y} width={group.width} height={group.height} rx="10" class="group-box" />
        <!-- Title bar: the box is a named thing, not an anonymous region. -->
        <rect x={group.x} y={group.y} width={group.width} height="26" rx="10" class="group-bar" />
        <use
          href="#topo-icon-network"
          x={group.x + 10}
          y={group.y + 7}
          width="12"
          height="12"
          class="group-icon"
        />
        <text x={group.x + GROUP_TITLE_LEFT} y={group.y + 17} class="group-label">
          {titleLabel(displayLabel(group.label), group.width - GROUP_TITLE_LEFT - GROUP_COUNT_RESERVE)}
        </text>
        <title>{group.label}</title>
        <text x={group.x + group.width - 10} y={group.y + 17} text-anchor="end" class="group-count">
          {group.count}
        </text>
      </g>
    {/each}

    {#each renderEdges as edge (edge.from + ">" + edge.to)}
      {@const a = anchorOf(edge.from)}
      {@const b = anchorOf(edge.to)}
      {#if a && b}
        <line
          x1={a.x}
          y1={a.y}
          x2={b.x}
          y2={b.y}
          class="edge edge-{edge.kind}"
          class:dimmed={highlighted && !(highlighted.has(edge.from) && highlighted.has(edge.to))}
        />
      {/if}
    {/each}

    {#each nodes as node (node.id)}
      {@const pos = positions.get(node.id)}
      {@const box = layout.sizes.get(node.id)}
      {#if pos && box}
        {@const left = pos.x - box.width / 2}
        {@const top = pos.y - box.height / 2}
        <g
          class="node kind-{node.kind} status-{node.status}"
          class:dimmed={highlighted && !highlighted.has(node.id)}
          class:selected={selectedId === node.id}
          role="button"
          tabindex="0"
          aria-label="{node.kind}: {node.label}"
          onpointerdown={(e) => onNodePointerDown(e, node.id)}
          onmouseenter={() => (hoveredId = node.id)}
          onmouseleave={() => (hoveredId = null)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onSelect(node.id);
            }
          }}
        >
          <rect x={left} y={top} width={box.width} height={box.height} rx="6" class="chip" />
          <!-- Status stripe down the left edge. Clipped by the chip's own round
               corners would need a mask; a plain bar inset by the radius reads
               the same and costs nothing. -->
          <rect x={left} y={top + 6} width="3" height={box.height - 12} rx="1.5" class="chip-stripe" />
          <use
            href="#topo-icon-{node.kind}"
            x={left + ICON_INSET}
            y={top + (box.height - ICON_SIZE) / 2}
            width={ICON_SIZE}
            height={ICON_SIZE}
            class="chip-icon"
          />
          <text x={left + TEXT_LEFT} y={pos.y + 4} class="node-label">{displayLabel(node.label)}</text>
          <!-- Status is encoded as shape as well as colour: filled dot, hollow
               dot, triangle. Colour alone would hide running-vs-unhealthy from
               anyone with red-green colour blindness — the one distinction on
               this screen that matters most. -->
          {#if node.status === "running"}
            <circle cx={left + box.width - MARKER_INSET} cy={pos.y} r="3.5" class="marker-running" />
          {:else if node.status === "unhealthy"}
            {@const mx = left + box.width - MARKER_INSET}
            <path d="M{mx},{pos.y - 4} L{mx + 4.5},{pos.y + 3.5} L{mx - 4.5},{pos.y + 3.5} Z"
              class="marker-unhealthy" />
          {:else if node.status === "stopped"}
            <circle cx={left + box.width - MARKER_INSET} cy={pos.y} r="3" class="marker-stopped" />
          {:else if node.status === "notCreated"}
            <!-- A dashed ring, not a hollow one: "does not exist" has to be
                 distinguishable from "exists but stopped", including in
                 greyscale, because the fix for each is different. -->
            <circle
              cx={left + box.width - MARKER_INSET}
              cy={pos.y}
              r="3"
              class="marker-not-created"
            />
          {/if}
        </g>
      {/if}
    {/each}
  </g>
</svg>

<!-- Wheel-zoom and drag-to-pan are invisible affordances; these buttons are how
     anyone finds out the canvas does either. Reset lives here rather than in the
     page header because it acts on this canvas. -->
<div class="canvas-controls">
  <button
    class="canvas-btn"
    onclick={() => zoomBy(1.25)}
    aria-label={t("topology.zoom_in", { default: "Zoom in" })}
    title={t("topology.zoom_in", { default: "Zoom in" })}
  >
    {@html Icons.ZoomIn}
  </button>
  <button
    class="canvas-btn"
    onclick={() => zoomBy(1 / 1.25)}
    aria-label={t("topology.zoom_out", { default: "Zoom out" })}
    title={t("topology.zoom_out", { default: "Zoom out" })}
  >
    {@html Icons.ZoomOut}
  </button>
  <button
    class="canvas-btn"
    onclick={fitToView}
    aria-label={t("topology.fit", { default: "Fit to view" })}
    title={t("topology.fit", { default: "Fit to view" })}
  >
    {@html Icons.Fit}
  </button>
  <button
    class="canvas-btn"
    onclick={resetLayout}
    aria-label={t("topology.reset_layout", { default: "Reset layout" })}
    title={t("topology.reset_layout", { default: "Reset layout" })}
  >
    {@html Icons.Refresh}
  </button>
</div>

<style>
  .topology-canvas {
    width: 100%;
    height: 100%;
    display: block;
    cursor: grab;
    touch-action: none;
    background: var(--bg-primary);
  }

  .edge {
    stroke: var(--border-primary, #334155);
    stroke-width: 1.4;
  }

  .edge-network {
    stroke: var(--color-info, #38bdf8);
    stroke-opacity: 0.55;
  }

  .edge-volume {
    stroke: var(--color-warning, #f59e0b);
    stroke-opacity: 0.5;
  }

  .edge-project {
    stroke-dasharray: 4 3;
    stroke-opacity: 0.5;
  }

  .edge-image {
    stroke-opacity: 0.28;
  }

  /* `depends_on` is the only edge that means "before", not "attached to", so it
     gets its own colour rather than another shade of the attachment lines. */
  .edge-dependsOn {
    stroke: var(--accent-purple, #bc8cff);
    stroke-opacity: 0.8;
    stroke-width: 1.8;
    stroke-dasharray: 7 3;
  }

  .group {
    cursor: pointer;
  }

  .group-box {
    fill: var(--bg-secondary, #1e293b);
    fill-opacity: 0.35;
    stroke: var(--color-info, #38bdf8);
    stroke-opacity: 0.4;
    stroke-width: 1.2;
    stroke-dasharray: 6 4;
  }

  .group-bar {
    fill: var(--color-info, #38bdf8);
    fill-opacity: 0.1;
  }

  .group-icon {
    color: var(--color-info, #38bdf8);
    pointer-events: none;
  }

  .group-label {
    fill: var(--color-info, #38bdf8);
    font-size: 11px;
    font-weight: 600;
    pointer-events: none;
    user-select: none;
  }

  .group-count {
    fill: var(--text-muted, #94a3b8);
    font-size: 10px;
    pointer-events: none;
    user-select: none;
  }

  .group.selected .group-box {
    stroke-opacity: 0.9;
    stroke-dasharray: none;
  }

  .node {
    cursor: pointer;
  }

  .chip {
    fill: var(--bg-secondary, #1e293b);
    stroke: var(--border-primary, #334155);
    stroke-width: 1;
  }

  /* Kind is carried by the icon's colour; status by the stripe and marker. Two
     independent channels, so neither has to compromise for the other. */
  .kind-container .chip-icon {
    color: var(--accent-blue, #58a6ff);
  }
  .kind-network .chip-icon {
    color: var(--color-info, #38bdf8);
  }
  .kind-volume .chip-icon {
    color: var(--color-warning, #f59e0b);
  }
  .kind-project .chip-icon {
    color: var(--accent-purple, #bc8cff);
  }
  .kind-image .chip-icon {
    color: var(--text-muted, #94a3b8);
  }
  .kind-service .chip-icon {
    color: var(--text-muted, #94a3b8);
  }

  /* A service that was never created is drawn as an outline: present in the
     compose file, absent from the engine. */
  .status-notCreated .chip {
    fill: none;
    stroke-dasharray: 4 3;
  }

  .status-notCreated .node-label {
    fill: var(--text-muted, #94a3b8);
  }

  .marker-not-created {
    fill: none;
    stroke: var(--text-muted, #94a3b8);
    stroke-width: 1.5;
    stroke-dasharray: 2 2;
  }

  .chip-stripe {
    fill: var(--text-muted, #94a3b8);
  }
  .status-running .chip-stripe {
    fill: var(--color-success, #22c55e);
  }
  .status-unhealthy .chip-stripe {
    fill: var(--color-danger, #ef4444);
  }
  /* A network, volume or image has no lifecycle of its own; a neutral stripe
     says "not applicable" rather than implying it is stopped. */
  .status-none .chip-stripe {
    fill: var(--border-primary, #334155);
  }

  .marker-running {
    fill: var(--color-success, #22c55e);
  }
  .marker-unhealthy {
    fill: var(--color-danger, #ef4444);
  }
  .marker-stopped {
    fill: none;
    stroke: var(--text-muted, #94a3b8);
    stroke-width: 1.5;
  }

  .node-label {
    fill: var(--text-primary, #e2e8f0);
    font-size: 11px;
    pointer-events: none;
    user-select: none;
  }

  .node.selected .chip {
    stroke: var(--accent-blue, #58a6ff);
    stroke-width: 2;
    filter: drop-shadow(0 0 6px var(--accent-blue, #58a6ff));
  }

  .dimmed {
    opacity: 0.15;
  }

  .canvas-controls {
    position: absolute;
    left: 12px;
    bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 3px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-secondary) 88%, transparent);
    border: 1px solid var(--border-primary);
  }

  .canvas-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .canvas-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .canvas-btn :global(svg) {
    width: 14px;
    height: 14px;
  }
</style>

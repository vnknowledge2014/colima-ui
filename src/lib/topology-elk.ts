/**
 * ELK-based layout for the Docker topology graph.
 *
 * Replaces the hand-written force simulation this page used to run. Force
 * layout was not deterministic in practice: the same graph settled differently
 * depending on how many iterations ran before paint, and every poll that added a
 * container nudged the whole drawing. ELK's layered algorithm places the same
 * graph at the same coordinates every time, and it understands containment —
 * which is what lets containers be drawn *inside* their network instead of
 * merely wired to it.
 *
 * Everything except `layoutTopology` is a pure function, so the mapping rules
 * are testable without a browser or a Worker.
 */

import type { TopologyNode, TopologyEdge } from "./api/topology";

/**
 * Every node is drawn as a chip of one height: icon, label, status marker on a
 * single line. Uniform height is what keeps a layered layout from looking
 * ragged, and the label sits *inside* the box rather than hanging under it, so
 * what ELK reserves is exactly what gets painted.
 */
export const CHIP_HEIGHT = 30;
/** Label starts after the icon; the status marker occupies the right end. */
const CHIP_TEXT_LEFT = 28;
const CHIP_TEXT_RIGHT = 22;
/** Roughly the advance width of the 11px label font. */
const LABEL_CHAR_WIDTH = 6;
export const MAX_LABEL_CHARS = 20;
const MIN_CHIP_WIDTH = 104;
const MAX_CHIP_WIDTH = 224;

/**
 * Title-bar geometry the group renderer paints: network icon at +10, name at
 * +27, count flush against the right edge. The box is sized by its children,
 * so without a floor the title bar overruns a narrow box — reserve this much
 * width so the name and count always fit.
 */
export const GROUP_TITLE_LEFT = 27;
export const GROUP_COUNT_RESERVE = 26;

export const ELK_LAYOUT_OPTIONS: Record<string, string> = {
  "elk.algorithm": "layered",
  "elk.direction": "RIGHT",
  // Without this, an edge crossing a group boundary is routed as if the group
  // were opaque and the groups end up overlapping each other.
  "elk.hierarchyHandling": "INCLUDE_CHILDREN",
  "elk.spacing.nodeNode": "36",
  "elk.layered.spacing.nodeNodeBetweenLayers": "72",
  // Top padding clears the group's 26px title bar; anything less and the first
  // row of chips is drawn under the network's own name.
  "elk.padding": "[top=38,left=20,bottom=20,right=20]",
  "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
};

export interface ElkNode {
  id: string;
  width?: number;
  height?: number;
  children?: ElkNode[];
  layoutOptions?: Record<string, string>;
  x?: number;
  y?: number;
}

export interface ElkGraph extends ElkNode {
  edges: Array<{ id: string; sources: string[]; targets: string[] }>;
}

export interface TopologyGroup {
  id: string;
  label: string;
  status: string;
  /** Containers drawn inside, shown in the box's title bar. */
  count: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface TopologyLayout {
  /** Centre point of every node that is drawn as a chip. */
  positions: Map<string, { x: number; y: number }>;
  /** Chip box ELK settled on, so the renderer draws exactly what was reserved. */
  sizes: Map<string, { width: number; height: number }>;
  /** Networks promoted to containment boxes; these are not in `positions`. */
  groups: TopologyGroup[];
  /** Attachment point for edges pointing at a group (top-centre of the box). */
  groupAnchors: Map<string, { x: number; y: number }>;
  /** Edges worth drawing — see `visibleEdges`. */
  edges: TopologyEdge[];
  width: number;
  height: number;
}

/** Label as drawn: truncated, so the reserved box matches the painted text. */
export function displayLabel(label: string): string {
  return label.length > MAX_LABEL_CHARS ? `${label.slice(0, MAX_LABEL_CHARS - 1)}…` : label;
}

/**
 * Truncate a group name to the width the box can actually afford.
 *
 * ELK sizes a network box from its containers plus padding — never from the
 * network's own name — so a long name would run past the box's rounded right
 * edge and collide with the container count. The renderer feeds the available
 * width in; the label shrinks to the box instead of overflowing it.
 */
export function titleLabel(label: string, availableWidth: number): string {
  const maxChars = Math.max(1, Math.floor(availableWidth / LABEL_CHAR_WIDTH));
  if (label.length <= maxChars) return label;
  return `${label.slice(0, Math.max(1, maxChars - 1))}…`;
}

function sizeFor(node: TopologyNode): { width: number; height: number } {
  const text = displayLabel(node.label).length * LABEL_CHAR_WIDTH;
  const width = Math.min(
    Math.max(CHIP_TEXT_LEFT + text + CHIP_TEXT_RIGHT, MIN_CHIP_WIDTH),
    MAX_CHIP_WIDTH
  );
  return { width, height: CHIP_HEIGHT };
}

/**
 * The network a container is drawn inside.
 *
 * Prefers the `primaryNetwork` the backend reports; falls back to the first
 * network edge so the grouping still works against an older backend rather than
 * silently flattening the graph.
 */
export function primaryNetworkOf(
  node: TopologyNode,
  edges: TopologyEdge[],
  knownNetworks: Set<string>
): string | null {
  const declared = (node.meta as Record<string, unknown> | undefined)?.primaryNetwork;
  if (typeof declared === "string" && declared && knownNetworks.has(declared)) {
    return declared;
  }
  for (const e of edges) {
    if (e.from === node.id && e.kind === "network" && knownNetworks.has(e.to)) return e.to;
  }
  return null;
}

/**
 * Networks that own at least one container become containment boxes.
 *
 * A network with no containers stays an ordinary node: an empty box carries the
 * same information and costs far more space.
 */
export function groupAssignments(
  nodes: TopologyNode[],
  edges: TopologyEdge[]
): { parentOf: Map<string, string>; groupIds: Set<string> } {
  const knownNetworks = new Set(nodes.filter((n) => n.kind === "network").map((n) => n.id));
  const parentOf = new Map<string, string>();
  const groupIds = new Set<string>();

  for (const node of nodes) {
    if (node.kind !== "container") continue;
    const parent = primaryNetworkOf(node, edges, knownNetworks);
    if (!parent) continue;
    parentOf.set(node.id, parent);
    groupIds.add(parent);
  }
  return { parentOf, groupIds };
}

/**
 * Edges worth drawing once containment is applied.
 *
 * A container's edge to its own primary network is dropped: the container is
 * already drawn inside that box, so the line would be a stub from a shape to the
 * box enclosing it. Edges to *other* networks are kept — a multi-homed container
 * is exactly what those lines are for.
 */
export function visibleEdges(
  edges: TopologyEdge[],
  parentOf: Map<string, string>
): TopologyEdge[] {
  const seen = new Set<string>();
  const out: TopologyEdge[] = [];
  for (const e of edges) {
    if (e.kind === "network" && parentOf.get(e.from) === e.to) continue;
    const key = `${e.from}>${e.to}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(e);
  }
  return out;
}

/** Assemble the ELK input graph. Pure — no ELK import needed to exercise it. */
export function buildElkGraph(nodes: TopologyNode[], edges: TopologyEdge[]): ElkGraph {
  const { parentOf, groupIds } = groupAssignments(nodes, edges);
  const byId = new Map(nodes.map((n) => [n.id, n]));

  const groups = new Map<string, ElkNode>();
  for (const id of groupIds) {
    groups.set(id, {
      id,
      children: [],
      layoutOptions: { "elk.padding": ELK_LAYOUT_OPTIONS["elk.padding"] },
    });
  }

  const children: ElkNode[] = [];
  for (const node of nodes) {
    if (groups.has(node.id)) continue; // becomes a box, sized by ELK
    const size = sizeFor(node);
    const elkNode: ElkNode = { id: node.id, ...size };
    const parent = parentOf.get(node.id);
    if (parent && groups.has(parent)) groups.get(parent)!.children!.push(elkNode);
    else children.push(elkNode);
  }
  for (const group of groups.values()) children.push(group);

  const drawn = visibleEdges(edges, parentOf);
  const elkEdges = drawn
    // An edge whose endpoint vanished would abort the whole ELK run, taking the
    // page down over one stale reference.
    .filter((e) => byId.has(e.from) && byId.has(e.to))
    .map((e, i) => ({ id: `e${i}`, sources: [e.from], targets: [e.to] }));

  return { id: "root", layoutOptions: ELK_LAYOUT_OPTIONS, children, edges: elkEdges };
}

/**
 * Convert ELK's parent-relative coordinates into the absolute centres the SVG
 * renderer draws with.
 */
export function readElkResult(
  result: ElkNode,
  nodes: TopologyNode[],
  edges: TopologyEdge[]
): TopologyLayout {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const { parentOf, groupIds } = groupAssignments(nodes, edges);

  const positions = new Map<string, { x: number; y: number }>();
  const sizes = new Map<string, { width: number; height: number }>();
  const groups: TopologyGroup[] = [];
  const groupAnchors = new Map<string, { x: number; y: number }>();
  let maxX = 0;
  let maxY = 0;

  const walk = (parent: ElkNode, offsetX: number, offsetY: number) => {
    for (const child of parent.children ?? []) {
      const x = offsetX + (child.x ?? 0);
      const y = offsetY + (child.y ?? 0);
      const w = child.width ?? 0;
      const h = child.height ?? 0;
      maxX = Math.max(maxX, x + w);
      maxY = Math.max(maxY, y + h);

      if (groupIds.has(child.id)) {
        const source = byId.get(child.id);
        groups.push({
          id: child.id,
          label: source?.label ?? child.id,
          status: source?.status ?? "none",
          count: (child.children ?? []).length,
          x,
          y,
          width: w,
          height: h,
        });
        groupAnchors.set(child.id, { x: x + w / 2, y });
        walk(child, x, y);
      } else {
        positions.set(child.id, { x: x + w / 2, y: y + h / 2 });
        sizes.set(child.id, { width: w, height: h });
      }
    }
  };
  walk(result, 0, 0);

  return {
    positions,
    sizes,
    groups,
    groupAnchors,
    edges: visibleEdges(edges, parentOf),
    width: maxX,
    height: maxY,
  };
}

/** Empty layout, used before the first run and for an empty graph. */
export function emptyLayout(): TopologyLayout {
  return {
    positions: new Map(),
    sizes: new Map(),
    groups: [],
    groupAnchors: new Map(),
    edges: [],
    width: 0,
    height: 0,
  };
}

type ElkEngine = { layout: (graph: ElkGraph) => Promise<ElkNode> };
let engine: ElkEngine | null = null;

/**
 * Load ELK once, preferring a Worker so a 200 ms layout never blocks input.
 *
 * The bundled build is the fallback rather than the default because it runs on
 * the main thread; it exists for environments without Worker support, where a
 * blocked frame still beats no graph.
 */
async function getEngine(): Promise<ElkEngine> {
  if (engine) return engine;
  if (typeof Worker !== "undefined") {
    try {
      const { default: ELK } = await import("elkjs/lib/elk-api.js");
      engine = new ELK({
        workerFactory: () =>
          new Worker(new URL("elkjs/lib/elk-worker.min.js", import.meta.url)),
      }) as unknown as ElkEngine;
      return engine;
    } catch {
      // Fall through to the bundled build.
    }
  }
  const { default: ELK } = await import("elkjs/lib/elk.bundled.js");
  engine = new ELK() as unknown as ElkEngine;
  return engine;
}

/** Lay out the graph. Resolves with absolute coordinates ready to draw. */
export async function layoutTopology(
  nodes: TopologyNode[],
  edges: TopologyEdge[]
): Promise<TopologyLayout> {
  if (nodes.length === 0) return emptyLayout();
  const elk = await getEngine();
  const result = await elk.layout(buildElkGraph(nodes, edges));
  return readElkResult(result, nodes, edges);
}

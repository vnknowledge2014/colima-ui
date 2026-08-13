import { describe, it, expect } from "vitest";
import {
  buildElkGraph,
  groupAssignments,
  primaryNetworkOf,
  readElkResult,
  titleLabel,
  visibleEdges,
  type ElkNode,
} from "./topology-elk";
import type { TopologyNode, TopologyEdge } from "./api/topology";

const node = (
  id: string,
  kind: string,
  extra: Partial<TopologyNode> = {}
): TopologyNode => ({
  id,
  kind,
  label: id.split(":")[1] ?? id,
  status: "running",
  meta: {},
  ...extra,
} as TopologyNode);

const edge = (from: string, to: string, kind: string): TopologyEdge =>
  ({ from, to, kind } as TopologyEdge);

describe("titleLabel", () => {
  it("keeps a short name unchanged", () => {
    expect(titleLabel("frontend-net", 100)).toBe("frontend-net");
  });

  it("truncates a long name to the width the box can afford", () => {
    // 174px available → 29 chars max; the 32-char name shrinks with an ellipsis.
    expect(titleLabel("my-production-backend-network-name", 174)).toBe(
      "my-production-backend-networ…"
    );
  });

  it("never returns an empty label even for a sliver of room", () => {
    expect(titleLabel("anything", 2)).toBe("a…");
  });
});

describe("primaryNetworkOf", () => {
  const known = new Set(["network:bridge", "network:back"]);

  it("prefers the backend's declared primary network", () => {
    const c = node("container:a", "container", { meta: { primaryNetwork: "network:back" } });
    const edges = [edge("container:a", "network:bridge", "network")];
    expect(primaryNetworkOf(c, edges, known)).toBe("network:back");
  });

  it("falls back to the first network edge when the backend declares nothing", () => {
    const c = node("container:a", "container");
    const edges = [edge("container:a", "network:bridge", "network")];
    expect(primaryNetworkOf(c, edges, known)).toBe("network:bridge");
  });

  /** A stale label must not create a group for a network that no longer exists. */
  it("ignores a declared network that is not in the graph", () => {
    const c = node("container:a", "container", { meta: { primaryNetwork: "network:gone" } });
    expect(primaryNetworkOf(c, [], known)).toBeNull();
  });

  it("returns null for a container attached to nothing", () => {
    expect(primaryNetworkOf(node("container:a", "container"), [], known)).toBeNull();
  });
});

describe("groupAssignments", () => {
  it("promotes only networks that own a container", () => {
    const nodes = [
      node("network:bridge", "network"),
      node("network:empty", "network"),
      node("container:a", "container"),
    ];
    const edges = [edge("container:a", "network:bridge", "network")];
    const { groupIds, parentOf } = groupAssignments(nodes, edges);
    expect([...groupIds]).toEqual(["network:bridge"]);
    expect(parentOf.get("container:a")).toBe("network:bridge");
  });
});

describe("visibleEdges", () => {
  it("drops the edge to a container's own group but keeps secondary networks", () => {
    const edges = [
      edge("container:a", "network:bridge", "network"),
      edge("container:a", "network:back", "network"),
      edge("container:a", "volume:data", "volume"),
    ];
    const parentOf = new Map([["container:a", "network:bridge"]]);
    const kept = visibleEdges(edges, parentOf).map((e) => e.to);
    expect(kept).toEqual(["network:back", "volume:data"]);
  });

  it("dedupes repeated pairs", () => {
    const edges = [
      edge("container:a", "volume:data", "volume"),
      edge("container:a", "volume:data", "volume"),
    ];
    expect(visibleEdges(edges, new Map())).toHaveLength(1);
  });
});

describe("buildElkGraph", () => {
  const nodes = [
    node("network:bridge", "network"),
    node("container:a", "container"),
    node("volume:data", "volume"),
  ];
  const edges = [
    edge("container:a", "network:bridge", "network"),
    edge("container:a", "volume:data", "volume"),
  ];

  it("nests containers inside their network and leaves other kinds at root", () => {
    const g = buildElkGraph(nodes, edges);
    const ids = g.children!.map((c) => c.id).sort();
    expect(ids).toEqual(["network:bridge", "volume:data"]);
    const group = g.children!.find((c) => c.id === "network:bridge")!;
    expect(group.children!.map((c) => c.id)).toEqual(["container:a"]);
    // A group is sized by ELK from its contents, never by us.
    expect(group.width).toBeUndefined();
  });

  it("emits only the edges that survive containment", () => {
    const g = buildElkGraph(nodes, edges);
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].targets).toEqual(["volume:data"]);
  });

  /** An edge to a node that vanished between poll and render aborts the ELK run. */
  it("drops edges pointing at absent nodes", () => {
    const g = buildElkGraph([node("container:a", "container")], [
      edge("container:a", "volume:gone", "volume"),
    ]);
    expect(g.edges).toHaveLength(0);
  });

  it("handles an empty graph without throwing", () => {
    const g = buildElkGraph([], []);
    expect(g.children).toEqual([]);
    expect(g.edges).toEqual([]);
  });
});

describe("readElkResult", () => {
  const nodes = [
    node("network:bridge", "network"),
    node("container:a", "container"),
    node("volume:data", "volume"),
  ];
  const edges = [
    edge("container:a", "network:bridge", "network"),
    edge("container:a", "volume:data", "volume"),
  ];

  /** ELK reports child coordinates relative to the parent box. */
  const result: ElkNode = {
    id: "root",
    children: [
      {
        id: "network:bridge",
        x: 10,
        y: 20,
        width: 100,
        height: 100,
        children: [{ id: "container:a", x: 20, y: 34, width: 44, height: 54 }],
      },
      { id: "volume:data", x: 200, y: 40, width: 40, height: 50 },
    ],
  };

  it("converts nested coordinates to absolute centres", () => {
    const layout = readElkResult(result, nodes, edges);
    expect(layout.positions.get("container:a")).toEqual({ x: 10 + 20 + 22, y: 20 + 34 + 27 });
    expect(layout.positions.get("volume:data")).toEqual({ x: 220, y: 65 });
  });

  it("reports the network as a group, not a positioned shape", () => {
    const layout = readElkResult(result, nodes, edges);
    expect(layout.positions.has("network:bridge")).toBe(false);
    expect(layout.groups).toEqual([
      {
        id: "network:bridge",
        label: "bridge",
        status: "running",
        count: 1,
        x: 10,
        y: 20,
        width: 100,
        height: 100,
      },
    ]);
    // Edges into a group land on its top edge rather than its centre, which
    // would otherwise put the line under the contained nodes.
    expect(layout.groupAnchors.get("network:bridge")).toEqual({ x: 60, y: 20 });
  });

  it("bounds cover the furthest box", () => {
    const layout = readElkResult(result, nodes, edges);
    expect(layout.width).toBe(240);
    expect(layout.height).toBe(120);
  });
});

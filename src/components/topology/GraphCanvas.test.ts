import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, waitFor } from "@testing-library/svelte";
import GraphCanvas from "./GraphCanvas.svelte";
import type { TopologyEdge, TopologyNode } from "../../lib/api/topology";

/**
 * These tests exist because the component's reactivity is where this feature
 * broke: the layout effect used to read the very state it wrote, so mounting it
 * looped until Svelte threw `effect_update_depth_exceeded`. Nothing short of an
 * actual mount catches that.
 *
 * Layout is now asynchronous (ELK runs in a Worker, or on the main thread in
 * this environment), so every assertion about geometry has to wait for the run
 * to resolve rather than reading the first paint.
 */

function graph(containerCount: number, status: TopologyNode["status"] = "running") {
  const nodes: TopologyNode[] = [];
  const edges: TopologyEdge[] = [];
  for (let i = 0; i < 10; i++) {
    nodes.push({ id: `network:n${i}`, kind: "network", label: `n${i}`, status: "none", meta: {} });
  }
  for (let i = 0; i < containerCount; i++) {
    const id = `container:c${i}`;
    nodes.push({ id, kind: "container", label: `c${i}`, status, meta: { containerId: `c${i}` } });
    edges.push({ from: id, to: `network:n${i % 10}`, kind: "network" });
  }
  return { nodes, edges };
}

const mount = (nodes: TopologyNode[], edges: TopologyEdge[]) =>
  render(GraphCanvas, { props: { nodes, edges, selectedId: null, onSelect: () => {} } });

afterEach(cleanup);

describe("GraphCanvas", () => {
  it("mounts a 50-container graph without an effect loop", async () => {
    const errors: unknown[] = [];
    const spy = vi.spyOn(console, "error").mockImplementation((...args) => errors.push(args));
    try {
      const { nodes, edges } = graph(50);
      const { container } = mount(nodes, edges);

      // Every network owns containers, so all ten become boxes and the
      // containers are drawn inside them.
      await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(50));
      expect(container.querySelectorAll("g.group")).toHaveLength(10);
      // Each container's only edge points at the network it is already drawn
      // inside, so none of them is worth a line.
      expect(container.querySelectorAll("line.edge")).toHaveLength(0);
      // An effect loop surfaces as a console error, not a thrown exception.
      expect(errors).toHaveLength(0);
    } finally {
      spy.mockRestore();
    }
  });

  it("draws an edge to a container's secondary network", async () => {
    const nodes: TopologyNode[] = [
      { id: "network:front", kind: "network", label: "front", status: "none", meta: {} },
      { id: "network:back", kind: "network", label: "back", status: "none", meta: {} },
      {
        id: "container:a",
        kind: "container",
        label: "a",
        status: "running",
        meta: { primaryNetwork: "network:front" },
      },
      { id: "container:b", kind: "container", label: "b", status: "running", meta: {} },
    ];
    const edges: TopologyEdge[] = [
      { from: "container:a", to: "network:front", kind: "network" },
      { from: "container:a", to: "network:back", kind: "network" },
      { from: "container:b", to: "network:back", kind: "network" },
    ];
    const { container } = mount(nodes, edges);

    await waitFor(() => expect(container.querySelectorAll("g.group")).toHaveLength(2));
    // Only a→back survives: a is inside front, b is inside back.
    expect(container.querySelectorAll("line.edge")).toHaveLength(1);
  });

  /**
   * Every kind must stamp *its own* icon. The graph used to draw abstract
   * shapes, which meant network and image were both circles and told apart only
   * by radius — indistinguishable at the zoom a 50-node graph opens at.
   */
  it("gives every node kind its own icon", async () => {
    const nodes: TopologyNode[] = [
      { id: "container:a", kind: "container", label: "a", status: "running", meta: {} },
      { id: "network:b", kind: "network", label: "b", status: "none", meta: {} },
      { id: "volume:c", kind: "volume", label: "c", status: "none", meta: {} },
      { id: "project:d", kind: "project", label: "d", status: "stopped", meta: {} },
      { id: "image:e", kind: "image", label: "e", status: "none", meta: {} },
    ];
    const { container } = mount(nodes, []);

    // A network with no containers stays a chip: an empty box would cost far
    // more space for the same information.
    await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(5));
    expect(container.querySelectorAll("g.group")).toHaveLength(0);

    for (const kind of ["container", "network", "volume", "project", "image"]) {
      const icon = container.querySelector(`g.kind-${kind} use.chip-icon`);
      expect(icon?.getAttribute("href")).toBe(`#topo-icon-${kind}`);
    }
    // One symbol definition per kind, not one per node. Six kinds: the five
    // above plus `service`, which no node in this graph uses.
    expect(container.querySelectorAll("defs symbol")).toHaveLength(6);
  });

  /**
   * Status has to survive greyscale: colour alone hides running-vs-unhealthy
   * from anyone with red-green colour blindness.
   */
  it("encodes status as shape, not only colour", async () => {
    const nodes: TopologyNode[] = [
      { id: "container:a", kind: "container", label: "a", status: "running", meta: {} },
      { id: "container:b", kind: "container", label: "b", status: "stopped", meta: {} },
      { id: "container:c", kind: "container", label: "c", status: "unhealthy", meta: {} },
      { id: "image:d", kind: "image", label: "d", status: "none", meta: {} },
    ];
    const { container } = mount(nodes, []);

    await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(4));
    expect(container.querySelectorAll("circle.marker-running")).toHaveLength(1);
    expect(container.querySelectorAll("circle.marker-stopped")).toHaveLength(1);
    expect(container.querySelectorAll("path.marker-unhealthy")).toHaveLength(1);
    // A node with no lifecycle gets no marker at all rather than a misleading one.
    expect(container.querySelectorAll("g.status-none .marker-stopped")).toHaveLength(0);
  });

  /**
   * A compose service that was never created must be visibly different from a
   * container that exists but is stopped: one needs `compose up`, the other
   * `start`, so collapsing them sends the user to the wrong command.
   */
  it("draws a not-created service as an outline with a depends-on edge", async () => {
    const nodes: TopologyNode[] = [
      {
        id: "service:shop/api",
        kind: "service",
        label: "api",
        status: "notCreated",
        meta: { project: "shop" },
      },
      {
        id: "service:shop/db",
        kind: "service",
        label: "db",
        status: "notCreated",
        meta: { project: "shop" },
      },
    ];
    const edges: TopologyEdge[] = [
      { from: "service:shop/api", to: "service:shop/db", kind: "dependsOn" },
    ];
    const { container } = mount(nodes, edges);

    await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(2));
    expect(container.querySelectorAll("g.status-notCreated")).toHaveLength(2);
    expect(container.querySelectorAll("circle.marker-not-created")).toHaveLength(2);
    // Never a stopped marker: that would claim the service exists.
    expect(container.querySelectorAll("circle.marker-stopped")).toHaveLength(0);
    expect(container.querySelectorAll("line.edge-dependsOn")).toHaveLength(1);
  });

  it("offers zoom and fit controls rather than relying on the wheel alone", async () => {
    const { nodes, edges } = graph(3);
    const { container } = mount(nodes, edges);
    // 3 containers plus the 7 networks left without one.
    await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(10));
    expect(container.querySelectorAll(".canvas-controls .canvas-btn")).toHaveLength(4);
  });

  it("keeps positions when only a container's status changes", async () => {
    // A status poll must not re-lay-out the graph: the user would see it jump
    // under the cursor every few seconds.
    const { nodes, edges } = graph(20, "running");
    const { container, rerender } = render(GraphCanvas, {
      props: { nodes, edges, selectedId: null, onSelect: () => {} },
    });
    await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(20));
    const before = [...container.querySelectorAll("g.node")].map((g) =>
      g.getAttribute("transform")
    );

    const flipped = nodes.map((n) =>
      n.kind === "container" ? { ...n, status: "unhealthy" as const } : n
    );
    await rerender({ nodes: flipped, edges, selectedId: null, onSelect: () => {} });

    const after = [...container.querySelectorAll("g.node")].map((g) => g.getAttribute("transform"));
    expect(after).toEqual(before);
  });

  /** ELK's whole point over the old force layout: same graph, same picture. */
  it("lays the same graph out identically on a second mount", async () => {
    const { nodes, edges } = graph(12);
    const first = mount(nodes, edges);
    await waitFor(() => expect(first.container.querySelectorAll("g.node")).toHaveLength(12));
    const a = [...first.container.querySelectorAll("g.node")].map((g) =>
      g.getAttribute("transform")
    );
    cleanup();

    const second = mount(nodes, edges);
    await waitFor(() => expect(second.container.querySelectorAll("g.node")).toHaveLength(12));
    const b = [...second.container.querySelectorAll("g.node")].map((g) =>
      g.getAttribute("transform")
    );
    expect(b).toEqual(a);
  });

  it("survives a shape change and renders the new node count", async () => {
    const first = graph(5);
    const { container, rerender } = render(GraphCanvas, {
      props: { nodes: first.nodes, edges: first.edges, selectedId: null, onSelect: () => {} },
    });
    // Five networks own a container and become boxes; five stay shapes.
    await waitFor(() => expect(container.querySelectorAll("g.group")).toHaveLength(5));

    const second = graph(7);
    await rerender({
      nodes: second.nodes,
      edges: second.edges,
      selectedId: null,
      onSelect: () => {},
    });
    // Waiting on the group count, not the node count: the latter is 10 both
    // before and after, so it would pass against the stale layout.
    await waitFor(() => expect(container.querySelectorAll("g.group")).toHaveLength(7));
    expect(container.querySelectorAll("g.node")).toHaveLength(10);
  });

  it("renders an empty graph without crashing", async () => {
    const { container } = mount([], []);
    await waitFor(() => expect(container.querySelectorAll("g.node")).toHaveLength(0));
  });

  it("drops duplicate edges rather than failing the keyed loop", async () => {
    // Defence in depth: the backend dedupes, but a duplicated pair reaching the
    // renderer must not take the page down.
    const nodes: TopologyNode[] = [
      { id: "container:a", kind: "container", label: "a", status: "running", meta: {} },
      { id: "volume:v", kind: "volume", label: "v", status: "none", meta: {} },
    ];
    const edges: TopologyEdge[] = [
      { from: "container:a", to: "volume:v", kind: "volume" },
      { from: "container:a", to: "volume:v", kind: "volume" },
    ];
    const { container } = mount(nodes, edges);
    await waitFor(() => expect(container.querySelectorAll("line.edge")).toHaveLength(1));
  });
});

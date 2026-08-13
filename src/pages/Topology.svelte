<script lang="ts">
  /**
   * Docker topology page: containers and what they attach to, as a graph.
   *
   * Deliberately distinct from the Kubernetes graph on the Kubernetes page —
   * that one draws cluster resources, this one draws the Docker engine.
   */
  import { onMount } from "svelte";
  import { topologyApi, type TopologyGraph, type TopologyNodeKind } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { consumeFocus } from "../lib/topology-link";
  import { t } from "../lib/i18n.svelte";
  import * as Icons from "../components/Icons.svelte";
  import GraphCanvas from "../components/topology/GraphCanvas.svelte";
  import NodeDetail from "../components/topology/NodeDetail.svelte";

  // Canonical kind order, shared with the graph's symbol order and the detail
  // panel's related list: container first, its attachments next, and the
  // not-yet-created compose service last as the container-shaped afterthought.
  const KINDS: TopologyNodeKind[] = [
    "container",
    "network",
    "volume",
    "project",
    "image",
    "service",
  ];

  /** Same glyph per kind as the graph draws, so the rail doubles as the legend. */
  const KIND_ICON: Record<TopologyNodeKind, string> = {
    container: Icons.Container,
    service: Icons.Container,
    network: Icons.Network,
    volume: Icons.Volume,
    project: Icons.Compose,
    image: Icons.Image,
  };

  let graph = $state<TopologyGraph>({ nodes: [], edges: [], warnings: [] });
  let loading = $state(true);
  let searchTerm = $state("");
  let enabledKinds = $state<Set<TopologyNodeKind>>(new Set(KINDS));
  let selectedId = $state<string | null>(null);
  // No handle on the canvas component: it owns its own viewport controls now
  // (zoom, fit and reset live in the floating cluster on the canvas itself).

  async function refresh() {
    loading = true;
    try {
      graph = await topologyApi.getTopology();
      // A subsystem that failed to list would otherwise read as "this container
      // attaches to nothing", which is a wrong answer rather than a missing one.
      for (const w of graph.warnings) {
        globalToast("error", `${t("topology.partial", { default: "Partial graph" })}: ${w}`);
      }
    } catch (e) {
      graph = { nodes: [], edges: [], warnings: [] };
      globalToast("error", `${t("topology.load_failed", { default: "Could not load topology" })}: ${e}`);
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    await refresh();
    // Arriving from another page's "View in topology". A node that no longer
    // exists is ignored rather than leaving a selection pointing at nothing.
    const focus = consumeFocus("topology");
    if (focus && graph.nodes.some((n) => n.id === focus)) selectedId = focus;
  });

  function toggleKind(kind: TopologyNodeKind) {
    const next = new Set(enabledKinds);
    if (next.has(kind)) next.delete(kind);
    else next.add(kind);
    enabledKinds = next;
  }

  /**
   * Search narrows to matching nodes *plus their neighbours*, so a hit never
   * appears as an orphan dot with its context filtered away.
   */
  const visibleNodes = $derived.by(() => {
    const byKind = graph.nodes.filter((n) => enabledKinds.has(n.kind));
    const term = searchTerm.trim().toLowerCase();
    if (!term) return byKind;

    const allowed = new Set(byKind.map((n) => n.id));
    const matched = new Set(
      byKind.filter((n) => n.label.toLowerCase().includes(term)).map((n) => n.id)
    );
    const keep = new Set(matched);
    for (const e of graph.edges) {
      if (matched.has(e.from) && allowed.has(e.to)) keep.add(e.to);
      if (matched.has(e.to) && allowed.has(e.from)) keep.add(e.from);
    }
    return byKind.filter((n) => keep.has(n.id));
  });

  const visibleEdges = $derived.by(() => {
    const ids = new Set(visibleNodes.map((n) => n.id));
    return graph.edges.filter((e) => ids.has(e.from) && ids.has(e.to));
  });

  const selectedNode = $derived(graph.nodes.find((n) => n.id === selectedId) ?? null);

  const counts = $derived.by(() => {
    const map = new Map<TopologyNodeKind, number>();
    for (const n of graph.nodes) map.set(n.kind, (map.get(n.kind) ?? 0) + 1);
    return map;
  });

  // $derived, not const: `t()` is only reactive when read inside a rune, so a
  // plain object would freeze these labels in whatever language was active at
  // mount and never follow a language switch.
  const kindLabels = $derived<Record<TopologyNodeKind, string>>({
    container: t("topology.kind_container", { default: "Containers" }),
    // A kind, not a status: the chip counts compose services (declared but not
    // created). Their "does not exist yet" state is the legend's job.
    service: t("topology.kind_service", { default: "Services" }),
    network: t("topology.kind_network", { default: "Networks" }),
    volume: t("topology.kind_volume", { default: "Volumes" }),
    project: t("topology.kind_project", { default: "Projects" }),
    image: t("topology.kind_image", { default: "Images" }),
  });
</script>

<div class="content-header" data-tauri-drag-region>
  <h1>
    {t("topology.title", { default: "Topology" })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {graph.nodes.length} {t("topology.nodes", { default: "nodes" })} ·
      {graph.edges.length} {t("topology.links", { default: "links" })}
    </span>
  </h1>
  <div class="content-header-actions">
    <input
      class="search-input"
      type="search"
      placeholder={t("topology.search", { default: "Search by name" })}
      bind:value={searchTerm}
    />
    <button class="btn btn-ghost btn-icon" onclick={refresh} aria-label={t("common.refresh", { default: "Refresh" })}>
      {@html Icons.Refresh}
    </button>
  </div>
</div>

<div class="content-body topology-body">
  <!-- One rail doing two jobs. Each chip carries the exact icon and colour the
       graph uses for that kind, so the filter *is* the legend — there is no
       second thing to keep in sync, and no band of checkboxes that explains
       nothing. -->
  <div class="rail">
    <div class="kind-chips">
      {#each KINDS as kind (kind)}
        <button
          type="button"
          class="kind-chip kind-{kind}"
          class:off={!enabledKinds.has(kind)}
          aria-pressed={enabledKinds.has(kind)}
          onclick={() => toggleKind(kind)}
        >
          <span class="chip-icon">{@html KIND_ICON[kind]}</span>
          <span>{kindLabels[kind]}</span>
          <span class="count">{counts.get(kind) ?? 0}</span>
        </button>
      {/each}
    </div>

    <div class="legend">
      <span class="legend-item"><span class="mark run"></span>{t("topology.status_running", { default: "Running" })}</span>
      <span class="legend-item"><span class="mark stop"></span>{t("topology.status_stopped", { default: "Stopped" })}</span>
      <span class="legend-item"><span class="mark bad"></span>{t("topology.status_unhealthy", { default: "Unhealthy" })}</span>
      <span class="legend-item"><span class="mark none"></span>{t("topology.status_not_created", { default: "Not created" })}</span>
    </div>
  </div>

  <div class="graph-area">
    {#if loading}
      <div class="placeholder">{t("common.loading", { default: "Loading…" })}</div>
    {:else if graph.nodes.length === 0}
      <div class="placeholder">
        {t("topology.empty", { default: "No Docker resources found for the current engine." })}
      </div>
    {:else}
      <div class="canvas-wrap">
        <GraphCanvas
          nodes={visibleNodes}
          edges={visibleEdges}
          {selectedId}
          onSelect={(id) => (selectedId = id)}
        />
      </div>
      <NodeDetail
        node={selectedNode}
        {graph}
        onClose={() => (selectedId = null)}
        onChanged={refresh}
        onSelect={(id) => (selectedId = id)}
      />
    {/if}
  </div>
</div>

<style>
  .topology-body {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding: 0;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .rail {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
    padding: 8px 20px;
    border-bottom: 1px solid var(--border-primary);
  }

  .kind-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .kind-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border: 1px solid var(--border-primary);
    border-radius: 14px;
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    cursor: pointer;
  }

  /* Off is dimmed rather than hidden: a filtered-out kind still has to show its
     count, otherwise "0 volumes" and "volumes turned off" look identical. */
  .kind-chip.off {
    opacity: 0.4;
  }

  .kind-chip :global(svg) {
    width: 14px;
    height: 14px;
  }

  .kind-container .chip-icon {
    color: var(--accent-blue);
  }
  .kind-network .chip-icon {
    color: var(--color-info, #38bdf8);
  }
  .kind-volume .chip-icon {
    color: var(--color-warning, #f59e0b);
  }
  .kind-project .chip-icon {
    color: var(--accent-purple);
  }
  .kind-image .chip-icon,
  .kind-service .chip-icon {
    color: var(--text-muted);
  }

  .chip-icon {
    display: flex;
  }

  .count {
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border-radius: 10px;
    padding: 0 6px;
  }

  .legend {
    display: flex;
    gap: 14px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  /* The legend marks mirror the graph's shapes exactly — filled dot, hollow
     dot, triangle — so status stays readable without colour. */
  .mark {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .mark.run {
    background: var(--color-success, #22c55e);
  }

  .mark.stop {
    border: 1.5px solid var(--text-muted);
  }

  /* Dashed, matching the graph: "does not exist" must not read as "stopped" —
     the fix for each is a different command. */
  .mark.none {
    border: 1.5px dashed var(--text-muted);
  }

  .mark.bad {
    width: 0;
    height: 0;
    border-radius: 0;
    border-left: 4.5px solid transparent;
    border-right: 4.5px solid transparent;
    border-bottom: 8px solid var(--color-danger, #ef4444);
  }

  .graph-area {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .canvas-wrap {
    flex: 1;
    min-width: 0;
    position: relative;
  }

  .placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }

  .search-input {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: var(--text-xs);
    padding: 5px 8px;
    width: 180px;
  }
</style>

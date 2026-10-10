<script lang="ts">
  /**
   * Right-hand detail panel for a selected topology node.
   *
   * Container actions call the same `dockerApi` methods the Containers page
   * uses — there is no second code path for start/stop here.
   */
  import { dockerApi } from "../../lib/api";
  import { globalToast } from "../../lib/globalToast";
  import { t } from "../../lib/i18n.svelte";
  import * as Icons from "../Icons.svelte";
  import { uiState } from "../../store.svelte";
  import type { TopologyNode, TopologyGraph } from "../../lib/api/topology";

  interface Props {
    node: TopologyNode | null;
    /** Whole graph, so related resources can be listed and jumped to. */
    graph: TopologyGraph;
    onClose: () => void;
    onChanged: () => void;
    onSelect: (id: string) => void;
  }

  let { node, graph, onClose, onChanged, onSelect }: Props = $props();

  /**
   * Which page owns each kind, for the "open in" jump. Kinds absent from this
   * map simply get no button rather than a dead one.
   */
  const OWNING_PAGE: Record<string, string> = {
    container: "containers",
    network: "networks",
    volume: "volumes",
    image: "images",
    project: "compose",
    // A service that does not exist yet is only meaningful in its project.
    service: "compose",
  };

  const KIND_ICON: Record<string, string> = {
    container: Icons.Container,
    service: Icons.Container,
    network: Icons.Network,
    volume: Icons.Volume,
    project: Icons.Compose,
    image: Icons.Image,
  };

  /**
   * Everything the selected node touches, grouped by kind.
   *
   * Derived from the edges already on screen — no extra call. Without this the
   * graph is a dead end: seeing that `api` mounts `pgdata` still left you to go
   * find `pgdata` by hand.
   */
  const related = $derived.by(() => {
    if (!node) return [] as Array<{ kind: string; nodes: TopologyNode[] }>;
    const byId = new Map(graph.nodes.map((n) => [n.id, n]));
    const neighbourIds = new Set<string>();
    for (const e of graph.edges) {
      if (e.from === node.id) neighbourIds.add(e.to);
      else if (e.to === node.id) neighbourIds.add(e.from);
    }
    const groups = new Map<string, TopologyNode[]>();
    for (const id of neighbourIds) {
      const n = byId.get(id);
      if (!n) continue;
      if (!groups.has(n.kind)) groups.set(n.kind, []);
      groups.get(n.kind)!.push(n);
    }
    // Stable kind order, and names sorted inside each — a list that reshuffles
    // between polls is unusable as a jump target.
    // Canonical kind order — same sequence as the page rail and the graph's
    // symbol order, so the panel never presents a different hierarchy from the
    // legend next to it.
    return ["container", "network", "volume", "project", "image", "service"]
      .filter((k) => groups.has(k))
      .map((kind) => ({
        kind,
        nodes: groups.get(kind)!.sort((a, b) => a.label.localeCompare(b.label)),
      }));
  });

  /**
   * Jump to the page that owns this kind, asking it to preselect the resource.
   *
   * `focusResource` must be set *before* `currentPage`, since the destination
   * reads it as it mounts.
   */
  function openInOwningPage() {
    if (!node) return;
    const page = OWNING_PAGE[node.kind];
    if (!page) return;
    // List pages key off the resource's own identity, not the prefixed graph id.
    // A service has no page of its own — the Compose page keys on its project.
    const id =
      node.kind === "container"
        ? String(node.meta?.containerId ?? node.label)
        : node.kind === "service"
          ? String(node.meta?.project ?? node.label)
          : node.label;
    uiState.focusResource = { page, id };
    uiState.currentPage = page;
  }

  let busy = $state<string | null>(null);
  let logs = $state<string>("");

  const containerId = $derived(
    node?.kind === "container" ? String(node.meta?.containerId ?? "") : ""
  );
  const isRunning = $derived(node?.status === "running" || node?.status === "unhealthy");

  async function runAction(action: "start" | "stop" | "restart") {
    if (!containerId) return;
    busy = action;
    try {
      if (action === "start") await dockerApi.startContainer(containerId);
      else if (action === "stop") await dockerApi.stopContainer(containerId);
      else await dockerApi.restartContainer(containerId);
      globalToast("success", t("topology.action_done", { default: "Action completed" }));
      onChanged();
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      busy = null;
    }
  }

  async function loadLogs() {
    if (!containerId) return;
    busy = "logs";
    try {
      logs = await dockerApi.containerLogs(containerId, 200);
    } catch (e) {
      globalToast("error", String(e));
    } finally {
      busy = null;
    }
  }

  // Stale logs from a previously selected container would be actively
  // misleading, so clear them whenever the selection moves.
  $effect(() => {
    void node?.id;
    logs = "";
  });

  // $derived so a language switch updates these; see the same note in
  // pages/Topology.svelte.
  const kindLabel = $derived<Record<string, string>>({
    container: t("topology.node_container", { default: "Container" }),
    service: t("topology.node_service", { default: "Compose service" }),
    network: t("topology.node_network", { default: "Network" }),
    volume: t("topology.node_volume", { default: "Volume" }),
    project: t("topology.node_project", { default: "Compose project" }),
    image: t("topology.node_image", { default: "Image" }),
  });

  /** Destination page names, as they read in the sidebar. */
  const kindPageLabel = $derived<Record<string, string>>({
    container: t("sidebar.containers", { default: "Containers" }),
    network: t("sidebar.networks", { default: "Networks" }),
    volume: t("sidebar.volumes", { default: "Volumes" }),
    image: t("sidebar.images", { default: "Images" }),
    project: t("sidebar.compose", { default: "Compose" }),
    service: t("sidebar.compose", { default: "Compose" }),
  });

  const statusLabel = $derived<Record<string, string>>({
    running: t("topology.status_running", { default: "Running" }),
    stopped: t("topology.status_stopped", { default: "Stopped" }),
    unhealthy: t("topology.status_unhealthy", { default: "Unhealthy" }),
    notCreated: t("topology.status_not_created", { default: "Not created" }),
    none: t("topology.status_none", { default: "—" }),
  });

  const metaEntries = $derived(
    Object.entries(node?.meta ?? {}).filter(([, v]) => v !== "" && v != null)
  );
</script>

{#if node}
  <aside class="node-detail">
    <header>
      <div>
        <span class="kind">{kindLabel[node.kind] ?? node.kind}</span>
        <h2>{node.label}</h2>
      </div>
      <button
        class="btn btn-ghost btn-icon"
        onclick={onClose}
        aria-label={t("common.close", { default: "Close" })}
      >
        {@html Icons.Close}
      </button>
    </header>

    <div class="status-row">
      <span class="status-dot status-{node.status}"></span>
      <span>{statusLabel[node.status] ?? node.status}</span>
    </div>

    {#if node.kind === "container"}
      <div class="actions">
        {#if isRunning}
          <button class="btn btn-ghost" disabled={busy !== null} onclick={() => runAction("stop")}>
            {t("topology.stop", { default: "Stop" })}
          </button>
          <button class="btn btn-ghost" disabled={busy !== null} onclick={() => runAction("restart")}>
            {t("topology.restart", { default: "Restart" })}
          </button>
        {:else}
          <button class="btn btn-primary" disabled={busy !== null} onclick={() => runAction("start")}>
            {t("topology.start", { default: "Start" })}
          </button>
        {/if}
        <button class="btn btn-ghost" disabled={busy !== null} onclick={loadLogs}>
          {t("topology.logs", { default: "Logs" })}
        </button>
      </div>
    {/if}

    {#if OWNING_PAGE[node.kind]}
      <button class="btn btn-ghost open-in" onclick={openInOwningPage}>
        {t("topology.open_in", { default: "Open in" })}
        {kindPageLabel[node.kind] ?? node.kind}
      </button>
    {/if}

    <!-- Cross-references. Clicking one moves the selection inside the graph
         rather than navigating away: the point is to walk the topology, and
         losing the graph on every hop would defeat that. -->
    {#if related.length > 0}
      <section class="related">
        <h3>{t("topology.related", { default: "Related" })}</h3>
        {#each related as group (group.kind)}
          <div class="related-group">
            <span class="related-kind">{kindLabel[group.kind] ?? group.kind}</span>
            <div class="related-items">
              {#each group.nodes as item (item.id)}
                <button class="related-item kind-{item.kind}" onclick={() => onSelect(item.id)}>
                  <span class="related-icon">{@html KIND_ICON[item.kind] ?? ""}</span>
                  <span class="related-label">{item.label}</span>
                </button>
              {/each}
            </div>
          </div>
        {/each}
      </section>
    {/if}

    {#if metaEntries.length > 0}
      <dl class="meta">
        {#each metaEntries as [key, value] (key)}
          <dt>{key}</dt>
          <dd>{String(value)}</dd>
        {/each}
      </dl>
    {/if}

    {#if logs}
      <pre class="logs">{logs}</pre>
    {/if}
  </aside>
{/if}

<style>
  .node-detail {
    width: 320px;
    flex-shrink: 0;
    border-left: 1px solid var(--border-primary);
    background: var(--bg-secondary);
    padding: 16px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }

  /* The panel scrolls; it does not squash what it holds.
     Its children sit on a column flex container's main axis, so they shrink
     by default once the content is taller than the panel — which flattened the
     bordered button below the height of its own text. `overflow-y: auto` is
     the statement of intent: overflow becomes scroll, never compression.
     Buttons inside `.actions` never showed it because that row is a nested
     flex container, so their heights are on a different axis. */
  .node-detail > * {
    flex-shrink: 0;
  }

  .open-in {
    align-self: flex-start;
  }

  .related h3 {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin: 0 0 8px;
  }

  .related-group {
    display: flex;
    gap: 8px;
    align-items: baseline;
    margin-bottom: 8px;
  }

  .related-kind {
    flex: 0 0 74px;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .related-items {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-width: 0;
  }

  .related-item {
    display: flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    padding: 3px 8px;
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    cursor: pointer;
  }

  .related-item:hover {
    border-color: var(--accent-blue);
    color: var(--text-primary);
  }

  .related-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .related-icon {
    display: flex;
    flex-shrink: 0;
  }

  .related-item :global(svg) {
    width: 12px;
    height: 12px;
  }

  /* Same kind-to-colour mapping as the graph and the filter rail. */
  .kind-container .related-icon {
    color: var(--accent-blue);
  }
  .kind-network .related-icon {
    color: var(--color-info, #38bdf8);
  }
  .kind-volume .related-icon {
    color: var(--color-warning, #f59e0b);
  }
  .kind-project .related-icon {
    color: var(--accent-purple);
  }
  .kind-image .related-icon,
  .kind-service .related-icon {
    color: var(--text-muted);
  }

  .kind {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  h2 {
    font-size: var(--text-sm);
    font-weight: 600;
    word-break: break-all;
    margin: 2px 0 0;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-muted);
  }

  .status-running {
    background: var(--color-success, #22c55e);
  }

  .status-unhealthy {
    background: var(--color-danger, #ef4444);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .meta {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 6px;
    font-size: var(--text-xs);
    margin: 0;
  }

  .meta dt {
    color: var(--text-muted);
  }

  .meta dd {
    margin: 0 0 4px;
    color: var(--text-secondary);
    word-break: break-all;
  }

  .logs {
    background: var(--bg-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 8px;
    font-size: 11px;
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>

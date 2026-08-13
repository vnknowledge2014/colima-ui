<script lang="ts">
  /**
   * The live table. Sorting and formatting only — sampling, history and gap
   * handling all belong to the page and `lib/metricsHistory`.
   */
  import Sparkline from "./Sparkline.svelte";
  import { t } from "../../lib/i18n.svelte";
  import type { MetricSample } from "../../lib/api/metrics";
  import type { MetricsHistory, SortKey } from "../../lib/metricsHistory";

  interface Props {
    rows: MetricSample[];
    history: MetricsHistory;
    /** Bumped by the page on every batch so sparklines re-read mutated history. */
    revision: number;
    sortKey: SortKey;
    sortAsc: boolean;
    onSort: (key: SortKey) => void;
    onSelect: (sample: MetricSample) => void;
    selectedId: string | null;
  }

  let { rows, history, revision, sortKey, sortAsc, onSort, onSelect, selectedId }: Props = $props();

  const columns: Array<{ key: SortKey; label: string; numeric: boolean }> = $derived([
    { key: "name", label: t("activity.col_name", { default: "Container" }), numeric: false },
    { key: "cpuPct", label: t("activity.col_cpu", { default: "CPU %" }), numeric: true },
    { key: "memBytes", label: t("activity.col_mem", { default: "Memory" }), numeric: true },
    { key: "memPct", label: t("activity.col_mem_pct", { default: "Mem %" }), numeric: true },
    { key: "netRxBytes", label: t("activity.col_net", { default: "Net I/O" }), numeric: true },
    { key: "blockReadBytes", label: t("activity.col_block", { default: "Block I/O" }), numeric: true },
    { key: "pids", label: t("activity.col_pids", { default: "PIDs" }), numeric: true },
  ]);

  const sorted = $derived.by(() => {
    const copy = [...rows];
    copy.sort((a, b) => {
      const av = a[sortKey];
      const bv = b[sortKey];
      const cmp =
        typeof av === "string" && typeof bv === "string"
          ? av.localeCompare(bv)
          : Number(av) - Number(bv);
      return sortAsc ? cmp : -cmp;
    });
    return copy;
  });

  function bytes(n: number): string {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = n;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit++;
    }
    return unit === 0 ? `${n} B` : `${value.toFixed(1)} ${units[unit]}`;
  }
</script>

<table class="metrics">
  <thead>
    <tr>
      {#each columns as col (col.key)}
        <th class:numeric={col.numeric}>
          <button type="button" onclick={() => onSort(col.key)}>
            {col.label}
            {#if sortKey === col.key}<span class="caret">{sortAsc ? "▲" : "▼"}</span>{/if}
          </button>
        </th>
      {/each}
      <th>{t("activity.col_trend", { default: "CPU trend" })}</th>
    </tr>
  </thead>
  <tbody>
    {#each sorted as row (row.containerId)}
      <tr
        class:selected={selectedId === row.containerId}
        class:stale={history.isStale(row.containerId)}
        onclick={() => onSelect(row)}
      >
        <td class="name" title={row.name}>{row.name}</td>
        <td class="numeric">{row.cpuPct.toFixed(2)}</td>
        <td class="numeric">{bytes(row.memBytes)} / {bytes(row.memLimitBytes)}</td>
        <td class="numeric">{row.memPct.toFixed(1)}</td>
        <td class="numeric">{bytes(row.netRxBytes)} / {bytes(row.netTxBytes)}</td>
        <td class="numeric">{bytes(row.blockReadBytes)} / {bytes(row.blockWriteBytes)}</td>
        <td class="numeric">{row.pids}</td>
        <td>
          {#key revision}
            <!-- CPU can exceed 100% on multi-core, so the sparkline scales to the
                 data rather than being pinned to a ceiling it would clip at. -->
            <Sparkline values={history.seriesFor(row.containerId, "cpuPct")} />
          {/key}
        </td>
      </tr>
    {/each}
  </tbody>
</table>

{#if sorted.length === 0}
  <p class="empty">{t("activity.no_containers", { default: "No running containers." })}</p>
{/if}

<style>
  .metrics {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-xs);
  }

  th {
    text-align: left;
    color: var(--text-muted);
    font-weight: 500;
    border-bottom: 1px solid var(--border-primary);
    position: sticky;
    top: 0;
    background: var(--bg-primary);
    z-index: 1;
  }

  th button {
    all: unset;
    cursor: pointer;
    padding: 8px 10px;
    display: block;
    width: 100%;
    box-sizing: border-box;
  }

  th.numeric button {
    text-align: right;
  }

  .caret {
    margin-left: 4px;
  }

  td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-primary);
    color: var(--text-secondary);
    white-space: nowrap;
  }

  td.numeric {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  td.name {
    color: var(--text-primary);
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  tbody tr {
    cursor: pointer;
  }

  tbody tr:hover {
    background: var(--bg-secondary);
  }

  tbody tr.selected {
    background: var(--bg-card-hover, var(--bg-secondary));
  }

  /* Dimmed rather than hidden: the container is still there, we just have no
     current reading for it. */
  tbody tr.stale td {
    opacity: 0.45;
  }

  .empty {
    color: var(--text-muted);
    font-size: var(--text-xs);
    padding: 16px 10px;
  }
</style>

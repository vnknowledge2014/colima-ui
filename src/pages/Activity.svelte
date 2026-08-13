<script lang="ts">
  /**
   * Live resource monitor.
   *
   * Sampling happens in one place on the backend and only while this page holds
   * its subscription — closing the page drops the SSE stream, the subscriber
   * count falls to zero and the collector stops calling Docker. That is the whole
   * lifecycle; there is no stop button to forget to press.
   */
  import { onDestroy, onMount } from "svelte";
  import {
    subscribeMetrics,
    metricsApi,
    alertsApi,
    type AlertEvent,
    type AlertRule,
    type HistorySeries,
    type MetricSample,
    type MetricsBatch,
    type WriterHealth,
  } from "../lib/api/metrics";
  import ProGate from "../components/ProGate.svelte";
  import HistoryChart from "../components/activity/HistoryChart.svelte";
  import AlertRuleEditor from "../components/activity/AlertRuleEditor.svelte";
  import AlertLog from "../components/activity/AlertLog.svelte";
  import { MetricsHistory, type SortKey } from "../lib/metricsHistory";
  import { getAppSetting, setAppSetting } from "../lib/settingsStore.svelte";
  import { systemApi, type EngineResources } from "../lib/api";
  import { globalToast } from "../lib/globalToast";
  import { t } from "../lib/i18n.svelte";
  import * as Icons from "../components/Icons.svelte";
  import ResourceTable from "../components/activity/ResourceTable.svelte";
  import ActionFeed from "../components/activity/ActionFeed.svelte";
  import HealSuggestionBanner from "../components/activity/HealSuggestionBanner.svelte";
  import { openSettingsSection } from "../store.svelte";

  const INTERVAL_SETTING = "colimaui_metrics_interval_ms";
  const INTERVALS = [1000, 2000, 5000];

  const history = new MetricsHistory();

  let rows = $state<MetricSample[]>([]);
  /** Bumped on every batch: history is a plain object, not reactive state. */
  let revision = $state(0);
  let connected = $state(false);
  let lastError = $state("");
  let droppedTotal = $state(0);
  let engine = $state<EngineResources | null>(null);

  let searchTerm = $state("");
  let sortKey = $state<SortKey>("cpuPct");
  let sortAsc = $state(false);

  let selected = $state<MetricSample | null>(null);
  let processes = $state("");
  let processesLoading = $state(false);

  let intervalMs = $state(Number(getAppSetting(INTERVAL_SETTING)) || 2000);

  const unsubscribe = subscribeMetrics({
    onBatch: (batch: MetricsBatch) => {
      connected = true;
      if (batch.error) {
        lastError = batch.error;
        return;
      }
      lastError = "";
      history.push(batch.samples ?? []);
      rows = history.current();
      revision++;
    },
    onLagged: (dropped) => {
      // Do not interpolate: record the hole so charts break their line and the
      // affected rows dim, rather than implying steady load we never observed.
      droppedTotal += dropped;
      history.markGap();
      rows = history.current();
      revision++;
    },
    onError: () => {
      connected = false;
    },
  });

  // Once, not per sample: core count, total memory and versions do not change
  // while the page is open, and obtaining them costs a `docker stats` call.
  onMount(async () => {
    try {
      engine = await systemApi.engineResources();
    } catch {
      // The header simply omits the engine line; the table does not depend on it.
    }
  });

  onDestroy(() => {
    unsubscribe();
    history.clear();
  });

  async function changeInterval(ms: number) {
    try {
      // The backend clamps, and returns what it actually applied.
      intervalMs = await metricsApi.setInterval(ms);
      setAppSetting(INTERVAL_SETTING, String(intervalMs));
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      sortAsc = !sortAsc;
    } else {
      sortKey = key;
      // Names read best A→Z; every numeric column is most useful biggest-first.
      sortAsc = key === "name";
    }
  }

  async function showProcesses(sample: MetricSample) {
    selected = sample;
    processes = "";
    processesLoading = true;
    try {
      // On demand only. In the tick loop this would be one extra `docker top`
      // per container per period, for a panel that is usually closed.
      processes = await metricsApi.containerTop(sample.containerId);
    } catch (e) {
      processes = String(e);
    } finally {
      processesLoading = false;
    }
  }

  const visibleRows = $derived.by(() => {
    const term = searchTerm.trim().toLowerCase();
    if (!term) return rows;
    return rows.filter((r) => r.name.toLowerCase().includes(term));
  });

  const totals = $derived.by(() => {
    let cpu = 0;
    let mem = 0;
    for (const r of rows) {
      cpu += r.cpuPct;
      mem += r.memBytes;
    }
    return { cpu, mem };
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

  // ===== History and alerts (Pro) =====

  type Tab = "live" | "history" | "alerts" | "actions";
  let tab = $state<Tab>("live");

  /** Ranges offered, and which stored table each one reads. */
  const RANGES = [
    { key: "1h", ms: 60 * 60 * 1000 },
    { key: "24h", ms: 24 * 60 * 60 * 1000 },
    { key: "7d", ms: 7 * 24 * 60 * 60 * 1000 },
    { key: "30d", ms: 30 * 24 * 60 * 60 * 1000 },
  ] as const;

  let range = $state<(typeof RANGES)[number]["key"]>("1h");
  let historyField = $state<"cpuPct" | "memPct" | "memBytes">("cpuPct");
  let series = $state<HistorySeries | null>(null);
  let historyLoading = $state(false);
  let health = $state<WriterHealth | null>(null);
  let alertRules = $state<AlertRule[]>([]);
  let alertEvents = $state<AlertEvent[]>([]);

  async function loadHistory() {
    historyLoading = true;
    try {
      const to = Date.now();
      const span = RANGES.find((r) => r.key === range)?.ms ?? RANGES[0].ms;
      series = await metricsApi.history(to - span, to);
      health = await metricsApi.health();
    } catch (e) {
      series = null;
      globalToast("error", String(e));
    } finally {
      historyLoading = false;
    }
  }

  async function loadAlerts() {
    try {
      alertRules = await alertsApi.listRules();
      alertEvents = await alertsApi.recentEvents(50);
    } catch (e) {
      globalToast("error", String(e));
    }
  }

  // Loaded when the tab is opened, not on mount: history is a query over a
  // database the Free build does not even have.
  //
  // `loadHistory` reads `range` before its first await, so this also re-runs
  // when the range changes — which is why the range buttons only set the value
  // and do not fetch as well. Two requests racing to write `series` is how a
  // chart ends up showing the range the user did not pick.
  $effect(() => {
    if (tab === "history") void loadHistory();
    if (tab === "alerts") void loadAlerts();
  });

  /** Containers to offer in the rule editor, named. */
  const alertTargets = $derived(
    rows.map((r) => ({ id: r.containerId, name: r.name || r.containerId.slice(0, 12) }))
  );
</script>

<div class="content-header" data-tauri-drag-region>
  <h1>
    {t("activity.title", { default: "Activity" })}
    <span style="font-size: var(--text-sm); color: var(--text-muted); font-weight: 400; margin-left: 12px;">
      {rows.length} {t("activity.containers", { default: "containers" })} ·
      {totals.cpu.toFixed(1)}% CPU · {bytes(totals.mem)}
    </span>
  </h1>
  <div class="content-header-actions">
    <input
      class="search-input"
      type="search"
      placeholder={t("activity.search", { default: "Filter by name" })}
      bind:value={searchTerm}
    />
    <div class="tabs" role="tablist" aria-label={t("activity.views", { default: "Views" })}>
      {#each [["live", t("activity.tab_live", { default: "Live" })], ["history", t("activity.tab_history", { default: "History" })], ["alerts", t("activity.tab_alerts", { default: "Alerts" })], ["actions", t("activity.tab_actions", { default: "Actions" })]] as [key, label] (key)}
        <button
          type="button"
          role="tab"
          class="btn btn-ghost"
          class:active={tab === key}
          aria-selected={tab === key}
          onclick={() => (tab = key as Tab)}
        >
          {label}
        </button>
      {/each}
    </div>
    <div class="intervals" role="group" aria-label={t("activity.interval", { default: "Sampling period" })}>
      {#each INTERVALS as ms (ms)}
        <button
          type="button"
          class="btn btn-ghost"
          class:active={intervalMs === ms}
          onclick={() => changeInterval(ms)}
        >
          {ms / 1000}s
        </button>
      {/each}
    </div>
  </div>
</div>

<div class="content-body activity-body">
  <div class="status-bar">
    <span class="dot" class:live={connected}></span>
    <span>
      {connected
        ? t("activity.live", { default: "Live" })
        : t("activity.connecting", { default: "Connecting…" })}
    </span>
    {#if droppedTotal > 0}
      <!-- Stated rather than hidden: the charts have holes and the user should
           know why, instead of reading a gap as idleness. -->
      <span class="dropped">
        {t("activity.dropped", { default: "{count} samples dropped", count: droppedTotal })}
      </span>
    {/if}
    {#if engine?.available}
      <!-- The VM behind the containers. Shown next to the container totals
           because "8% CPU across containers" means little without knowing how
           many cores the engine actually has. -->
      <span class="engine">
        {engine.engineName || "engine"}
        {#if engine.serverVersion}· v{engine.serverVersion}{/if}
        · {engine.cpuCores}
        {t("activity.cores", { default: "cores" })}
        · {bytes(engine.memoryUsedBytes)} / {bytes(engine.memoryTotalBytes)}
        · {t("activity.disk", { default: "disk" })}
        {bytes(engine.diskUsedBytes)}
      </span>
    {/if}
    {#if lastError}
      <span class="failure">{lastError}</span>
    {/if}
  </div>

  <!-- Above the tabs' content, not inside one of them: a suggestion is about
       the machine, and which tab happens to be open does not change that. -->
  <HealSuggestionBanner onOpenSettings={() => openSettingsSection("self-healing")} />

  {#if tab === "live"}
  <div class="table-wrap">
    <ResourceTable
      rows={visibleRows}
      {history}
      {revision}
      {sortKey}
      {sortAsc}
      onSort={toggleSort}
      onSelect={showProcesses}
      selectedId={selected?.containerId ?? null}
    />
  </div>

  {#if selected}
    <aside class="processes">
      <header>
        <span>{t("activity.processes", { default: "Processes" })} · {selected.name}</span>
        <button
          class="btn btn-ghost btn-icon"
          onclick={() => (selected = null)}
          aria-label={t("common.close", { default: "Close" })}
        >
          {@html Icons.Close}
        </button>
      </header>
      {#if processesLoading}
        <p class="hint">{t("common.loading", { default: "Loading…" })}</p>
      {:else}
        <pre>{processes}</pre>
      {/if}
    </aside>
  {/if}
  {/if}

  {#if tab === "history"}
    <ProGate
      id="metrics.history"
      feature={t("activity.history_feature", { default: "Metrics history" })}
      description={t("activity.history_feature_desc", {
        default: "Keep samples on disk and look at last night, not just right now.",
      })}
    >
      <div class="panel">
        <div class="panel-controls">
          <div class="ranges" role="group" aria-label={t("activity.range", { default: "Range" })}>
            {#each RANGES as r (r.key)}
              <button
                type="button"
                class="btn btn-ghost"
                class:active={range === r.key}
                onclick={() => (range = r.key)}
              >
                {r.key}
              </button>
            {/each}
          </div>
          <select class="input" bind:value={historyField}>
            <option value="cpuPct">{t("activity.metric_cpu", { default: "CPU %" })}</option>
            <option value="memPct">{t("activity.metric_mem_pct", { default: "Memory %" })}</option>
            <option value="memBytes">{t("activity.metric_mem_bytes", { default: "Memory bytes" })}</option>
          </select>
          <button class="btn btn-ghost" onclick={loadHistory} disabled={historyLoading}>
            {historyLoading
              ? t("activity.loading", { default: "Loading…" })
              : t("activity.reload", { default: "Reload" })}
          </button>
        </div>

        {#if health && !health.writing}
          <!-- Said plainly rather than shown as an empty chart: an entitlement
               that lapsed looks exactly like a machine that was idle. -->
          <p class="notice">
            {t("activity.history_not_recording", {
              default: "History is not being recorded on this install.",
            })}
          </p>
        {:else if health && health.skippedBatches > 0}
          <p class="notice">
            {t("activity.history_paused", {
              default:
                "Recording paused while the subscription was inactive. What was already stored is untouched.",
            })}
          </p>
        {:else if health && health.droppedBatches > 0}
          <p class="notice">
            {t("activity.history_dropped", {
              default:
                "{count} batches were dropped because the disk could not keep up — the gaps below are real.",
              count: health.droppedBatches,
            })}
          </p>
        {/if}

        {#if series}
          <HistoryChart {series} field={historyField} />
        {/if}
      </div>
    </ProGate>
  {/if}

  {#if tab === "alerts"}
    <ProGate
      id="metrics.alerts"
      feature={t("activity.alerts_feature", { default: "Alerts" })}
      description={t("activity.alerts_feature_desc", {
        default: "Be told when a container stays over a threshold, instead of finding out later.",
      })}
    >
      <div class="panel">
        <AlertRuleEditor rules={alertRules} containers={alertTargets} onChanged={loadAlerts} />
        <h2 class="panel-heading">{t("activity.alert_log", { default: "Fired" })}</h2>
        <AlertLog events={alertEvents} />
      </div>
    </ProGate>
  {/if}

  {#if tab === "actions"}
    <!-- Deliberately outside any ProGate. This is the machine's own record of
         what was done to it, kept locally; a Free install keeps a shorter
         window, but reading back what it kept is not a paid feature. -->
    <div class="panel">
      <ActionFeed />
    </div>
  {/if}
</div>

<style>
  .tabs {
    display: inline-flex;
    gap: 2px;
  }
  .tabs .active,
  .ranges .active {
    background: var(--bg-secondary);
    color: var(--text-primary);
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 16px;
    border: 1px solid var(--border-primary);
    border-radius: 8px;
  }
  .panel-controls {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
  }
  .ranges {
    display: inline-flex;
    gap: 2px;
  }
  .panel-heading {
    margin: 0;
    font-size: var(--text-base);
  }
  .notice {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }

  .activity-body {
    display: flex;
    flex-direction: column;
    padding: 0;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 20px;
    border-bottom: 1px solid var(--border-primary);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-muted);
  }

  .dot.live {
    background: var(--color-success, #22c55e);
  }

  .engine {
    margin-left: auto;
    color: var(--text-muted);
  }

  .dropped {
    color: var(--color-warning, #f59e0b);
  }

  .failure {
    color: var(--color-danger, #ef4444);
  }

  .table-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .processes {
    border-top: 1px solid var(--border-primary);
    background: var(--bg-secondary);
    max-height: 260px;
    overflow: auto;
    padding: 12px 20px;
  }

  .processes header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    margin-bottom: 8px;
  }

  .processes pre {
    margin: 0;
    font-size: 11px;
    white-space: pre;
    overflow-x: auto;
    color: var(--text-secondary);
  }

  .hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 0;
  }

  .intervals {
    display: flex;
    gap: 2px;
  }

  .intervals .active {
    background: var(--bg-card-hover, var(--bg-secondary));
    color: var(--text-primary);
  }

  .search-input {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: var(--text-xs);
    padding: 5px 8px;
    width: 170px;
  }
</style>

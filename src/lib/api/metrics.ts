import { call } from "./client";
import { resolveApiBase, getApiToken } from "./client";

/**
 * Live metrics, and the history behind them.
 *
 * Sampling happens once on the backend, for everyone. This module is the two
 * ways to read it: `subscribeMetrics` for what is happening now, and
 * `metricsApi.history` for what happened before the page was open.
 *
 * **The live stream is lossy on purpose.** A client that falls behind gets a
 * `stream-lagged` frame and a gap, rather than a buffer that grows without
 * limit. Anything that must not lose samples reads the durable store instead —
 * which is exactly why history is written from the collector and not from here.
 */

/** One container, one instant. Mirrors `MetricSample` in the collector. */
export interface MetricSample {
  /** Unix milliseconds. */
  ts: number;
  instance: string;
  containerId: string;
  name: string;
  cpuPct: number;
  memBytes: number;
  memLimitBytes: number;
  memPct: number;
  netRxBytes: number;
  netTxBytes: number;
  blockReadBytes: number;
  blockWriteBytes: number;
  pids: number;
}

/** One tick's worth of samples, or the reason there are none. */
export interface MetricsBatch {
  samples?: MetricSample[];
  intervalMs?: number;
  /** Present when sampling itself failed — the daemon is unreachable. */
  error?: string;
}

export interface MetricsHandlers {
  onBatch?: (batch: MetricsBatch) => void;
  /**
   * The server dropped frames for this client. Record the hole; do not
   * interpolate across it, or the chart claims load that was never observed.
   */
  onLagged?: (dropped: number) => void;
  onError?: () => void;
}

const TOPIC = "metrics.sample";
const LAGGED_EVENT = "stream-lagged";
const INITIAL_RETRY_MS = 2000;
const MAX_RETRY_MS = 30000;

/**
 * Subscribe to the live sample stream until the returned function is called.
 *
 * The subscription is also what starts sampling: the collector counts
 * subscribers to this topic and does nothing while there are none. Closing the
 * page therefore stops the daemon calls, with no stop button to forget.
 */
export function subscribeMetrics(handlers: MetricsHandlers): () => void {
  // One transport, in both the desktop app and the browser.
  //
  // The collector publishes this topic to the SSE broadcast only — there is no
  // `app.emit` for it — and it decides whether to sample at all by counting
  // subscribers to `/api/events`. A Tauri-event listener would therefore hear
  // nothing *and* leave that count at zero, so nothing would be sampled to hear.
  // The desktop webview can reach the local API exactly like the browser does.
  let source: EventSource | null = null;
  let cancelled = false;
  let retryTimeout: ReturnType<typeof setTimeout> | null = null;
  let retryDelay = INITIAL_RETRY_MS;

  async function connect() {
    const token = await getApiToken();
    const base = await resolveApiBase();
    if (cancelled) return;
    // The topic is in the query string because the backend counts subscribers
    // per topic — without it the collector never starts.
    const params = new URLSearchParams({ topics: TOPIC });
    if (token) params.set("token", token);
    source = new EventSource(`${base}/api/events?${params}`);

    source.addEventListener(TOPIC, (e: MessageEvent) => {
      try {
        handlers.onBatch?.(JSON.parse(e.data) as MetricsBatch);
      } catch {
        // A malformed frame is not worth tearing the page down for.
      }
    });

    source.addEventListener(LAGGED_EVENT, (e: MessageEvent) => {
      let dropped = 0;
      try {
        dropped = Number(JSON.parse(e.data)?.dropped ?? 0);
      } catch {
        // The count is a detail; the gap is the point.
      }
      handlers.onLagged?.(dropped);
    });

    source.onopen = () => {
      retryDelay = INITIAL_RETRY_MS;
    };

    source.onerror = () => {
      handlers.onError?.();
      source?.close();
      source = null;
      if (cancelled) return;
      retryTimeout = setTimeout(connect, retryDelay);
      retryDelay = Math.min(retryDelay * 2, MAX_RETRY_MS);
    };
  }

  void connect();

  return () => {
    cancelled = true;
    if (retryTimeout) clearTimeout(retryTimeout);
    source?.close();
    source = null;
  };
}

// ===== History (Pro) =====

export interface HistoryPoint {
  ts: number;
  containerId: string;
  cpuPct: number;
  memBytes: number;
  memPct: number;
  netRxBytes: number;
  netTxBytes: number;
  blockReadBytes: number;
  blockWriteBytes: number;
}

export interface HistorySeries {
  /**
   * `raw` (the collector's tick) or `1m` (per-minute averages). Say which: a
   * one-minute average is a different claim from a two-second sample.
   */
  resolution: "raw" | "1m";
  /**
   * Break the line when two points are further apart than this. Interpolating
   * across a gap draws data nobody collected — usually while the machine slept.
   */
  expectedStepMs: number;
  points: HistoryPoint[];
  /** Names for every container id, including ones since deleted. */
  names: Record<string, string>;
}

/** Why history might have holes. */
export interface WriterHealth {
  acceptedBatches: number;
  /** Lost because the disk could not keep up — a real hole in the data. */
  droppedBatches: number;
  /** Not written because Pro had lapsed. A different sentence entirely. */
  skippedBatches: number;
  dbBytes: number;
  /** False when this build is not recording history at all. */
  writing: boolean;
}

export const metricsApi = {
  /** Sampling period, in milliseconds. Clamped by the backend. */
  setInterval: (ms: number) =>
    call<number>("set_metrics_interval", { ms }, "POST", "/api/metrics/interval", undefined, { ms }),

  /**
   * Processes inside one container, on demand.
   *
   * Not part of the tick: this is one `docker top` per container per period for
   * a panel that is usually closed.
   */
  containerTop: (containerId: string) =>
    call<string>("container_top", { containerId }, "GET", "/api/containers/top", { containerId }),

  history: (fromMs: number, toMs: number, containerIds: string[] = []) =>
    call<HistorySeries>("", undefined, "GET", "/api/metrics/history", {
      from: String(fromMs),
      to: String(toMs),
      ...(containerIds.length ? { containers: containerIds.join(",") } : {}),
    }),

  health: () => call<WriterHealth>("", undefined, "GET", "/api/metrics/health"),
};

// ===== Alerts (Pro) =====

export type AlertMetric = "cpu_pct" | "mem_pct" | "mem_bytes";

export interface AlertRule {
  /** 0 for a rule that has not been saved yet. */
  id: number;
  name: string;
  metric: AlertMetric;
  threshold: number;
  /** The threshold must hold this long before the rule fires. */
  durationSecs: number;
  /** Silence after firing, so one incident is one notification. */
  cooldownSecs: number;
  /** Absent means every container. */
  containerId?: string;
  enabled: boolean;
}

export interface AlertEvent {
  id: number;
  ruleId: number;
  ruleName: string;
  containerId: string;
  containerName: string;
  ts: number;
  value: number;
  threshold: number;
  metric: AlertMetric;
}

export interface BacktestResult {
  /** How many times this rule would have fired over the stored window. */
  wouldFire: number;
  fromMs: number;
  toMs: number;
  sampleCount: number;
}

export const alertsApi = {
  listRules: () => call<AlertRule[]>("alerts_list_rules", undefined, "GET", "/api/alerts/rules"),

  saveRule: (rule: AlertRule) =>
    call<number>("alerts_save_rule", { rule }, "POST", "/api/alerts/rules", undefined, rule),

  deleteRule: (id: number) =>
    call<void>("alerts_delete_rule", { id }, "POST", "/api/alerts/rules/delete", undefined, { id }),

  recentEvents: (limit = 50) =>
    call<AlertEvent[]>("alerts_recent_events", { limit }, "GET", "/api/alerts/events", {
      limit: String(limit),
    }),

  /**
   * Replay a rule over stored history before saving it.
   *
   * This is how somebody picks a sensible threshold on the first try: "80% for
   * five minutes" means nothing until you know it would have fired eleven times
   * last week.
   */
  backtest: (rule: AlertRule, fromMs: number, toMs: number) =>
    call<BacktestResult>(
      "alerts_backtest",
      { rule, fromMs, toMs },
      "POST",
      "/api/alerts/backtest",
      undefined,
      { rule, fromMs, toMs },
    ),
};

/**
 * Wires fired alerts into the notification centre, once per session.
 *
 * The backend decides *whether* a rule fired — it has the samples and the state
 * machine. This decides what the user sees, because the rules about that already
 * live here: `osNotify` only speaks when the window is in the background, and
 * the notification store is where anything the user might have missed belongs.
 *
 * Started from `App.svelte` rather than the Activity page: an alert is worth
 * knowing about whether or not that page is open, and a per-page subscription
 * would mean alerts only arrive while you are already watching.
 */

import { resolveApiBase, getApiToken } from "./api";
import { osNotify } from "./osNotify";
import { pushNotification } from "../store/notifications.svelte";
import { t } from "./i18n.svelte";

const EVENT = "alert.fired";
const INITIAL_RETRY_MS = 2000;
const MAX_RETRY_MS = 30000;

export interface FiredAlert {
  ruleId: number;
  ruleName: string;
  containerId: string;
  containerName: string;
  ts: number;
  value: number;
  threshold: number;
  /**
   * Every metric the backend can emit.
   *
   * Previously only the three container metrics were listed, so `format` could
   * assume a percentage and TypeScript raised nothing when security and runtime
   * alerts started arriving — they rendered as "5.0%, over the 5 threshold".
   * Keep this in step with `AlertMetric` in `commands/alerts.rs`.
   */
  metric:
    | "cpu_pct"
    | "mem_pct"
    | "mem_bytes"
    | "security_score"
    | "new_vulnerabilities"
    | "runtime_event";
  /** Set for security and runtime alerts, whose subject is not a container. */
  imageRef?: string;
}

/** Falco priorities, by the numeric level the backend sends. */
const PRIORITY_NAMES = [
  "debug",
  "informational",
  "notice",
  "warning",
  "error",
  "critical",
  "alert",
  "emergency",
];

function format(alert: FiredAlert): { title: string; detail: string } {
  // A scan or a runtime event has no container; its subject is the image or the
  // service. Falling through to `containerId.slice()` on those threw, and the
  // subscriber's catch turned that into silence rather than a visible bug.
  const container =
    alert.containerName || alert.imageRef || alert.containerId?.slice(0, 12) || "";

  let value: string;
  if (alert.metric === "mem_bytes") {
    value = `${(alert.value / (1024 * 1024)).toFixed(0)} MB`;
  } else if (alert.metric === "security_score") {
    value = `${alert.value.toFixed(0)}/100`;
  } else if (alert.metric === "new_vulnerabilities") {
    value = `${alert.value.toFixed(0)}`;
  } else if (alert.metric === "runtime_event") {
    // The value is a priority level, not a quantity — reporting "5.0%" here was
    // worse than reporting nothing.
    value = PRIORITY_NAMES[Math.round(alert.value)] ?? String(alert.value);
  } else {
    value = `${alert.value.toFixed(1)}%`;
  }
  return {
    title: t("activity.alert_fired_title", {
      default: "{rule}: {container}",
      rule: alert.ruleName,
      container,
    }),
    // The number that tripped it, not just that something did — "CPU alert" and
    // "CPU at 340%" are different amounts of information at 3am.
    //
    // "over the N threshold" only reads correctly for a metric that rose past a
    // ceiling. A posture score trips by falling, and a runtime event has no
    // threshold to speak of, so those get their own wording.
    detail:
      alert.metric === "runtime_event"
        ? t("activity.alert_fired_runtime", {
            default: "{value} priority runtime event",
            value,
          })
        : alert.metric === "security_score"
          ? t("activity.alert_fired_below", {
              default: "{value}, below the {threshold} minimum",
              value,
              threshold: alert.threshold,
            })
          : t("activity.alert_fired_detail", {
              default: "{value}, over the {threshold} threshold",
              value,
              threshold: alert.threshold,
            }),
  };
}

function handle(alert: FiredAlert): void {
  const { title, detail } = format(alert);
  pushNotification({ kind: "message", status: "error", title, detail });
  // Best effort, and silent when the window has focus: the in-app entry has
  // already been made either way.
  void osNotify(title, detail);
}

/** Subscribe until the returned function is called. */
export function startAlertNotifications(): () => void {
  // SSE in both transports: `alert.fired` is published to the broadcast channel
  // only, so a Tauri-event listener would wait forever.
  let source: EventSource | null = null;
  let cancelled = false;
  let retryTimeout: ReturnType<typeof setTimeout> | null = null;
  let retryDelay = INITIAL_RETRY_MS;

  async function connect() {
    const token = await getApiToken();
    const base = await resolveApiBase();
    if (cancelled) return;
    // Only this topic: subscribing to everything would make this connection
    // count as a metrics watcher and keep the collector sampling forever.
    const params = new URLSearchParams({ topics: EVENT });
    if (token) params.set("token", token);
    source = new EventSource(`${base}/api/events?${params}`);

    source.addEventListener(EVENT, (e: MessageEvent) => {
      try {
        handle(JSON.parse(e.data) as FiredAlert);
      } catch {
        // A malformed frame is not worth a toast about a toast.
      }
    });

    source.onopen = () => {
      retryDelay = INITIAL_RETRY_MS;
    };

    source.onerror = () => {
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

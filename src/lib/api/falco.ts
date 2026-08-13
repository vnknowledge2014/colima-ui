import { call } from "./client";

/**
 * Runtime security events, read from Falco.
 *
 * Mirrors `src-tauri/src/commands/falco_bridge.rs`.
 *
 * # ColimaUI does not detect anything
 *
 * Falco (CNCF, Apache-2.0) does the detecting, with eBPF and a rule set the
 * user controls. This screen reads its output and names containers the way the
 * user names them. Every string shown to a user must keep that distinction:
 * coverage is whatever their Falco rules cover, and promising more than that is
 * promising something we cannot deliver.
 */

/** Falco's own priority ladder, in its own words. */
export type FalcoPriority =
  | "emergency"
  | "alert"
  | "critical"
  | "error"
  | "warning"
  | "notice"
  | "informational"
  | "debug";

/**
 * Why the Runtime tab can or cannot show events.
 *
 * `runningWithoutRules` is the one that matters most: Falco up, healthy, and
 * detecting nothing because its rule set is empty. It is reported separately
 * from `ready` because a user seeing an empty event list needs to know which of
 * "nothing happened" and "nothing is being watched" they are looking at.
 */
export type FalcoStatus =
  | "missing"
  | "not_falco"
  | "installed"
  | "running_without_rules"
  | "output_not_configured"
  | "ready";

export interface FalcoState {
  status: FalcoStatus;
  version?: string | null;
  ruleCount: number;
  eventFile?: string | null;
  profile?: string | null;
}

export interface FalcoEvent {
  id: number;
  tsMs: number;
  priority: string;
  rule: string;
  output: string;
  source: string;
  tags: string[];
  containerId?: string | null;
  /** Compose `project/service` when known, else a container name, else the id. */
  containerLabel?: string | null;
  image?: string | null;
}

/** Severity order, so a filter threshold means something. */
const PRIORITY_RANK: Record<string, number> = {
  debug: 0,
  informational: 1,
  info: 1,
  notice: 2,
  warning: 3,
  error: 4,
  critical: 5,
  alert: 6,
  emergency: 7,
};

export function priorityRank(priority: string): number {
  return PRIORITY_RANK[priority.toLowerCase()] ?? 0;
}

/** Whether Falco is in a state where the event list is trustworthy. */
export function canShowEvents(status: FalcoStatus): boolean {
  return status === "ready";
}

/**
 * The prompt that would be sent to explain a set of events.
 *
 * Built backend-side from the *stored* rows, which are the redacted ones, and
 * returned rather than sent: the user reads it before it leaves the machine.
 */
export interface FalcoExplainPayload {
  payload: string;
  eventCount: number;
  /** Events beyond the per-request ceiling. Shown, never silently dropped. */
  omitted: number;
}

export const falcoApi = {
  state: () => call<FalcoState>("falco_state", undefined, "GET", "/api/security/falco/state"),

  events: (limit = 200) =>
    call<FalcoEvent[]>("falco_events", { limit }, "GET", "/api/security/falco/events", {
      limit: String(limit),
    }),

  /**
   * Start or stop the reader. Stopping is never gated on entitlement — a
   * background loop a lapsed user can see and cannot stop is indefensible.
   */
  setWatching: (enabled: boolean) =>
    call<boolean>(
      "falco_set_watching",
      { enabled },
      "POST",
      "/api/security/falco/watch",
      undefined,
      { enabled }
    ),

  watching: () =>
    call<boolean>("falco_watching", undefined, "GET", "/api/security/falco/watch"),

  /**
   * Build (but do not send) the prompt explaining these events.
   *
   * The request itself is made by the caller with the user's own key, so the
   * payload can be shown first — the same order compose diagnosis established.
   */
  explainPayload: (eventIds: number[]) =>
    call<FalcoExplainPayload>(
      "falco_explain_payload",
      { eventIds },
      "POST",
      "/api/security/falco/explain",
      undefined,
      { eventIds }
    ),
};

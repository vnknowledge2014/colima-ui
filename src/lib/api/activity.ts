import { call } from "./client";

// ===== Activity history =====

/** Mirrors `ActivityKind` in `activity.rs`. */
export type ActivityKind = "destructive" | "lifecycle" | "task" | "config";
/** Mirrors `ActivityActor`. `app` means the app decided, not the user. */
export type ActivityActor = "user" | "app";
/** Mirrors `ActivityOutcome`. */
export type ActivityOutcome = "ok" | "failed" | "denied" | "cancelled";

/** Mirrors `FeedSource` in `activity_feed.rs`. */
export type FeedSource = "activity" | "self_heal" | "alert" | "security_scan" | "compose_fix";

/** Mirrors `FeedItem` in `activity_feed.rs`. */
export interface FeedItem {
  id: string;
  ts: number;
  source: FeedSource;
  kind: ActivityKind;
  verb: string;
  targetKind: string;
  target: string;
  targetName: string;
  actor: ActivityActor;
  outcome: ActivityOutcome;
  detail: string;
  durationMs?: number;
  /** Sources folded into this row, when it stands for more than one event record. */
  mergedFrom?: FeedSource[];
}

/** Mirrors `Feed`. */
export interface ActivityFeed {
  items: FeedItem[];
  /**
   * Stores that could not be read. Non-empty means the timeline is incomplete
   * and the UI must say so — silence would read as "nothing happened".
   */
  partial: string[];
}

export interface ActivityFeedFilter {
  kind?: ActivityKind;
  target?: string;
  fromMs?: number;
  toMs?: number;
  limit?: number;
}

export const activityApi = {
  /**
   * The merged timeline: the action log plus the four stores that keep their
   * own outcomes.
   *
   * Not gated. A Free install keeps a shorter history, but what it kept is its
   * own record of its own machine.
   */
  feed: (filter: ActivityFeedFilter = {}) =>
    call<ActivityFeed>(
      "activity_feed",
      {
        kind: filter.kind,
        target: filter.target,
        fromMs: filter.fromMs,
        toMs: filter.toMs,
        limit: filter.limit,
      },
      "GET",
      "/api/activity/feed",
      {
        ...(filter.kind ? { kind: filter.kind } : {}),
        ...(filter.target ? { target: filter.target } : {}),
        ...(filter.fromMs ? { fromMs: String(filter.fromMs) } : {}),
        ...(filter.toMs ? { toMs: String(filter.toMs) } : {}),
        ...(filter.limit ? { limit: String(filter.limit) } : {}),
      },
    ),

  /**
   * Write the timeline to a file the user picked.
   *
   * Pro. Reading the timeline is not gated — this is, because taking the log
   * somewhere else is the part that goes beyond "see your own machine".
   */
  export: (destDir: string, fileName: string, format: "json" | "csv", overwrite = false) =>
    call<string>(
      "activity_export",
      { destDir, fileName, format, overwrite },
      "POST",
      "/api/activity/export",
      undefined,
      { destDir, fileName, format, overwrite },
    ),
};

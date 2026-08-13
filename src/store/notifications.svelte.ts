/**
 * Session notifications: things that happened, and things still happening.
 *
 * This grew out of the session error log rather than beside it. That log already
 * had the shape a notification centre needs — session-scoped, capped, unpersisted,
 * with a panel reading it — and `errorReporter` already wrote every error into it.
 * A second store of the same shape would have been the same state under a new name,
 * with two chances to disagree about what the user has seen.
 *
 * What is new: entries that are not errors, entries with a *lifetime* (a transfer
 * that is still running, with progress and a cancel), and a read/unread flag for
 * the badge.
 *
 * Nothing is persisted. Entries carry command output we have no reason to keep on
 * disk, and every transfer dies with the process anyway.
 */

import type { AppError } from "../lib/errors";
import type { TransferSnapshot } from "../lib/api/transfer";

export type NotificationKind = "job" | "message" | "announcement";
export type NotificationStatus =
  | "running"
  | "cancelling"
  | "success"
  | "error"
  | "cancelled"
  /**
   * It ended, but this client never saw how.
   *
   * The backend forgets a finished transfer after a minute; a client offline for
   * longer comes back to a job that is simply absent. Calling that "success"
   * would report a failed export as a completed one.
   */
  | "ended";

/** The live part of a `kind: "job"` entry. */
export interface JobProgress {
  jobId: string;
  /**
   * When this client learned the job existed.
   *
   * Separate from the entry's `timestamp`, which moves on every settle — this one
   * has to stay put because `reconcileJobs` compares it against when a snapshot
   * was requested.
   */
  startedAt: number;
  bytes: number;
  /**
   * Docker reports no total for transfers, so this is derived from image metadata
   * where it can be had at all. Never a completion contract.
   */
  totalEstimate: number | null;
  /** The last line the runtime printed, for operations that report text.  */
  message?: string;
}

/** The vendor-authored part of a `kind: "announcement"` entry. */
export interface AnnouncementMeta {
  /** The feed's own id, which is what the read-set on disk remembers. */
  announcementId: string;
  /** `info` | `warning` | `critical`. Anything unrecognised arrives as `info`. */
  severity: "info" | "warning" | "critical";
  /** Vendor-controlled, so it is only rendered once Phase 3 has vetted it. */
  linkUrl?: string;
}

export interface NotificationEntry {
  id: number;
  kind: NotificationKind;
  status: NotificationStatus;
  title: string;
  /** Second line: a destination, a container path, a runtime message. */
  detail?: string;
  timestamp: number;
  read: boolean;
  /** How many identical repeats collapsed into this entry. */
  count: number;
  /** Present only while `kind === "job"`. No callbacks live here — see `cancelJob`. */
  job?: JobProgress;
  /** Present only while `kind === "announcement"`. */
  announcement?: AnnouncementMeta;
  /** Structured payload when this came from a failed operation. */
  error?: AppError;
  hint?: string;
  /** What the user was doing, when the call site said so. */
  action?: string;
}

/**
 * Older entries are dropped. A running job is never dropped: it is the one thing
 * here the user can still act on, and losing it would strand a transfer with no
 * way to cancel it.
 */
const MAX_ENTRIES = 100;

/**
 * How long an identical message collapses into the existing entry instead of
 * stacking.
 *
 * Deliberately not tied to how long a toast stays on screen. Reusing the toast's
 * own collapsing was the first attempt and does not work: a toast expires after
 * 4-9s, so anything retrying on a slower cadence than that finds no toast to
 * collapse into and adds a fresh entry every time — precisely the flood this
 * guards against.
 */
const COLLAPSE_WINDOW_MS = 5 * 60 * 1000;

let _nextId = 1;

export const notificationState = $state({
  entries: [] as NotificationEntry[],
  /** The error-details panel, opened from a toast's "Details" action. */
  isOpen: false,
  /** The notification centre. Separate: the two show different things. */
  panelOpen: false,
});

/**
 * Unread count for the badge.
 *
 * Computed from the entries on every read rather than kept as a counter, so
 * trimming and clearing cannot leave it disagreeing with the list.
 */
export function unreadCount(): number {
  return notificationState.entries.filter((e) => !e.read).length;
}

function isRunning(entry: NotificationEntry): boolean {
  return entry.status === "running" || entry.status === "cancelling";
}

/** Identity for collapsing repeats. Progress is excluded: it always differs. */
function collapseKey(entry: NotificationEntry): string {
  return `${entry.kind}\u0000${entry.status}\u0000${entry.title}`;
}

function trim(): void {
  if (notificationState.entries.length <= MAX_ENTRIES) return;
  // Drop the oldest finished entries until the cap is met, leaving order alone.
  // Partitioning into running-then-rest also met the cap, but reordered the list
  // the user was reading — a transfer would jump to the top because something
  // unrelated was pushed.
  let excess = notificationState.entries.length - MAX_ENTRIES;
  for (let i = notificationState.entries.length - 1; i >= 0 && excess > 0; i--) {
    if (isRunning(notificationState.entries[i])) continue;
    notificationState.entries.splice(i, 1);
    excess--;
  }
}

export interface PushInput {
  kind?: NotificationKind;
  status?: NotificationStatus;
  title: string;
  detail?: string;
  job?: JobProgress;
  announcement?: AnnouncementMeta;
  error?: AppError;
  hint?: string;
  action?: string;
}

/**
 * Add an entry, or bump the identical one already present.
 *
 * Returns the entry's id so a caller can update it later.
 */
export function pushNotification(input: PushInput): number {
  const entry: NotificationEntry = {
    id: _nextId++,
    kind: input.kind ?? "message",
    status: input.status ?? "success",
    title: input.title,
    detail: input.detail,
    timestamp: Date.now(),
    read: false,
    count: 1,
    job: input.job,
    announcement: input.announcement,
    error: input.error,
    hint: input.hint,
    action: input.action,
  };

  // Jobs are never collapsed: two exports of the same image are two transfers,
  // each with its own progress and its own cancel. Announcements are not either:
  // two entries from the feed are two pieces of news, and the feed's `id` is
  // their real identity — merging them by title would hide one behind a `×2`.
  if (entry.kind !== "job" && entry.kind !== "announcement") {
    const key = collapseKey(entry);
    const existing = notificationState.entries.find(
      (e) =>
        collapseKey(e) === key && Date.now() - e.timestamp < COLLAPSE_WINDOW_MS
    );
    if (existing) {
      existing.count += 1;
      existing.timestamp = Date.now();
      existing.read = false;
      // Keep the newest payload: an error's detail can change between retries.
      existing.error = entry.error;
      existing.detail = entry.detail;
      // The list is ordered by position, not sorted on read. Bumping the time
      // without moving the entry would show it as newer than the entries above.
      const at = notificationState.entries.indexOf(existing);
      if (at > 0) {
        notificationState.entries.splice(at, 1);
        notificationState.entries.unshift(existing);
      }
      return existing.id;
    }
  }

  // Open panel means the user is already looking at this list, so a new entry is
  // seen the moment it lands. Leaving it unread lit the badge behind the very
  // panel that was showing it.
  entry.read = notificationState.panelOpen;
  notificationState.entries.unshift(entry);
  trim();
  return entry.id;
}

// ===== Jobs =====

export function startJob(input: {
  jobId: string;
  title: string;
  detail?: string;
  totalEstimate: number | null;
}): number {
  // The backend spawns the command before returning the id, so a fast failure can
  // reach `settleJob` first and create the entry. Pushing another here would leave
  // one transfer showing as two, forever — jobs are deliberately never collapsed.
  const existing = findJob(input.jobId);
  if (existing) {
    existing.title = input.title;
    if (input.detail) existing.detail = input.detail;
    if (existing.job) existing.job.totalEstimate = input.totalEstimate;
    return existing.id;
  }
  return pushNotification({
    kind: "job",
    status: "running",
    title: input.title,
    detail: input.detail,
    job: {
      jobId: input.jobId,
      startedAt: Date.now(),
      bytes: 0,
      totalEstimate: input.totalEstimate,
    },
  });
}

function findJob(jobId: string): NotificationEntry | undefined {
  return notificationState.entries.find((e) => e.job?.jobId === jobId);
}

export function updateJob(
  jobId: string,
  update: { bytes?: number; totalEstimate?: number | null; message?: string }
): void {
  const entry = findJob(jobId);
  if (!entry?.job) return;
  if (update.bytes !== undefined) entry.job.bytes = update.bytes;
  if (update.totalEstimate) entry.job.totalEstimate = update.totalEstimate;
  if (update.message) entry.job.message = update.message;
}

/** Mark a job as cancelling. The backend still decides how it actually ends. */
export function markJobCancelling(jobId: string): void {
  const entry = findJob(jobId);
  if (entry && isRunning(entry)) entry.status = "cancelling";
}

/**
 * Record a job's outcome.
 *
 * An unknown job id creates an entry rather than doing nothing. The backend spawns
 * the command before the caller holds the id, so a fast failure can be reported
 * before this store has ever heard of the job — silently dropping it was how an
 * entry could sit at "running" forever.
 */
export function settleJob(
  jobId: string,
  status: Extract<NotificationStatus, "success" | "error" | "cancelled" | "ended">,
  info: { bytes?: number; title?: string; detail?: string; error?: AppError } = {}
): void {
  const entry = findJob(jobId);
  if (!entry) {
    pushNotification({
      kind: "job",
      status,
      title: info.title ?? jobId,
      detail: info.detail,
      error: info.error,
      job: { jobId, startedAt: Date.now(), bytes: info.bytes ?? 0, totalEstimate: null },
    });
    return;
  }
  entry.status = status;
  entry.read = false;
  entry.timestamp = Date.now();
  if (info.bytes !== undefined && entry.job) entry.job.bytes = info.bytes;
  if (info.detail) entry.detail = info.detail;
  if (info.error) entry.error = info.error;
}

/**
 * Bring job entries in line with what the backend actually has.
 *
 * Events are lossy — the SSE channel drops frames under lag by design, and a
 * reconnect misses everything sent while it was down — so this runs on connect and
 * after every reconnect. A job the backend no longer lists has finished and been
 * forgotten; leaving it spinning would be a lie, so it is settled as done.
 */
export function reconcileJobs(snapshots: TransferSnapshot[], asOf = Date.now()): void {
  const byId = new Map(snapshots.map((s) => [s.jobId, s]));

  for (const snapshot of snapshots) {
    const entry = findJob(snapshot.jobId);
    if (!entry) {
      // A job started by another window, or before this page loaded.
      pushNotification({
        kind: "job",
        status: statusFromSnapshot(snapshot.status),
        title: snapshot.targetLabel || snapshot.jobId,
        detail: snapshot.error ?? undefined,
        job: {
          jobId: snapshot.jobId,
          startedAt: snapshot.startedAt,
          bytes: snapshot.bytes,
          totalEstimate: snapshot.totalEstimate,
        },
      });
      continue;
    }
    if (entry.job) {
      entry.job.bytes = snapshot.bytes;
      if (snapshot.totalEstimate) entry.job.totalEstimate = snapshot.totalEstimate;
    }
    const status = statusFromSnapshot(snapshot.status);
    if (status === "running") {
      // Never downgrade: `cancelling` means the user has pressed cancel and the
      // backend has not settled it yet, and the registry still reports `running`
      // for exactly that window. Overwriting it makes the button un-press itself.
      if (entry.status === "running") entry.status = "running";
    } else {
      // A terminal state always wins, whichever transport delivered it first —
      // and it is news, so it should light the badge.
      if (entry.status !== status) entry.read = false;
      entry.status = status;
    }
    if (snapshot.error) entry.detail = snapshot.error;
  }

  // Anything we still think is running but the backend has forgotten. It ended —
  // the registry only drops terminal entries — but *how* is not knowable from
  // here, and guessing "success" would report a failed export as a finished one.
  //
  // `asOf` is when the question was asked, and only jobs that already existed then
  // can be answered by it. A snapshot requested before a transfer started but
  // delivered after would otherwise declare that live transfer over.
  //
  // Strictly earlier, not "at or earlier": these are millisecond timestamps, so a
  // job starting in the same tick as the request is genuinely ambiguous. Skipping
  // it costs one more reconcile round; ending it would strand a transfer that is
  // still writing behind a cancel button that no longer does anything.
  for (const entry of notificationState.entries) {
    if (
      entry.job &&
      isRunning(entry) &&
      entry.job.startedAt < asOf &&
      !byId.has(entry.job.jobId)
    ) {
      entry.status = "ended";
      entry.read = false;
      entry.timestamp = Date.now();
    }
  }
}

function statusFromSnapshot(status: TransferSnapshot["status"]): NotificationStatus {
  switch (status) {
    case "starting":
    case "running":
      return "running";
    case "success":
      return "success";
    case "cancelled":
      return "cancelled";
    case "failed":
      return "error";
  }
}

// ===== Read state =====

export function markRead(id: number): void {
  const entry = notificationState.entries.find((e) => e.id === id);
  if (entry) entry.read = true;
}

export function markAllRead(): void {
  for (const entry of notificationState.entries) entry.read = true;
}

/** Clear finished entries. A running job stays: it is still actionable. */
export function clearFinished(): void {
  notificationState.entries = notificationState.entries.filter(isRunning);
}

// ===== Error log (unchanged surface) =====

/**
 * A toast disappears after a few seconds, which is not long enough to read a
 * command and an exit code, let alone copy them into a bug report. Every reported
 * error is kept here so the details view can show it afterwards.
 */
export function recordError(error: AppError, action?: string, text?: string): void {
  pushNotification({
    kind: "message",
    status: "error",
    // `text` is what the user was shown in the toast. Falling back to the code
    // would put `command_failed` in the notification list.
    title: text ?? (action ? `${action}: ${error.detail}` : error.detail),
    detail: error.detail,
    error,
    action,
  });
}

/** A notification that definitely came from a failure. */
export type ErrorNotification = NotificationEntry & { error: AppError };

/**
 * Entries the error-details panel shows: the failures, not the transfers.
 *
 * The predicate is what lets that panel read `entry.error.code` without a
 * non-null assertion at every use.
 */
export function errorEntries(): ErrorNotification[] {
  return notificationState.entries.filter(
    (e): e is ErrorNotification => e.kind === "message" && e.error !== undefined
  );
}

export function clearErrorLog(): void {
  // Only what that panel actually shows. A failed transfer also carries an
  // `error`, and clearing the error panel must not delete it from the
  // notification list where the user manages transfers.
  const shown = new Set(errorEntries().map((e) => e.id));
  notificationState.entries = notificationState.entries.filter(
    (e) => !shown.has(e.id)
  );
}

export function openErrorLog(): void {
  notificationState.isOpen = true;
}

export function closeErrorLog(): void {
  notificationState.isOpen = false;
}

// ===== Notification centre =====

/** Opening is reading: everything visible stops counting towards the badge. */
export function openNotificationPanel(): void {
  notificationState.panelOpen = true;
  markAllRead();
}

export function closeNotificationPanel(): void {
  notificationState.panelOpen = false;
}

/**
 * Plain-text rendering of an entry for the clipboard.
 *
 * Fields are already redacted upstream (`errors.ts` / `redact.rs`), so this is
 * safe to paste into an issue.
 */
export function formatEntryForClipboard(entry: NotificationEntry): string {
  const lines: string[] = [];
  if (entry.action) lines.push(`action: ${entry.action}`);
  if (entry.error) {
    lines.push(`code: ${entry.error.code}`, `detail: ${entry.error.detail}`);
    if (entry.error.command) lines.push(`command: ${entry.error.command}`);
    if (entry.error.exitCode !== undefined) {
      lines.push(`exit code: ${entry.error.exitCode}`);
    }
  } else {
    lines.push(entry.title);
    if (entry.detail) lines.push(entry.detail);
  }
  lines.push(`time: ${new Date(entry.timestamp).toISOString()}`);
  return lines.join("\n");
}

/** Test seam: forget everything so collapsing starts fresh. */
export function _resetNotificationsForTest(): void {
  notificationState.entries = [];
  notificationState.isOpen = false;
  notificationState.panelOpen = false;
}

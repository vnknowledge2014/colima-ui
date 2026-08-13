import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import {
  notificationState,
  pushNotification,
  startJob,
  markJobCancelling,
  updateJob,
  settleJob,
  reconcileJobs,
  markAllRead,
  clearFinished,
  unreadCount,
  errorEntries,
  recordError,
  clearErrorLog,
  formatEntryForClipboard,
  _resetNotificationsForTest,
} from "./notifications.svelte";
import type { AppError } from "../lib/errors";
import type { TransferSnapshot } from "../lib/api/transfer";

/**
 * The behaviours worth pinning down are the ones that were wrong before: a
 * retrying poller flooding the list, a running transfer being evicted or cleared
 * out from under the user, and an outcome arriving for a job the store never saw.
 */

const anError: AppError = { code: "command_failed", detail: "no such container" };

function snapshot(over: Partial<TransferSnapshot> = {}): TransferSnapshot {
  return {
    jobId: "save-1",
    kind: "save",
    status: "running",
    bytes: 0,
    totalEstimate: null,
    startedAt: 0,
    finishedAt: null,
    targetLabel: "alpine:3.19",
    error: null,
    ...over,
  };
}

beforeEach(() => _resetNotificationsForTest());
afterEach(() => vi.useRealTimers());

describe("collapsing", () => {
  it("collapses a repeat instead of stacking it", () => {
    pushNotification({ title: "Poller failed", status: "error" });
    pushNotification({ title: "Poller failed", status: "error" });

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].count).toBe(2);
  });

  it("keeps collapsing long after a toast would have expired", () => {
    // The first attempt at this reused the toast's own 4-9s collapsing, so a
    // poller retrying every 5s added a fresh entry each time — the exact flood
    // the guard existed to prevent.
    vi.useFakeTimers();
    pushNotification({ title: "Poller failed", status: "error" });
    vi.advanceTimersByTime(30_000);
    pushNotification({ title: "Poller failed", status: "error" });

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].count).toBe(2);
  });

  it("does not collapse two transfers that happen to share a title", () => {
    startJob({ jobId: "save-1", title: "alpine:3.19", totalEstimate: null });
    startJob({ jobId: "save-2", title: "alpine:3.19", totalEstimate: null });

    // Two exports are two transfers: each has its own progress and its own cancel.
    expect(notificationState.entries).toHaveLength(2);
  });
});

describe("jobs", () => {
  it("tracks progress against the right job", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: 100 });
    startJob({ jobId: "save-2", title: "b", totalEstimate: null });
    updateJob("save-2", { bytes: 42 });

    const entries = notificationState.entries;
    expect(entries.find((e) => e.job?.jobId === "save-2")?.job?.bytes).toBe(42);
    expect(entries.find((e) => e.job?.jobId === "save-1")?.job?.bytes).toBe(0);
  });

  it("creates an entry for an outcome it never saw start", () => {
    // The backend spawns the command before the caller holds the job id, so a
    // fast failure can arrive first. Dropping it silently left the UI able to
    // show a transfer that never resolves.
    settleJob("save-99", "error", { title: "alpine:3.19", error: anError });

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].status).toBe("error");
  });

  it("keeps a running job when the list is trimmed", () => {
    startJob({ jobId: "save-1", title: "important export", totalEstimate: null });
    for (let i = 0; i < 200; i++) pushNotification({ title: `noise ${i}` });

    const survivor = notificationState.entries.find((e) => e.job?.jobId === "save-1");
    expect(survivor, "a running transfer must not be evicted").toBeDefined();
  });

  it("keeps a running job when finished entries are cleared", () => {
    startJob({ jobId: "save-1", title: "running", totalEstimate: null });
    pushNotification({ title: "done thing" });
    clearFinished();

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].job?.jobId).toBe("save-1");
  });
});

describe("reconciling against the backend", () => {
  it("adopts a job it has never seen", () => {
    reconcileJobs([snapshot({ jobId: "cp-out-3", targetLabel: "/tmp/x" })]);

    expect(notificationState.entries[0].job?.jobId).toBe("cp-out-3");
    expect(notificationState.entries[0].status).toBe("running");
  });

  it("does not claim success for a job it never saw finish", () => {
    // The backend forgets a terminal transfer after a minute. A client offline
    // longer than that comes back to an absent job — which ended, but reporting a
    // failed export as "success" is worse than admitting the outcome is unknown.
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    // The snapshot was requested after the job started, so its absence is real
    // information about this job rather than a race.
    reconcileJobs([], Date.now() + 1);

    expect(notificationState.entries[0].status).toBe("ended");
    expect(notificationState.entries[0].read).toBe(false);
  });

  it("does not un-press a cancel that is still in flight", () => {
    // The registry keeps reporting `running` until the process actually dies, so
    // a reconcile landing in that window used to flip the button back.
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    markJobCancelling("save-1");
    reconcileJobs([snapshot({ status: "running" })]);

    expect(notificationState.entries[0].status).toBe("cancelling");
  });

  it("lights the badge when a job finishes while events were down", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    markAllRead();
    reconcileJobs([snapshot({ status: "success" })]);

    expect(unreadCount()).toBe(1);
  });

  it("lets a terminal state win over a stale running one", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    reconcileJobs([snapshot({ status: "failed", error: "boom" })]);

    expect(notificationState.entries[0].status).toBe("error");
    expect(notificationState.entries[0].detail).toBe("boom");
  });

  it("does not resurrect a job that already finished", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    settleJob("save-1", "cancelled");
    reconcileJobs([snapshot({ status: "running" })]);

    expect(notificationState.entries[0].status).toBe("cancelled");
  });
});

describe("read state", () => {
  it("counts only unread entries", () => {
    pushNotification({ title: "one" });
    pushNotification({ title: "two" });
    expect(unreadCount()).toBe(2);

    markAllRead();
    expect(unreadCount()).toBe(0);
  });

  it("marks a collapsed repeat unread again", () => {
    pushNotification({ title: "same" });
    markAllRead();
    pushNotification({ title: "same" });

    expect(unreadCount()).toBe(1);
  });
});

describe("error log surface", () => {
  it("shows failures and hides transfers", () => {
    recordError(anError, "Stop container");
    startJob({ jobId: "save-1", title: "export", totalEstimate: null });

    expect(errorEntries()).toHaveLength(1);
    expect(errorEntries()[0].error.code).toBe("command_failed");
  });

  it("clearing the error log leaves transfers alone, failed ones included", () => {
    recordError(anError);
    startJob({ jobId: "save-1", title: "export", totalEstimate: null });
    settleJob("save-1", "error", { error: anError });
    clearErrorLog();

    // A failed transfer carries an `error` too, but it belongs to the transfer
    // list — clearing the *error panel* must not delete it from there.
    expect(errorEntries()).toHaveLength(0);
    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].kind).toBe("job");
  });

  it("keeps a repeat where its timestamp says it belongs", () => {
    pushNotification({ title: "first" });
    pushNotification({ title: "second" });
    pushNotification({ title: "first" });

    // The list is rendered in array order, so a bumped timestamp has to come with
    // a move — otherwise an entry shows a newer time than the one above it.
    expect(notificationState.entries[0].title).toBe("first");
    expect(notificationState.entries[0].count).toBe(2);
  });

  it("does not reorder the list when trimming", () => {
    startJob({ jobId: "save-1", title: "running job", totalEstimate: null });
    for (let i = 0; i < 150; i++) pushNotification({ title: `noise ${i}` });

    // The newest push stays on top; the running job is kept but not hoisted.
    expect(notificationState.entries[0].title).toBe("noise 149");
    expect(
      notificationState.entries.some((e) => e.job?.jobId === "save-1")
    ).toBe(true);
  });

  it("lists a reported failure exactly once", async () => {
    // `reportError` writes the entry itself and then shows a toast. If the toast
    // also recorded one, every failure in the app would appear twice.
    const { reportError } = await import("../lib/errorReporter");
    reportError(anError, { action: "Stop container" });

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].error?.code).toBe("command_failed");
  });

  it("shows the message the user was given, not the error code", async () => {
    const { reportError } = await import("../lib/errorReporter");
    reportError(anError, { action: "Stop container" });

    expect(notificationState.entries[0].title).not.toBe("command_failed");
    expect(notificationState.entries[0].title).toContain("Stop container");
  });

  it("renders an error for the clipboard", () => {
    recordError(anError, "Stop container");
    const text = formatEntryForClipboard(errorEntries()[0]);

    expect(text).toContain("action: Stop container");
    expect(text).toContain("code: command_failed");
    expect(text).toContain("detail: no such container");
  });
});

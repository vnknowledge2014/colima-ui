/**
 * Wires transfer events into the notification store, once per session.
 *
 * Lives here rather than in `App.svelte` so the reconciliation rules can be tested
 * without mounting the app, and rather than in the store so the store stays free of
 * transport concerns.
 *
 * The shape worth understanding: **events are the fast path, `transferApi.list()`
 * is the truth.** The SSE channel drops frames for a client that falls behind and
 * replays nothing after a reconnect, so every gap in the stream — a reconnect, a
 * lag notice, a fresh mount after a reload — is followed by a reconcile. Without
 * that, a transfer that finished while the connection was down stays on screen as
 * "running" for the rest of the session, with a cancel button that does nothing.
 */

import { transferApi } from "./api/transfer";
import { subscribeTransfer } from "./transferEvents";
import { normalizeError } from "./errors";
import { refetchAllResources } from "./dataPoller";
import { osNotify } from "./osNotify";
import { t } from "./i18n.svelte";
import {
  settleJob,
  updateJob,
  reconcileJobs,
  notificationState,
} from "../store/notifications.svelte";

/**
 * What to call this transfer in a notification.
 *
 * Read before settling, because the entry may not survive: `settleJob` for an
 * unknown job invents one titled with the raw job id, which is not a name anybody
 * would recognise.
 */
function jobLabel(jobId: string): string | undefined {
  return notificationState.entries.find((e) => e.job?.jobId === jobId)?.title;
}

/** At most one reconcile in flight; later requests wait for it and re-ask after. */
let inFlight: Promise<void> | null = null;
let lastReconcileAt = 0;

/**
 * A connection that accepts and immediately drops would otherwise trigger a
 * reconcile per attempt — roughly one every two seconds once backoff resets on
 * each successful open.
 */
const MIN_RECONCILE_INTERVAL_MS = 2000;

/**
 * Ask the backend what transfers actually exist and correct the store.
 *
 * The snapshot is timestamped *before* the request goes out: the answer describes
 * the world as it was then, and a transfer started while it was in flight must not
 * be declared over by it.
 *
 * Failures are swallowed — this runs on a schedule the network chooses, the user
 * did not ask for it, and there is nothing they could do about it. The next gap
 * triggers another attempt.
 */
export async function reconcileTransfers(): Promise<void> {
  // Overlapping calls would interleave two views of the past. Sharing the one in
  // flight also collapses a burst of reconnects into a single request.
  if (inFlight) return inFlight;

  const asOf = Date.now();
  inFlight = (async () => {
    try {
      reconcileJobs(await transferApi.list(), asOf);
      lastReconcileAt = Date.now();
    } catch {
      // Backend unreachable. The stored view stays as it was rather than being
      // cleared — stale is better than empty, and a reconnect will follow.
    } finally {
      inFlight = null;
    }
  })();
  return inFlight;
}

/** Reconcile unless one just happened. Used for signals that can arrive in bursts. */
function reconcileThrottled(): void {
  if (Date.now() - lastReconcileAt < MIN_RECONCILE_INTERVAL_MS) return;
  void reconcileTransfers();
}

/**
 * Test seam: forget the throttle and any in-flight request.
 *
 * This module keeps its state at module scope because there is one event stream
 * per session, which means a test file's cases would otherwise inherit each
 * other's throttle window.
 */
export function _resetTransferNotificationsForTest(): void {
  inFlight = null;
  lastReconcileAt = 0;
}

/** Subscribe for the session. Returns the teardown. */
export function startTransferNotifications(): () => void {
  const unsubscribe = subscribeTransfer({
    onProgress: (e) => {
      updateJob(e.jobId, {
        bytes: e.bytes,
        totalEstimate: e.totalEstimate,
        message: e.message ?? undefined,
      });
    },
    onDone: (e) => {
      const label = jobLabel(e.jobId);
      settleJob(e.jobId, e.cancelled ? "cancelled" : "success", { bytes: e.bytes });
      // An import adds images; a copy-in changes nothing this app lists. Refreshing
      // after any completed transfer is simpler than tracking which kind changed
      // what, and it has to happen here: the dialog that used to do it on close now
      // closes at *start*, long before there is anything new to show.
      if (!e.cancelled) void refetchAllResources();
      // A cancellation is the user's own action, seconds old — telling them about
      // it through the operating system would be reporting their click back to them.
      if (!e.cancelled) {
        void osNotify(
          t("notifications.transfer_done", { default: "Transfer finished" }),
          label
        );
      }
    },
    onFailed: (e) => {
      const label = jobLabel(e.jobId);
      // The backend already redacted this text; `normalizeError` gives the panel
      // the same structured shape every other failure in the app has.
      settleJob(e.jobId, "error", {
        error: normalizeError(e.error),
        detail: e.error,
      });
      // Deliberately not `e.error`: see `osNotify` on why the runtime's text does
      // not leave the app. What went wrong is in the panel.
      void osNotify(
        t("notifications.transfer_failed", { default: "Transfer failed" }),
        label
      );
    },
    onDesync: reconcileThrottled,
  });

  return unsubscribe;
}

/**
 * Progress events for background transfers.
 *
 * Two transports because the app has two: the desktop shell delivers Tauri
 * events, browser mode delivers SSE. The backend publishes on both.
 *
 * This opens its own `EventSource` in browser mode rather than reusing the one in
 * `dataPoller`, which attaches a fixed set of listeners and exposes no bus. A
 * second SSE connection is a small cost; reaching into that module's connection
 * lifecycle from here would be a larger one. If a shared bus appears later, this
 * should move onto it.
 *
 * **Events are not the source of truth.** The SSE channel is deliberately lossy —
 * the server drops frames for a client that falls behind rather than buffering
 * without limit — and nothing is replayed after a reconnect. Anything built on this
 * must reconcile against `transferApi.list()` when `onDesync` fires, or it will
 * eventually show a transfer that finished long ago as still running.
 */

import { resolveApiBase, getApiToken } from "./api";
import { isRunningInTauri } from "./env";
import type { TransferProgress, TransferDone, TransferFailed } from "./api/transfer";

export interface TransferHandlers {
  onProgress?: (event: TransferProgress) => void;
  onDone?: (event: TransferDone) => void;
  onFailed?: (event: TransferFailed) => void;
  /**
   * The event stream cannot be trusted to be complete right now — reconcile.
   *
   * Fires on every (re)connect and whenever the server reports that it dropped
   * frames for this client. Events are the fast path; this is the signal that the
   * fast path has a hole in it and the caller should ask for the real state.
   */
  onDesync?: () => void;
}

const EVENTS = ["transfer.progress", "transfer.done", "transfer.failed"] as const;

/**
 * The server publishes this when a slow client falls behind and frames are
 * dropped. Shared name with the metrics stream, which uses it to draw a gap.
 */
const LAGGED_EVENT = "stream-lagged";

const INITIAL_RETRY_MS = 2000;
const MAX_RETRY_MS = 30000;

/**
 * Subscribe until the returned function is called.
 *
 * Handlers receive every job's events; callers filter by `jobId`, since several
 * transfers can be in flight at once.
 *
 * Subscribe **once**, at app level. This used to be called per dialog, which in
 * browser mode opened a fresh `EventSource` every time one was opened.
 */
export function subscribeTransfer(handlers: TransferHandlers): () => void {
  const dispatch = (name: string, payload: unknown) => {
    if (name === "transfer.progress") handlers.onProgress?.(payload as TransferProgress);
    else if (name === "transfer.done") handlers.onDone?.(payload as TransferDone);
    else if (name === "transfer.failed") handlers.onFailed?.(payload as TransferFailed);
  };

  if (isRunningInTauri()) {
    // Unlisten functions arrive asynchronously; collect them so a caller that
    // unsubscribes before they resolve still tears the listeners down.
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    import("@tauri-apps/api/event")
      .then(async (mod) => {
        for (const name of EVENTS) {
          const un = await mod.listen(name, (e: { payload: unknown }) =>
            dispatch(name, e.payload)
          );
          if (disposed) un();
          else unlisteners.push(un);
        }
        // No connection to lose here, but a fresh mount is still a gap: the
        // webview may have reloaded while a transfer was running, and Tauri
        // replays nothing that was emitted before these listeners existed.
        if (!disposed) handlers.onDesync?.();
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisteners.forEach((un) => un());
    };
  }

  let source: EventSource | null = null;
  let cancelled = false;
  let retryTimeout: ReturnType<typeof setTimeout> | null = null;
  let retryDelay = INITIAL_RETRY_MS;

  async function connect() {
    const token = await getApiToken();
    const base = await resolveApiBase();
    if (cancelled) return;
    const url = token ? `${base}/api/events?token=${token}` : `${base}/api/events`;
    source = new EventSource(url);

    for (const name of EVENTS) {
      source.addEventListener(name, (e: MessageEvent) => {
        try {
          dispatch(name, JSON.parse(e.data));
        } catch {
          // A malformed frame is not worth taking the dialog down for.
        }
      });
    }

    // The server drops frames for a client that falls behind rather than
    // buffering without limit, and says so. Missed frames are gone, so the only
    // correct response is to ask what the state actually is.
    source.addEventListener(LAGGED_EVENT, () => handlers.onDesync?.());

    source.onopen = () => {
      retryDelay = INITIAL_RETRY_MS;
      // Every reconnect starts with an unknown gap: anything that happened while
      // the connection was down was never delivered and never will be. This is
      // the callback that turns a dropped connection into a corrected view
      // instead of a transfer stuck at "running" forever.
      handlers.onDesync?.();
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
  };
}

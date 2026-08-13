import { call } from "./client";

// ===== Background transfers =====
//
// These four operations return a job id immediately and run in the background.
// Progress arrives as `transfer.progress` events and finishes with either
// `transfer.done` or `transfer.failed` — see `subscribeTransfer`.

/** Which of the four transfers a dialog is performing. */
export type TransferMode = "export" | "import" | "copy-in" | "copy-out";

export interface TransferStarted {
  jobId: string;
  /**
   * Approximate byte total, when one is knowable. Docker reports no total for
   * these operations, so this is derived from image metadata; treat it as an
   * estimate and never as a completion contract.
   */
  totalEstimate: number | null;
}

export interface TransferProgress {
  jobId: string;
  bytes: number;
  totalEstimate: number | null;
  /** A line the runtime printed, for operations that report text instead of size. */
  message: string | null;
}

export interface TransferDone {
  jobId: string;
  bytes: number;
  cancelled: boolean;
}

export interface TransferFailed {
  jobId: string;
  error: string;
}

/** What a cancel request actually did. See `transferApi.cancel`. */
export type CancelOutcome = "cancelled" | "alreadyFinished" | "unknownJob";

export type TransferStatus = "starting" | "running" | "success" | "failed" | "cancelled";

/**
 * A transfer as the backend currently sees it.
 *
 * `targetLabel` is a name — an image reference, a path inside a container — never a
 * host path: the destination is something the user chose and already knows, and
 * keeping it out of this payload means the list needs no redaction pass.
 */
export interface TransferSnapshot {
  jobId: string;
  /** `save` | `load` | `cp-in` | `cp-out`. */
  kind: string;
  status: TransferStatus;
  bytes: number;
  totalEstimate: number | null;
  startedAt: number;
  finishedAt: number | null;
  targetLabel: string;
  /** Redacted failure text, when the job failed. */
  error: string | null;
}

export const transferApi = {
  /**
   * Export images to a TAR archive.
   *
   * `destDir` and `fileName` stay separate all the way to the backend: the write
   * is confined to the folder the user chose, which a single joined path could not
   * express.
   */
  saveImages: (images: string[], destDir: string, fileName: string, overwrite = false) =>
    call<TransferStarted>(
      "image_save",
      { images, destDir, fileName, overwrite },
      "POST",
      "/api/images/save",
      undefined,
      { images, destDir, fileName, overwrite }
    ),

  loadImages: (tarPath: string) =>
    call<TransferStarted>("image_load", { tarPath }, "POST", "/api/images/load", undefined, {
      tarPath,
    }),

  copyToContainer: (containerId: string, hostPath: string, containerPath: string) =>
    call<TransferStarted>(
      "copy_to_container",
      { containerId, hostPath, containerPath },
      "POST",
      "/api/containers/cp/to",
      undefined,
      { containerId, hostPath, containerPath }
    ),

  copyFromContainer: (
    containerId: string,
    containerPath: string,
    destDir: string,
    fileName: string,
    overwrite = false
  ) =>
    call<TransferStarted>(
      "copy_from_container",
      { containerId, containerPath, destDir, fileName, overwrite },
      "POST",
      "/api/containers/cp/from",
      undefined,
      { containerId, containerPath, destDir, fileName, overwrite }
    ),

  /**
   * Ask a transfer to stop.
   *
   * The outcome is not a boolean because the two ways it can fail need different
   * handling: `alreadyFinished` should settle the entry, `unknownJob` means the
   * client's view is stale and should be reconciled against `list()`. A `false`
   * covered both and left the UI showing "cancelling…" forever.
   *
   * `cancelled` only means the request landed — the terminal state still arrives
   * as a `transfer.done` event carrying `cancelled: true`.
   */
  cancel: (jobId: string) =>
    call<CancelOutcome>("cancel_transfer", { jobId }, "POST", "/api/transfers/cancel", undefined, {
      jobId,
    }),

  /**
   * Every transfer this backend knows about.
   *
   * Events are the fast path and are lossy — the SSE channel drops frames under
   * lag by design — so this is the source of truth after a reconnect or a reload.
   * Finished jobs stay listed for about a minute, which is what lets a client that
   * missed the terminal event still learn how the job ended.
   */
  list: () => call<TransferSnapshot[]>("transfer_list", {}, "GET", "/api/transfers"),
};

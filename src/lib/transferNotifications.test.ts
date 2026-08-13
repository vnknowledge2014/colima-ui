import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import type { TransferHandlers } from "./transferEvents";
import type { TransferSnapshot } from "./api/transfer";
import {
  notificationState,
  startJob,
  _resetNotificationsForTest,
} from "../store/notifications.svelte";

/**
 * The rule this layer exists to enforce: **events are the fast path, the list is
 * the truth.** The SSE channel drops frames for a slow client and replays nothing
 * after a reconnect, so anything that only listened would eventually show a
 * transfer that finished long ago as still running, with a cancel that does
 * nothing.
 */

const { transferApi, handlers } = vi.hoisted(() => ({
  transferApi: { list: vi.fn() },
  handlers: { current: null as TransferHandlers | null },
}));

vi.mock("./api/transfer", async () => {
  const actual = await vi.importActual<typeof import("./api/transfer")>("./api/transfer");
  return { ...actual, transferApi };
});
vi.mock("./dataPoller", () => ({ refetchAllResources: vi.fn() }));
vi.mock("./osNotify", () => ({ osNotify: vi.fn() }));
vi.mock("./transferEvents", () => ({
  subscribeTransfer: (h: TransferHandlers) => {
    handlers.current = h;
    return () => {
      handlers.current = null;
    };
  },
}));

import {
  startTransferNotifications,
  _resetTransferNotificationsForTest,
} from "./transferNotifications";

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

let stop: (() => void) | null = null;

beforeEach(() => {
  _resetNotificationsForTest();
  _resetTransferNotificationsForTest();
  transferApi.list.mockResolvedValue([]);
  stop = startTransferNotifications();
});

afterEach(() => {
  stop?.();
  vi.clearAllMocks();
});

describe("events", () => {
  it("does not list one transfer twice when it fails before it starts", () => {
    // The backend spawns before returning the id, so the failure can arrive first.
    handlers.current?.onFailed?.({ jobId: "save-1", error: "boom" });
    startJob({ jobId: "save-1", title: "alpine:3.19", totalEstimate: null });

    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].status).toBe("error");
  });

  it("refreshes the resource lists once a transfer actually completes", async () => {
    const { refetchAllResources } = await import("./dataPoller");
    startJob({ jobId: "load-1", title: "archive.tar", totalEstimate: null });
    handlers.current?.onDone?.({ jobId: "load-1", bytes: 10, cancelled: false });

    // The dialog used to do this on close; it now closes at start, before an
    // import has added anything.
    expect(refetchAllResources).toHaveBeenCalled();
  });

  it("routes progress to the job it belongs to", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    startJob({ jobId: "save-2", title: "b", totalEstimate: null });

    handlers.current?.onProgress?.({
      jobId: "save-2",
      bytes: 99,
      totalEstimate: 200,
      message: null,
    });

    const entries = notificationState.entries;
    expect(entries.find((e) => e.job?.jobId === "save-2")?.job?.bytes).toBe(99);
    expect(entries.find((e) => e.job?.jobId === "save-1")?.job?.bytes).toBe(0);
  });

  it("distinguishes a cancellation from a completion", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    handlers.current?.onDone?.({ jobId: "save-1", bytes: 0, cancelled: true });

    // The user asked for this, so it is not a failure — and not a success either.
    expect(notificationState.entries[0].status).toBe("cancelled");
  });

  it("never sends the runtime's error text to the operating system", async () => {
    const { osNotify } = await import("./osNotify");
    startJob({ jobId: "save-1", title: "alpine:3.19", totalEstimate: null });
    handlers.current?.onFailed?.({
      jobId: "save-1",
      error: "Cannot create /Volumes/backup-2024/img.tar: Permission denied",
    });

    // `redact` masks home-directory account segments and known secret shapes, not
    // arbitrary absolute paths — and an OS notification lands in a system-wide
    // centre with its own history. Only the transfer's name leaves the app.
    expect(osNotify).toHaveBeenCalledWith(expect.any(String), "alpine:3.19");
    const args = vi.mocked(osNotify).mock.calls[0].join(" ");
    expect(args).not.toContain("/Volumes");
    expect(args).not.toContain("Permission denied");
  });

  it("does not report a cancellation back through the operating system", async () => {
    const { osNotify } = await import("./osNotify");
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    handlers.current?.onDone?.({ jobId: "save-1", bytes: 0, cancelled: true });

    // The user pressed cancel seconds ago; telling them about it is telling them
    // what they just did.
    expect(osNotify).not.toHaveBeenCalled();
  });

  it("gives a failure the structured shape the details panel needs", () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    handlers.current?.onFailed?.({ jobId: "save-1", error: "docker: no space left" });

    const entry = notificationState.entries[0];
    expect(entry.status).toBe("error");
    expect(entry.error, "a failed transfer must be readable like any other error")
      .toBeDefined();
    expect(entry.detail).toContain("no space left");
  });
});

describe("reconciling", () => {
  it("asks the backend for the truth whenever the stream has a gap", async () => {
    transferApi.list.mockResolvedValue([snapshot({ status: "success", bytes: 10 })]);
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });

    handlers.current?.onDesync?.();
    await vi.waitFor(() =>
      expect(notificationState.entries[0].status).toBe("success")
    );
  });

  it("adopts a transfer started before this page existed", async () => {
    // A reload does not stop the transfer, and Tauri replays nothing — so the
    // only way the job reappears is by asking.
    transferApi.list.mockResolvedValue([snapshot({ jobId: "cp-out-7" })]);

    handlers.current?.onDesync?.();
    await vi.waitFor(() =>
      expect(notificationState.entries[0].job?.jobId).toBe("cp-out-7")
    );
  });

  it("does not end a transfer that started after the question was asked", async () => {
    // The snapshot describes the world when the request went out. A job created
    // while it was in flight is simply not in it — and ending that live transfer
    // would leave the user with a cancel button for a job still writing to disk.
    let release: (v: TransferSnapshot[]) => void = () => {};
    transferApi.list.mockReturnValue(
      new Promise<TransferSnapshot[]>((r) => {
        release = r;
      })
    );

    handlers.current?.onDesync?.();
    startJob({ jobId: "save-late", title: "started mid-flight", totalEstimate: null });
    release([]);

    await vi.waitFor(() => expect(transferApi.list).toHaveBeenCalled());
    expect(notificationState.entries[0].status).toBe("running");
  });

  it("collapses a burst of reconnects into one request", async () => {
    handlers.current?.onDesync?.();
    handlers.current?.onDesync?.();
    handlers.current?.onDesync?.();

    await vi.waitFor(() => expect(transferApi.list).toHaveBeenCalled());
    // A proxy that accepts then drops would otherwise reconcile on every attempt.
    expect(transferApi.list).toHaveBeenCalledTimes(1);
  });

  it("keeps the stored view when the backend cannot be reached", async () => {
    startJob({ jobId: "save-1", title: "a", totalEstimate: null });
    transferApi.list.mockRejectedValue(new Error("connection refused"));

    handlers.current?.onDesync?.();
    await vi.waitFor(() => expect(transferApi.list).toHaveBeenCalled());

    // Stale beats empty: the user did not ask for this call and can do nothing
    // about its failure, and a reconnect will follow.
    expect(notificationState.entries).toHaveLength(1);
    expect(notificationState.entries[0].status).toBe("running");
  });
});

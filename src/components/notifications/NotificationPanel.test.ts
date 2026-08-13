import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent, waitFor, within } from "@testing-library/svelte";
import {
  notificationState,
  openNotificationPanel,
  startJob,
  updateJob,
  settleJob,
  pushNotification,
  unreadCount,
  _resetNotificationsForTest,
  type AnnouncementMeta,
} from "../../store/notifications.svelte";

/**
 * The panel is where a background transfer becomes visible and controllable again
 * after the dialog closed. What matters: a running job is reachable and cancellable
 * from anywhere, and nothing the user can still act on gets cleared away.
 */

const { transferApi } = vi.hoisted(() => ({
  transferApi: { cancel: vi.fn(), list: vi.fn() },
}));

vi.mock("../../lib/api/transfer", async () => {
  const actual = await vi.importActual<typeof import("../../lib/api/transfer")>(
    "../../lib/api/transfer"
  );
  return { ...actual, transferApi };
});
vi.mock("../../lib/transferNotifications", () => ({
  reconcileTransfers: vi.fn(),
}));

import NotificationPanel from "./NotificationPanel.svelte";

beforeEach(() => {
  _resetNotificationsForTest();
  transferApi.cancel.mockResolvedValue("cancelled");
  transferApi.list.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  // `reset`, not `clear`: one test replaces `reconcileTransfers`'s implementation,
  // and `clearAllMocks` only forgets the calls — the stub would leak into every
  // test after it.
  vi.resetAllMocks();
});

describe("NotificationPanel", () => {
  it("stays out of the way until it is opened", () => {
    pushNotification({ title: "something happened" });
    render(NotificationPanel);

    expect(screen.queryByText("something happened")).not.toBeInTheDocument();
  });

  it("shows a running transfer with its progress", async () => {
    startJob({ jobId: "save-1", title: "alpine:3.19", totalEstimate: 1000 });
    updateJob("save-1", { bytes: 250 });
    openNotificationPanel();
    render(NotificationPanel);

    expect(screen.getByText("alpine:3.19")).toBeInTheDocument();
    // A known total means a real percentage rather than an indeterminate bar.
    expect(screen.getByText(/25%/)).toBeInTheDocument();
  });

  it("cancels the job the row belongs to", async () => {
    startJob({ jobId: "save-1", title: "first", totalEstimate: null });
    startJob({ jobId: "save-2", title: "second", totalEstimate: null });
    openNotificationPanel();
    render(NotificationPanel);

    const rows = screen.getAllByRole("listitem");
    const secondRow = rows.find((r) => r.textContent?.includes("second"))!;
    await fireEvent.click(
      within(secondRow).getByRole("button", { name: /cancel/i })
    );

    await waitFor(() => expect(transferApi.cancel).toHaveBeenCalledWith("save-2"));
  });

  it("does not call a finished job a success just because cancelling was too late", async () => {
    // `alreadyFinished` is returned for *any* terminal state, failures included.
    // Guessing "success" here would put a green tick on a failed export. The
    // backend still lists it, so the real outcome is one reconcile away.
    const { reconcileTransfers } = await import("../../lib/transferNotifications");
    vi.mocked(reconcileTransfers).mockImplementation(async () => {
      settleJob("save-1", "error", { detail: "no space left on device" });
    });
    transferApi.cancel.mockResolvedValue("alreadyFinished");

    startJob({ jobId: "save-1", title: "late", totalEstimate: null });
    openNotificationPanel();
    render(NotificationPanel);

    await fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));

    await waitFor(() => expect(notificationState.entries[0].status).toBe("error"));
  });

  it("admits it does not know when the job aged out of the backend", async () => {
    // Retention is about a minute. Past that the outcome is genuinely unknowable
    // from here — but the row must still stop spinning.
    transferApi.cancel.mockResolvedValue("unknownJob");
    startJob({ jobId: "save-1", title: "long gone", totalEstimate: null });
    openNotificationPanel();
    render(NotificationPanel);

    await fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));

    await waitFor(() => expect(notificationState.entries[0].status).toBe("ended"));
  });

  it("puts the row back when the cancel request itself fails", async () => {
    // Nothing was cancelled, so no event is coming. Leaving it disabled at
    // "Cancelling…" would strand a transfer that is still running.
    transferApi.cancel.mockRejectedValue(new Error("connection refused"));
    startJob({ jobId: "save-1", title: "still running", totalEstimate: null });
    openNotificationPanel();
    render(NotificationPanel);

    await fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));

    // By id, not by position: the failure itself is reported as a notification,
    // which puts a message entry above the job.
    await waitFor(() => {
      const job = notificationState.entries.find((e) => e.job?.jobId === "save-1");
      expect(job?.status).toBe("running");
    });
  });

  it("keeps a running transfer when finished entries are cleared", async () => {
    startJob({ jobId: "save-1", title: "still going", totalEstimate: null });
    pushNotification({ title: "old news" });
    openNotificationPanel();
    render(NotificationPanel);

    await fireEvent.click(screen.getByRole("button", { name: /clear/i }));

    // Clearing must not strand a transfer the user can still cancel.
    await waitFor(() => expect(screen.getByText("still going")).toBeInTheDocument());
    expect(screen.queryByText("old news")).not.toBeInTheDocument();
  });

  it("puts a running transfer above newer chatter", async () => {
    startJob({ jobId: "save-1", title: "the transfer", totalEstimate: null });
    pushNotification({ title: "later message" });
    openNotificationPanel();
    render(NotificationPanel);

    const titles = screen.getAllByRole("listitem").map((li) => li.textContent);
    expect(titles[0]).toContain("the transfer");
  });

  it("closes on Escape", async () => {
    openNotificationPanel();
    render(NotificationPanel);

    await fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(notificationState.panelOpen).toBe(false));
  });

  it("clears the badge when opened", () => {
    pushNotification({ title: "one" });
    pushNotification({ title: "two" });
    expect(unreadCount()).toBe(2);

    // Opening is reading.
    openNotificationPanel();
    expect(unreadCount()).toBe(0);
  });

  /**
   * Announcement text and links come from a file on the network — the only
   * content in this panel that does. Both are rendered on the assumption that
   * the feed could be hostile.
   */
  describe("announcements", () => {
    function announce(over: Partial<AnnouncementMeta> = {}, title = "Advisory") {
      pushNotification({
        kind: "announcement",
        title,
        detail: "<script>alert(1)</script>",
        announcement: { announcementId: "a1", severity: "critical", ...over },
      });
    }

    it("renders feed text as text, never as markup", () => {
      announce();
      openNotificationPanel();
      const { container } = render(NotificationPanel);

      expect(screen.getByText("<script>alert(1)</script>")).toBeInTheDocument();
      expect(container.querySelector("script")).toBeNull();
    });

    it("offers a link on an allowed host", () => {
      announce({ linkUrl: "https://github.com/vnknowledge2014/colima-ui/releases" });
      openNotificationPanel();
      render(NotificationPanel);

      expect(screen.getByRole("button", { name: /learn more/i })).toBeInTheDocument();
    });

    it("marks the link as leaving the app", () => {
      // ↗ means "this opens a browser" everywhere in this app. This button
      // really does, and used to be the only one that left without saying so —
      // while the Pro gate's button wore the arrow and only opened a dialog.
      // Asserted on the accessible name rather than the DOM, because the name
      // is what the promise is made in.
      announce({ linkUrl: "https://github.com/vnknowledge2014/colima-ui/releases" });
      openNotificationPanel();
      render(NotificationPanel);

      expect(screen.getByRole("button", { name: /learn more/i }).textContent).toContain("↗");
    });

    it("does not offer a link the app would refuse to open", () => {
      // Not rendered-then-blocked: a button that does nothing when clicked is a
      // worse answer than no button.
      announce({ linkUrl: "javascript:alert(1)" });
      openNotificationPanel();
      render(NotificationPanel);

      expect(screen.queryByRole("button", { name: /learn more/i })).not.toBeInTheDocument();
    });

    it("does not offer a link to a host outside the allowlist", () => {
      announce({ linkUrl: "https://evil.test/advisory" });
      openNotificationPanel();
      render(NotificationPanel);

      expect(screen.queryByRole("button", { name: /learn more/i })).not.toBeInTheDocument();
    });
  });

  it("shows an empty inbox rather than a bare sentence", () => {
    openNotificationPanel();
    const { container } = render(NotificationPanel);

    expect(screen.getByText(/inbox zero/i)).toBeInTheDocument();
    expect(screen.getByText(/show up here/i)).toBeInTheDocument();
    // The illustration is what makes it read as a place with nothing in it.
    expect(container.querySelector(".empty-icon svg")).not.toBeNull();
  });
});

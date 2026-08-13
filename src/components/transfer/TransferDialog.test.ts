import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, waitFor, fireEvent } from "@testing-library/svelte";
import {
  notificationState,
  _resetNotificationsForTest,
} from "../../store/notifications.svelte";

/**
 * What matters here is the gate and the handover.
 *
 * The gate: Start stays unavailable until the fields name a real operation, and a
 * rejected start is readable in place — the backend never sees the attempts this
 * prevents, and a validation message is useless once the fields are gone.
 *
 * The handover: a started transfer moves to the notification list and this dialog
 * closes. It no longer follows the job, so there is nothing here about progress or
 * cancelling — those belong to the store and its panel.
 */

const { transferApi } = vi.hoisted(() => ({
  transferApi: {
    saveImages: vi.fn(),
    loadImages: vi.fn(),
    copyToContainer: vi.fn(),
    copyFromContainer: vi.fn(),
    cancel: vi.fn(),
    list: vi.fn(),
  },
}));

vi.mock("../../lib/api/transfer", async () => {
  const actual = await vi.importActual<typeof import("../../lib/api/transfer")>(
    "../../lib/api/transfer"
  );
  return { ...actual, transferApi };
});
// The picker is a desktop affordance; these tests drive the typed-path path.
vi.mock("../../lib/env", () => ({ isRunningInTauri: () => false }));

import TransferDialog from "./TransferDialog.svelte";

beforeEach(() => {
  _resetNotificationsForTest();
  transferApi.saveImages.mockResolvedValue({ jobId: "save-1", totalEstimate: 1024 });
  transferApi.loadImages.mockResolvedValue({ jobId: "load-1", totalEstimate: null });
  transferApi.copyToContainer.mockResolvedValue({ jobId: "cp-1", totalEstimate: null });
  transferApi.copyFromContainer.mockResolvedValue({ jobId: "cp-out-1", totalEstimate: null });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const startButton = () => screen.getByRole("button", { name: /start in background/i });
const typeInto = (placeholder: RegExp | string, value: string) =>
  fireEvent.input(screen.getByPlaceholderText(placeholder), { target: { value } });

describe("TransferDialog", () => {
  it("keeps Start unavailable until an export names a folder and a file", async () => {
    render(TransferDialog, {
      props: { mode: "export", images: ["alpine:3.19"], onClose: () => {} },
    });

    // The file name is prefilled for export, the folder is not.
    expect(startButton()).toBeDisabled();
    await typeInto(/Downloads/, "/tmp/out");
    await waitFor(() => expect(startButton()).toBeEnabled());
  });

  it("refuses to start an export with nothing selected", async () => {
    render(TransferDialog, { props: { mode: "export", images: [], onClose: () => {} } });
    await typeInto(/Downloads/, "/tmp/out");
    // No images: there is nothing to write, so the button stays shut.
    expect(startButton()).toBeDisabled();
    expect(transferApi.saveImages).not.toHaveBeenCalled();
  });

  it("requires both sides of a copy into a container", async () => {
    render(TransferDialog, {
      props: { mode: "copy-in", containerId: "abc123", onClose: () => {} },
    });
    expect(startButton()).toBeDisabled();

    await typeInto("/path/to/file", "/etc/hosts");
    expect(startButton()).toBeDisabled(); // container path still missing

    await typeInto("/app/data.json", "/tmp/hosts");
    await waitFor(() => expect(startButton()).toBeEnabled());

    await fireEvent.click(startButton());
    await waitFor(() =>
      expect(transferApi.copyToContainer).toHaveBeenCalledWith(
        "abc123",
        "/etc/hosts",
        "/tmp/hosts"
      )
    );
  });

  it("passes the folder and the file name separately, as the confinement needs", async () => {
    render(TransferDialog, {
      props: { mode: "export", images: ["alpine:3.19"], onClose: () => {} },
    });
    await typeInto(/Downloads/, "/tmp/out");
    await fireEvent.click(startButton());

    await waitFor(() => expect(transferApi.saveImages).toHaveBeenCalledTimes(1));
    // A single joined path could not express "confine the write to this folder".
    expect(transferApi.saveImages).toHaveBeenCalledWith(
      ["alpine:3.19"],
      "/tmp/out",
      "images.tar",
      false
    );
  });

  it("hands the transfer to the notification list and closes", async () => {
    const onClose = vi.fn();
    render(TransferDialog, {
      props: { mode: "export", images: ["alpine:3.19"], onClose },
    });
    await typeInto(/Downloads/, "/tmp/out");
    await fireEvent.click(startButton());

    // Closing is the point: a multi-gigabyte export used to hold the page behind
    // this modal for its whole duration.
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));

    const entry = notificationState.entries.find((e) => e.job?.jobId === "save-1");
    expect(entry, "the transfer must survive the dialog that started it").toBeDefined();
    expect(entry?.status).toBe("running");
    expect(entry?.job?.totalEstimate).toBe(1024);
  });

  it("shows a rejected start in place and stays open", async () => {
    transferApi.loadImages.mockRejectedValue(new Error("Archive is not a file: /nope"));
    const onClose = vi.fn();
    render(TransferDialog, { props: { mode: "import", onClose } });

    await typeInto(/images\.tar/, "/nope");
    await fireEvent.click(startButton());

    // A validation failure is something to correct in the fields still on screen.
    // Closing first would make the user reopen the dialog and retype everything.
    await waitFor(() => expect(screen.getByText(/not a file/)).toBeInTheDocument());
    expect(onClose).not.toHaveBeenCalled();
    expect(notificationState.entries).toHaveLength(0);
  });

  it("adds the .tar the backend requires when the name was typed without it", async () => {
    render(TransferDialog, {
      props: { mode: "export", images: ["alpine:3.19"], onClose: () => {} },
    });
    await typeInto(/Downloads/, "/tmp/out");
    await typeInto("images.tar", "backup");
    await fireEvent.click(startButton());

    await waitFor(() =>
      expect(transferApi.saveImages).toHaveBeenCalledWith(
        ["alpine:3.19"],
        "/tmp/out",
        "backup.tar",
        false
      )
    );
  });

  it("leaves a compressed name alone so the backend can explain it", async () => {
    render(TransferDialog, {
      props: { mode: "export", images: ["alpine:3.19"], onClose: () => {} },
    });
    await typeInto(/Downloads/, "/tmp/out");
    await typeInto("images.tar", "backup.tar.gz");
    await fireEvent.click(startButton());

    // Turning this into `backup.tar.gz.tar` would hide the real problem: these
    // commands write an uncompressed archive, and the backend says so plainly.
    await waitFor(() =>
      expect(transferApi.saveImages).toHaveBeenCalledWith(
        ["alpine:3.19"],
        "/tmp/out",
        "backup.tar.gz",
        false
      )
    );
  });

  it("tells the user that copying out yields an archive", async () => {
    render(TransferDialog, {
      props: { mode: "copy-out", containerId: "abc123", onClose: () => {} },
    });

    // A single file also comes back as a .tar. Leaving that to be discovered makes
    // a correct result look like a bug.
    expect(screen.getByText(/\.tar archive/i)).toBeInTheDocument();
  });
});

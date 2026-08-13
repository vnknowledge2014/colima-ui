import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, waitFor, fireEvent } from "@testing-library/svelte";

/**
 * The privacy-critical behaviour of this dialog is that unchecking a section
 * actually removes it from what leaves the app. A checkbox that looks like it
 * excludes something but does not would be worse than having no checkbox at all,
 * and it is invisible to any backend test — the selection lives here.
 *
 * These mount the real component and click it. That is not a substitute for a
 * human looking at the window, but it is the half of "clicking" that carries
 * consequences.
 */

const bundle = {
  sections: [
    { id: "app", title: "Application", content: "ColimaUI 1.0.0", includedByDefault: true },
    { id: "logs", title: "Logs — abc", content: "SECRET-LOG-LINE", includedByDefault: false },
  ],
  signature: "some failure",
  appVersion: "1.0.0",
  truncatedBytes: 0,
};

// `vi.hoisted` because `vi.mock` is lifted above the file's own declarations:
// a plain const would not exist yet when the factory runs.
const { diagnosticsApi, openExternal } = vi.hoisted(() => ({
  diagnosticsApi: { bundle: vi.fn(), save: vi.fn() },
  openExternal: vi.fn(),
}));

vi.mock("../../lib/api/diagnostics", async () => {
  const actual = await vi.importActual<typeof import("../../lib/api/diagnostics")>(
    "../../lib/api/diagnostics"
  );
  return { ...actual, diagnosticsApi };
});

vi.mock("../../lib/external-links", async () => {
  const actual = await vi.importActual<typeof import("../../lib/external-links")>(
    "../../lib/external-links"
  );
  return { ...actual, openExternal };
});

import BundlePreview from "./BundlePreview.svelte";

let clipboard: string[] = [];

beforeEach(() => {
  clipboard = [];
  diagnosticsApi.bundle.mockResolvedValue(structuredClone(bundle));
  diagnosticsApi.save.mockResolvedValue("/tmp/report.md");
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: (t: string) => { clipboard.push(t); return Promise.resolve(); } },
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const mount = () => render(BundlePreview, { props: { onClose: () => {} } });

describe("BundlePreview", () => {
  it("lists every collected section once the bundle arrives", async () => {
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());
    expect(screen.getByText("Logs — abc")).toBeInTheDocument();
    // The signature is what groups duplicate reports, so it has to be visible.
    expect(screen.getByText("some failure")).toBeInTheDocument();
  });

  it("leaves the log section unchecked and excludes it from the copied report", async () => {
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());

    const boxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    expect(boxes[0].checked).toBe(true); // app
    expect(boxes[1].checked).toBe(false); // logs — opt-in

    await fireEvent.click(screen.getByRole("button", { name: /copy report/i }));
    await waitFor(() => expect(clipboard).toHaveLength(1));
    expect(clipboard[0]).toContain("ColimaUI 1.0.0");
    expect(clipboard[0]).not.toContain("SECRET-LOG-LINE");
  });

  it("includes a section the user explicitly checks", async () => {
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());

    const boxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    await fireEvent.click(boxes[1]);
    await fireEvent.click(screen.getByRole("button", { name: /copy report/i }));

    await waitFor(() => expect(clipboard).toHaveLength(1));
    expect(clipboard[0]).toContain("SECRET-LOG-LINE");
  });

  it("drops a section the user unchecks", async () => {
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());

    const boxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    await fireEvent.click(boxes[0]); // uncheck the one that was on
    await fireEvent.click(screen.getByRole("button", { name: /copy report/i }));

    await waitFor(() => expect(clipboard).toHaveLength(1));
    expect(clipboard[0]).not.toContain("ColimaUI 1.0.0");
  });

  it("shows a section's full content when expanded, not a sample", async () => {
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());

    // Hidden until asked for: the log body is the largest and most sensitive part.
    expect(screen.queryByText("SECRET-LOG-LINE")).not.toBeInTheDocument();
    const showButtons = screen.getAllByRole("button", { name: /^show$/i });
    await fireEvent.click(showButtons[1]);
    expect(screen.getByText("SECRET-LOG-LINE")).toBeInTheDocument();
  });

  it("saves only the checked sections", async () => {
    vi.stubGlobal("prompt", () => "/tmp/reports");
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());

    await fireEvent.click(screen.getByRole("button", { name: /save \.md/i }));
    await waitFor(() => expect(diagnosticsApi.save).toHaveBeenCalledTimes(1));

    const [, include, destDir, fileName] = diagnosticsApi.save.mock.calls[0];
    expect(include).toEqual(["app"]);
    expect(destDir).toBe("/tmp/reports");
    expect(fileName).toMatch(/^colimaui-diagnostics-.*\.md$/);
    vi.unstubAllGlobals();
  });

  it("opens a prefilled issue without embedding the bundle", async () => {
    mount();
    await waitFor(() => expect(screen.getByText("Application")).toBeInTheDocument());

    await fireEvent.click(screen.getByRole("button", { name: /github issue/i }));
    expect(openExternal).toHaveBeenCalledTimes(1);

    const url = new URL(openExternal.mock.calls[0][0] as string);
    expect(url.searchParams.get("title")).toContain("some failure");
    // The body asks the user to paste; embedding the report would exceed what
    // GitHub accepts in a query string.
    expect(url.searchParams.get("body")).not.toContain("ColimaUI 1.0.0");
    expect(url.toString().length).toBeLessThan(8000);
  });

  it("reports a failed collection instead of rendering an empty dialog", async () => {
    diagnosticsApi.bundle.mockRejectedValue(new Error("daemon is down"));
    mount();
    await waitFor(() => expect(screen.getByText(/daemon is down/)).toBeInTheDocument());
  });
});

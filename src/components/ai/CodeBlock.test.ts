import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import CodeBlock from "./CodeBlock.svelte";
import { dashboardState, openTerminalSession } from "../../store.svelte";
import { globalToast } from "../../lib/globalToast";

vi.mock("../../lib/globalToast", () => ({
  globalToast: vi.fn(),
}));

vi.mock("../../store.svelte", async () => {
  const actual = await vi.importActual<typeof import("../../store.svelte")>("../../store.svelte");
  return {
    ...actual,
    openTerminalSession: vi.fn(),
  };
});

describe("CodeBlock.svelte", () => {
  const originalClipboard = navigator.clipboard;
  let mockWriteText: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockWriteText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: mockWriteText },
      writable: true,
      configurable: true,
    });
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
    Object.defineProperty(navigator, "clipboard", {
      value: originalClipboard,
      writable: true,
      configurable: true,
    });
  });

  it("renders code content and language pill", () => {
    render(CodeBlock, { code: '{"status": "ok"}', language: "json" });

    expect(screen.getByText('{"status": "ok"}')).toBeInTheDocument();
    expect(screen.getByText("json")).toBeInTheDocument();
  });

  it("defaults language to text for non-shell code without language", () => {
    render(CodeBlock, { code: "hello world", isShell: false });

    expect(screen.getByText("text")).toBeInTheDocument();
    expect(screen.queryByText("shell")).not.toBeInTheDocument();
  });

  it("displays shell badge and defaults language to shell when isShell is true", () => {
    render(CodeBlock, { code: "docker ps", isShell: true });

    expect(screen.getAllByText("shell").length).toBeGreaterThanOrEqual(1);
  });

  it("copies code to clipboard and shows feedback", async () => {
    render(CodeBlock, { code: "colima start", language: "bash" });

    const copyBtn = screen.getByRole("button", { name: /copy/i });
    await fireEvent.click(copyBtn);

    expect(mockWriteText).toHaveBeenCalledWith("colima start");
    expect(screen.getByText("Copied!")).toBeInTheDocument();
  });

  it("handles clipboard write failure gracefully with toast", async () => {
    mockWriteText.mockRejectedValueOnce(new Error("Permission denied"));
    render(CodeBlock, { code: "colima status", language: "bash" });

    const copyBtn = screen.getByRole("button", { name: /copy/i });
    await fireEvent.click(copyBtn);

    expect(globalToast).toHaveBeenCalledWith("error", expect.any(String));
  });

  it("renders insert button when onInsert is provided and fires callback on click", async () => {
    const onInsert = vi.fn();
    render(CodeBlock, { code: "kubectl get pods", onInsert });

    const insertBtn = screen.getByRole("button", { name: /insert/i });
    expect(insertBtn).toBeInTheDocument();

    await fireEvent.click(insertBtn);
    expect(onInsert).toHaveBeenCalledWith("kubectl get pods");
  });

  it("omits insert button when onInsert is not provided", () => {
    render(CodeBlock, { code: "echo 123" });

    expect(screen.queryByRole("button", { name: /insert/i })).not.toBeInTheDocument();
  });

  it("renders Open Terminal button only when isShell is true", () => {
    const { unmount } = render(CodeBlock, { code: "echo 1", isShell: false });
    expect(screen.queryByRole("button", { name: /terminal/i })).not.toBeInTheDocument();
    unmount();

    render(CodeBlock, { code: "echo 1", isShell: true });
    expect(screen.getByRole("button", { name: /terminal/i })).toBeInTheDocument();
  });

  it("opens terminal session, copies command, and shows non-persistent toast without auto-execution", async () => {
    dashboardState.colimaInstances = [
      { name: "dev-k8s", status: "Running" } as never,
    ];

    render(CodeBlock, { code: "colima restart dev-k8s", isShell: true });

    const termBtn = screen.getByRole("button", { name: /terminal/i });
    await fireEvent.click(termBtn);

    // 1. Command copied to clipboard
    expect(mockWriteText).toHaveBeenCalledWith("colima restart dev-k8s");

    // 2. Non-persistent toast displayed
    expect(globalToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("Command copied"),
      { record: false }
    );

    // 3. Terminal session target resolved to active running profile
    expect(openTerminalSession).toHaveBeenCalledWith({
      kind: "colima",
      profile: "dev-k8s",
    });
  });

  it("normalizes 'colima' instance name to 'default' profile when opening terminal", async () => {
    dashboardState.colimaInstances = [
      { name: "colima", status: "running" } as never,
    ];

    render(CodeBlock, { code: "docker ps", isShell: true });

    const termBtn = screen.getByRole("button", { name: /terminal/i });
    await fireEvent.click(termBtn);

    expect(openTerminalSession).toHaveBeenCalledWith({
      kind: "colima",
      profile: "default",
    });
  });

  it("falls back to 'default' profile when no instances are running", async () => {
    dashboardState.colimaInstances = [
      { name: "staging", status: "stopped" } as never,
    ];

    render(CodeBlock, { code: "colima start", isShell: true });

    const termBtn = screen.getByRole("button", { name: /terminal/i });
    await fireEvent.click(termBtn);

    expect(openTerminalSession).toHaveBeenCalledWith({
      kind: "colima",
      profile: "default",
    });
  });
});

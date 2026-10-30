import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import ToolStepCard from "./ToolStepCard.svelte";
import type { ToolStepPayload } from "../../lib/aiToolStep";

afterEach(cleanup);

describe("ToolStepCard.svelte", () => {
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
    Object.defineProperty(navigator, "clipboard", {
      value: originalClipboard,
      writable: true,
      configurable: true,
    });
  });

  it("renders pending approval card with safety badge, warning, and approve/deny actions", async () => {
    const onApprove = vi.fn();
    const onDeny = vi.fn();
    const step: ToolStepPayload = {
      version: 1,
      id: "step-101",
      kind: "command",
      name: "prune_containers",
      title: "Prune All Stopped Containers",
      safety: "DANGEROUS",
      status: "pending_approval",
      warning: "This will permanently remove all stopped containers.",
      rawInput: JSON.stringify({ all: true, force: true }),
    };

    render(ToolStepCard, { step, onApprove, onDeny });

    expect(screen.getByText("Prune All Stopped Containers")).toBeInTheDocument();
    expect(screen.getByText("prune_containers")).toBeInTheDocument();
    expect(screen.getByText("DANGEROUS")).toBeInTheDocument();
    expect(screen.getByText("Pending Approval")).toBeInTheDocument();
    expect(screen.getByText("This will permanently remove all stopped containers.")).toBeInTheDocument();

    const approveBtn = screen.getByRole("button", { name: /approve/i });
    const denyBtn = screen.getByRole("button", { name: /deny/i });

    await fireEvent.click(approveBtn);
    expect(onApprove).toHaveBeenCalledWith("step-101");

    await fireEvent.click(denyBtn);
    expect(onDeny).toHaveBeenCalledWith("step-101");
  });

  it("falls back to step.name when step.id is not provided in approval callbacks", async () => {
    const onApprove = vi.fn();
    const step: ToolStepPayload = {
      version: 1,
      kind: "event",
      name: "restart_vm",
      title: "Restart Colima VM",
      safety: "NORMAL",
      status: "pending_approval",
    };

    render(ToolStepCard, { step, onApprove });

    const approveBtn = screen.getByRole("button", { name: /approve/i });
    await fireEvent.click(approveBtn);

    expect(onApprove).toHaveBeenCalledWith("restart_vm");
  });

  it("renders running state with spinner and without approve/deny buttons", () => {
    const step: ToolStepPayload = {
      version: 1,
      id: "step-102",
      kind: "diagnose",
      name: "docker_diagnose",
      title: "Diagnosing Docker Daemon",
      safety: "SAFE",
      status: "running",
    };

    render(ToolStepCard, { step });

    expect(screen.getByText("Running…")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /approve/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /deny/i })).not.toBeInTheDocument();
  });

  it("renders succeeded state with Completed badge and collapsible payload", async () => {
    const step: ToolStepPayload = {
      version: 1,
      id: "step-103",
      kind: "query",
      name: "list_containers",
      title: "List Running Containers",
      safety: "SAFE",
      status: "succeeded",
      exitCode: 0,
      output: "CONTAINER ID   IMAGE     COMMAND\n123abc456def   alpine    sh",
    };

    render(ToolStepCard, { step });

    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("exit 0")).toBeInTheDocument();

    // Expand payload drawer
    const toggleBtn = screen.getByRole("button", { name: /parameters & output/i });
    expect(screen.queryByText(/CONTAINER ID/)).not.toBeInTheDocument();

    await fireEvent.click(toggleBtn);
    expect(screen.getByText(/CONTAINER ID/)).toBeInTheDocument();

    // Copy output
    const copyOutputBtn = screen.getByRole("button", { name: /copy/i });
    await fireEvent.click(copyOutputBtn);
    expect(mockWriteText).toHaveBeenCalledWith(step.output);
  });

  it("renders failed state with Failed badge and error exit code", () => {
    const step: ToolStepPayload = {
      version: 1,
      id: "step-104",
      kind: "command",
      name: "start_broken",
      title: "Start Crashing Container",
      safety: "NORMAL",
      status: "failed",
      exitCode: 137,
      output: "OOMKilled: container exceeded memory limit",
    };

    render(ToolStepCard, { step });

    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("exit 137")).toBeInTheDocument();
  });

  it("renders denied state with Denied status badge", () => {
    const step: ToolStepPayload = {
      version: 1,
      id: "step-105",
      kind: "security",
      name: "open_firewall",
      title: "Open Firewall Port",
      safety: "DANGEROUS",
      status: "denied",
    };

    render(ToolStepCard, { step });

    expect(screen.getByText("Denied")).toBeInTheDocument();
  });

  it("formats valid JSON rawInput and allows copying it", async () => {
    const step: ToolStepPayload = {
      version: 1,
      kind: "query",
      name: "query_config",
      title: "Query Configuration",
      safety: "SAFE",
      status: "succeeded",
      rawInput: '{"profile":"default","verbose":true}',
    };

    render(ToolStepCard, { step });

    const toggleBtn = screen.getByRole("button", { name: /parameters & output/i });
    await fireEvent.click(toggleBtn);

    expect(screen.getByText(/"profile": "default"/)).toBeInTheDocument();

    const copyInputBtn = screen.getByRole("button", { name: /copy/i });
    await fireEvent.click(copyInputBtn);
    expect(mockWriteText).toHaveBeenCalledWith(expect.stringContaining('"profile": "default"'));
  });

  it("renders various kind icons without throwing", () => {
    const kinds = ["query", "event", "command", "diagnose", "security", "navigate"] as const;
    for (const kind of kinds) {
      const { unmount } = render(ToolStepCard, {
        step: {
          version: 1,
          kind,
          name: `${kind}_test`,
          title: `Test ${kind}`,
          safety: "SAFE",
          status: "succeeded",
        },
      });
      expect(screen.getByText(`Test ${kind}`)).toBeInTheDocument();
      unmount();
    }
  });
});

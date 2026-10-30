import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import AiFollowUpBar from "./AiFollowUpBar.svelte";

afterEach(cleanup);

describe("AiFollowUpBar.svelte", () => {
  it("renders suggestions as chips capped at a maximum of 3 items", () => {
    const suggestions: [string, string][] = [
      ["Check CPU", "docker stats --no-stream"],
      ["Inspect logs", "docker logs --tail 50 web"],
      ["Restart container", "docker restart web"],
      ["Prune stopped", "docker container prune -f"],
      ["Show volumes", "docker volume ls"],
    ];

    render(AiFollowUpBar, { suggestions });

    expect(screen.getByText("Check CPU")).toBeInTheDocument();
    expect(screen.getByText("Inspect logs")).toBeInTheDocument();
    expect(screen.getByText("Restart container")).toBeInTheDocument();

    // 4th and 5th items must be capped out
    expect(screen.queryByText("Prune stopped")).not.toBeInTheDocument();
    expect(screen.queryByText("Show volumes")).not.toBeInTheDocument();
  });

  it("renders nothing when suggestions array is empty or undefined", () => {
    const { container, unmount } = render(AiFollowUpBar, { suggestions: [] });
    expect(container.querySelector(".ai-follow-up-bar")).toBeNull();
    unmount();

    const { container: container2 } = render(AiFollowUpBar, {});
    expect(container2.querySelector(".ai-follow-up-bar")).toBeNull();
  });

  it("handles clicking chip to send prompt immediately", async () => {
    const onSend = vi.fn();
    const suggestions: [string, string][] = [
      ["Check disk", "colima ssh -- df -h"],
    ];

    render(AiFollowUpBar, { suggestions, onSend });

    const chipBtn = screen.getByRole("button", { name: /send suggestion: check disk/i });
    await fireEvent.click(chipBtn);

    expect(onSend).toHaveBeenCalledWith("colima ssh -- df -h");
  });

  it("falls back to label when prompt is omitted or empty", async () => {
    const onSend = vi.fn();
    const suggestions = [["Status", ""]];

    render(AiFollowUpBar, { suggestions, onSend });

    const chipBtn = screen.getByRole("button", { name: /send suggestion: status/i });
    await fireEvent.click(chipBtn);

    expect(onSend).toHaveBeenCalledWith("Status");
  });

  it("handles clicking insert button to populate prompt without sending", async () => {
    const onSend = vi.fn();
    const onInsert = vi.fn();
    const suggestions: [string, string][] = [
      ["Check memory", "colima ssh -- free -m"],
    ];

    render(AiFollowUpBar, { suggestions, onSend, onInsert });

    const insertBtn = screen.getByRole("button", { name: /insert 'check memory'/i });
    await fireEvent.click(insertBtn);

    expect(onInsert).toHaveBeenCalledWith("colima ssh -- free -m");
    expect(onSend).not.toHaveBeenCalled();
  });

  it("handles keyboard Enter to send and Shift+Enter to insert", async () => {
    const onSend = vi.fn();
    const onInsert = vi.fn();
    const suggestions: [string, string][] = [
      ["Verify k8s", "kubectl get nodes"],
    ];

    render(AiFollowUpBar, { suggestions, onSend, onInsert });

    const chipBtn = screen.getByRole("button", { name: /send suggestion: verify k8s/i });

    // Shift+Enter inserts
    await fireEvent.keyDown(chipBtn, { key: "Enter", shiftKey: true });
    expect(onInsert).toHaveBeenCalledWith("kubectl get nodes");
    expect(onSend).not.toHaveBeenCalled();

    // Plain Enter sends
    await fireEvent.keyDown(chipBtn, { key: "Enter", shiftKey: false });
    expect(onSend).toHaveBeenCalledWith("kubectl get nodes");
  });

  it("disables all chip buttons and ignores clicks when disabled is true", async () => {
    const onSend = vi.fn();
    const onInsert = vi.fn();
    const suggestions: [string, string][] = [
      ["Disabled chip", "echo disabled"],
    ];

    render(AiFollowUpBar, { suggestions, disabled: true, onSend, onInsert });

    const chipBtn = screen.getByRole("button", { name: /send suggestion: disabled chip/i });
    const insertBtn = screen.getByRole("button", { name: /insert 'disabled chip'/i });

    expect(chipBtn).toBeDisabled();
    expect(insertBtn).toBeDisabled();

    await fireEvent.click(chipBtn);
    await fireEvent.click(insertBtn);

    expect(onSend).not.toHaveBeenCalled();
    expect(onInsert).not.toHaveBeenCalled();
  });

  it("survives duplicate labels without key collision errors", () => {
    const suggestions: [string, string][] = [
      ["Status", "colima status"],
      ["Status", "docker ps"],
    ];

    expect(() => {
      render(AiFollowUpBar, { suggestions });
    }).not.toThrow();

    expect(screen.getAllByText("Status").length).toBe(2);
  });
});

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import AiEmptyState from "./AiEmptyState.svelte";
import type { StarterPrompt } from "../../lib/aiSuggestions";

afterEach(cleanup);

describe("AiEmptyState.svelte", () => {
  it("renders welcoming header and greeting text", () => {
    render(AiEmptyState);

    expect(screen.getByText("Hi! I'm your Colima AI Assistant.")).toBeInTheDocument();
    expect(screen.getByText(/I can control containers, troubleshoot issues/i)).toBeInTheDocument();
  });

  it("renders onboarding card when isConfigured is false and handles action buttons", async () => {
    const onUseOllama = vi.fn();
    const onOpenSettings = vi.fn();

    render(AiEmptyState, {
      isConfigured: false,
      onUseOllama,
      onOpenSettings,
    });

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("AI Provider Not Configured")).toBeInTheDocument();

    const ollamaBtn = screen.getByRole("button", { name: /use local ollama/i });
    const settingsBtn = screen.getByRole("button", { name: /configure provider/i });

    await fireEvent.click(ollamaBtn);
    expect(onUseOllama).toHaveBeenCalledTimes(1);

    await fireEvent.click(settingsBtn);
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it("does not render onboarding card when isConfigured is true", () => {
    render(AiEmptyState, { isConfigured: true });

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText("AI Provider Not Configured")).not.toBeInTheDocument();
  });

  it("renders starter prompt cards with warning priority styling", async () => {
    const onSelectPrompt = vi.fn();
    const onInsertPrompt = vi.fn();

    const starterPrompts: StarterPrompt[] = [
      {
        id: "warn-stopped",
        title: "Colima VM 'default' is stopped",
        prompt: "Why is Colima stopped and how can I start it?",
        icon: "warning",
        category: "warning",
      },
      {
        id: "ctx-containers",
        title: "Show unhealthy containers",
        prompt: "Check docker ps for exited containers",
        icon: "container",
        category: "context",
      },
    ];

    render(AiEmptyState, {
      starterPrompts,
      onSelectPrompt,
      onInsertPrompt,
    });

    expect(screen.getByText("Colima VM 'default' is stopped")).toBeInTheDocument();
    expect(screen.getByText("Show unhealthy containers")).toBeInTheDocument();

    const warnCard = screen.getByText("Colima VM 'default' is stopped").closest(".ai-starter-card")!;
    expect(warnCard).toHaveClass("is-warning");

    // Click card body triggers select
    await fireEvent.click(warnCard);
    expect(onSelectPrompt).toHaveBeenCalledWith("Why is Colima stopped and how can I start it?");

    // Click insert button triggers insert without select
    const insertBtns = screen.getAllByRole("button", { name: /insert .* into chat input/i });
    await fireEvent.click(insertBtns[1]);
    expect(onInsertPrompt).toHaveBeenCalledWith("Check docker ps for exited containers");
  });

  it("supports keyboard Enter and Shift+Enter on prompt cards", async () => {
    const onSelectPrompt = vi.fn();
    const onInsertPrompt = vi.fn();

    const starterPrompts: StarterPrompt[] = [
      {
        id: "p1",
        title: "Diagnose VM",
        prompt: "colima status --verbose",
        icon: "vm",
        category: "context",
      },
    ];

    render(AiEmptyState, {
      starterPrompts,
      onSelectPrompt,
      onInsertPrompt,
    });

    const card = screen.getByText("Diagnose VM").closest(".ai-starter-card")!;

    // Plain Enter selects
    await fireEvent.keyDown(card, { key: "Enter", shiftKey: false });
    expect(onSelectPrompt).toHaveBeenCalledWith("colima status --verbose");

    // Shift+Enter inserts
    await fireEvent.keyDown(card, { key: "Enter", shiftKey: true });
    expect(onInsertPrompt).toHaveBeenCalledWith("colima status --verbose");
  });

  it("renders various icon categories correctly without throwing", () => {
    const icons = ["warning", "container", "vm", "security", "compose", "k8s", "general"] as const;
    const starterPrompts: StarterPrompt[] = icons.map((icon, idx) => ({
      id: `p-${idx}`,
      title: `Prompt ${icon}`,
      prompt: `Command ${icon}`,
      icon,
      category: "context",
    }));

    render(AiEmptyState, { starterPrompts });

    for (const icon of icons) {
      expect(screen.getByText(`Prompt ${icon}`)).toBeInTheDocument();
    }
  });
});

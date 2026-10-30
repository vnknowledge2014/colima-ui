import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import AiModelQuickBar from "./AiModelQuickBar.svelte";
import { dashboardState } from "../../store.svelte";
import { aiApi } from "../../lib/api";

afterEach(cleanup);

describe("AiModelQuickBar.svelte", () => {
  beforeEach(() => {
    dashboardState.colimaInstances = [
      {
        name: "default",
        status: "Running",
        arch: "aarch64",
        cpus: 4,
        memory: 4,
        disk: 60,
        runtime: "docker",
        address: "192.168.5.2",
        kubernetes: false,
      },
    ];
  });

  it("renders active provider and model in the pill button", () => {
    render(AiModelQuickBar, {
      activeProviderId: "anthropic",
      activeModel: "claude-3-5-sonnet-20241022",
    });

    expect(screen.getByText("Anthropic")).toBeInTheDocument();
    expect(screen.getByText("claude-3-5-sonnet-20241022")).toBeInTheDocument();
  });

  it("displays running Colima VM indicator correctly", () => {
    render(AiModelQuickBar, {
      activeProviderId: "ollama-local",
      activeModel: "llama3.2",
    });

    expect(screen.getByText("default")).toBeInTheDocument();
  });

  it("displays current page in the live context toggle chip", () => {
    render(AiModelQuickBar, {
      currentPage: "containers",
      liveContextEnabled: true,
    });

    expect(screen.getByText("Containers")).toBeInTheDocument();
  });

  it("handles live context chip toggle click", async () => {
    const onToggleLiveContext = vi.fn();
    render(AiModelQuickBar, {
      currentPage: "volumes",
      liveContextEnabled: true,
      onToggleLiveContext,
    });

    const chip = screen.getByRole("button", { name: /volumes/i });
    await fireEvent.click(chip);

    expect(onToggleLiveContext).toHaveBeenCalledWith(false);
  });

  it("opens popover dialog when model pill is clicked", async () => {
    render(AiModelQuickBar, {
      activeProviderId: "openai",
      activeModel: "gpt-4o",
    });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    const pill = screen.getByRole("button", { name: /gpt-4o/i });
    await fireEvent.click(pill);

    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText(/select ai provider/i)).toBeInTheDocument();
  });

  it("switches model when a popular preset chip is clicked", async () => {
    const onSelectModel = vi.fn();
    render(AiModelQuickBar, {
      activeProviderId: "ollama-local",
      activeModel: "llama3.2",
      onSelectModel,
    });

    const pill = screen.getByRole("button", { name: /llama3.2/i });
    await fireEvent.click(pill);

    const presetChip = screen.getByRole("button", { name: "deepseek-r1:8b" });
    await fireEvent.click(presetChip);

    expect(onSelectModel).toHaveBeenCalledWith("ollama-local", "deepseek-r1:8b");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("calls onOpenSettings when settings gear button is clicked", async () => {
    const onOpenSettings = vi.fn();
    render(AiModelQuickBar, { onOpenSettings });

    const settingsBtn = screen.getByRole("button", { name: /open ai settings/i });
    await fireEvent.click(settingsBtn);

    expect(onOpenSettings).toHaveBeenCalled();
  });

  it("fetches available models via aiApi.listModels and renders them", async () => {
    vi.spyOn(aiApi, "listModels").mockResolvedValueOnce(
      JSON.stringify(["model-alpha", "model-beta"])
    );

    render(AiModelQuickBar, {
      activeProviderId: "ollama-local",
      activeModel: "llama3.2",
    });

    const pill = screen.getByRole("button", { name: /llama3.2/i });
    await fireEvent.click(pill);

    const fetchBtn = screen.getByTitle(/fetch models/i);
    await fireEvent.click(fetchBtn);

    expect(aiApi.listModels).toHaveBeenCalled();
    expect(await screen.findByText("model-alpha")).toBeInTheDocument();
    expect(screen.getByText("model-beta")).toBeInTheDocument();
  });

  it("closes popover when escape key is pressed", async () => {
    render(AiModelQuickBar, {
      activeProviderId: "ollama-local",
      activeModel: "llama3.2",
    });

    const pill = screen.getByRole("button", { name: /llama3.2/i });
    await fireEvent.click(pill);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("switches and preserves active provider tab when activeProviderId prop is supplied", async () => {
    const onSelectModel = vi.fn();
    render(AiModelQuickBar, {
      activeProviderId: "ollama-local",
      activeModel: "llama3.2",
      onSelectModel,
    });

    const pill = screen.getByRole("button", { name: /llama3.2/i });
    await fireEvent.click(pill);

    const [ollamaTab] = screen.getAllByRole("button", { name: "Ollama" });
    expect(ollamaTab).toHaveClass("active");

    const anthropicTab = screen.getByRole("button", { name: "Anthropic" });
    await fireEvent.click(anthropicTab);

    // Verified: Anthropic tab remains active
    expect(anthropicTab).toHaveClass("active");
    expect(ollamaTab).not.toHaveClass("active");

    const preset = screen.getByRole("button", { name: "claude-3-5-sonnet-20241022" });
    await fireEvent.click(preset);

    expect(onSelectModel).toHaveBeenCalledWith("anthropic", "claude-3-5-sonnet-20241022");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("preserves typed model name without keystroke rollback when activeModel prop is supplied", async () => {
    const onSelectModel = vi.fn();
    render(AiModelQuickBar, {
      activeProviderId: "openai",
      activeModel: "gpt-4o",
      onSelectModel,
    });

    const pill = screen.getByRole("button", { name: /gpt-4o/i });
    await fireEvent.click(pill);

    const input = screen.getByPlaceholderText(/enter model name/i) as HTMLInputElement;
    expect(input.value).toBe("gpt-4o");

    await fireEvent.input(input, { target: { value: "o3-mini-high" } });
    expect(input.value).toBe("o3-mini-high");

    const applyBtn = screen.getByRole("button", { name: /apply/i });
    expect(applyBtn).not.toBeDisabled();
    await fireEvent.click(applyBtn);

    expect(onSelectModel).toHaveBeenCalledWith("openai", "o3-mini-high");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("syncs internal state when external activeProviderId and activeModel props change", async () => {
    const { rerender } = render(AiModelQuickBar, {
      activeProviderId: "ollama-local",
      activeModel: "llama3.2",
    });

    expect(screen.getByText("llama3.2")).toBeInTheDocument();

    await rerender({
      activeProviderId: "anthropic",
      activeModel: "claude-3-5-haiku-20241022",
    });

    expect(screen.getByText("Anthropic")).toBeInTheDocument();
    expect(screen.getByText("claude-3-5-haiku-20241022")).toBeInTheDocument();
  });
});

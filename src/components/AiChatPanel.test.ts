import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent, waitFor } from "@testing-library/svelte";
import AiChatPanel from "./AiChatPanel.svelte";
import { aiState } from "../store/ai.svelte";
import { uiState, dashboardState } from "../store.svelte";
import { globalToast } from "../lib/globalToast";

vi.mock("../lib/globalToast", () => ({
  globalToast: vi.fn(),
  onError: vi.fn(),
}));

vi.mock("../store/confirm.svelte", () => ({
  confirm: vi.fn().mockResolvedValue(true),
}));

vi.mock("../lib/agentCore", () => ({
  runAgent: vi.fn().mockImplementation(async () => {}),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    aiApi: {
      ...actual.aiApi,
      saveMessage: vi.fn().mockResolvedValue(undefined),
      clearHistory: vi.fn().mockResolvedValue(undefined),
      loadHistory: vi.fn().mockResolvedValue([]),
      listConversations: vi.fn().mockResolvedValue([]),
      listModels: vi.fn().mockResolvedValue("[]"),
      getAppContext: vi.fn().mockResolvedValue(""),
      createConversation: vi.fn().mockResolvedValue("new-conv"),
      renameConversation: vi.fn().mockResolvedValue(undefined),
      deleteConversation: vi.fn().mockResolvedValue(undefined),
    },
  };
});

describe("AiChatPanel.svelte Integration", () => {
  beforeEach(() => {
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
    aiState.messages = [];
    aiState.isProcessing = false;
    uiState.aiPanelOpen = true;
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
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("1. renders AiEmptyState when chat messages list is empty", () => {
    render(AiChatPanel);
    expect(screen.getByText(/Hi! I'm your Colima AI Assistant/i)).toBeInTheDocument();
  });

  it("2. renders message turns and mounts AiMessageBody for assistant", () => {
    aiState.messages = [
      { id: "m1", role: "user", content: "How to check containers?" },
      { id: "m2", role: "assistant", content: "Run `docker ps -a` to inspect." },
    ];
    render(AiChatPanel);

    expect(screen.getByText("How to check containers?")).toBeInTheDocument();
    expect(screen.getByText(/docker ps -a/i)).toBeInTheDocument();
  });

  it("3. copy action button copies assistant markdown and displays toast", async () => {
    aiState.messages = [
      { id: "m1", role: "assistant", content: "Colima is running on vz." },
    ];
    render(AiChatPanel);

    const copyBtn = screen.getByRole("button", { name: /^copy$/i });
    await fireEvent.click(copyBtn);

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Colima is running on vz.");
    expect(globalToast).toHaveBeenCalledWith("success", expect.stringMatching(/copied/i), expect.anything());
  });

  it("4. retry action button triggers re-send after truncating", async () => {
    const { runAgent } = await import("../lib/agentCore");
    aiState.messages = [
      { id: "u1", role: "user", content: "List all VMs" },
      { id: "a1", role: "assistant", content: "Here is your VM: default" },
    ];
    render(AiChatPanel);

    const retryBtn = screen.getByRole("button", { name: /retry/i });
    await fireEvent.click(retryBtn);

    await waitFor(() => {
      expect(runAgent).toHaveBeenCalled();
    });
  });

  it("5. edit action button populates input composer with user message", async () => {
    aiState.messages = [
      { id: "u1", role: "user", content: "Initial query text" },
      { id: "a1", role: "assistant", content: "Response" },
    ];
    render(AiChatPanel);

    const editBtn = screen.getByRole("button", { name: /edit & resend/i });
    await fireEvent.click(editBtn);

    const textarea = screen.getByPlaceholderText(/type a message/i) as HTMLTextAreaElement;
    expect(textarea.value).toBe("Initial query text");
  });

  it("6. auto-scroll maintains user scroll position and shows scroll-to-bottom button when scrolled up", async () => {
    const { container } = render(AiChatPanel);
    const messagesEl = container.querySelector(".ai-panel-messages") as HTMLDivElement;

    messagesEl.scrollTo = function (options?: ScrollToOptions | number) {
      if (typeof options === "object" && options?.top !== undefined) {
        this.scrollTop = options.top;
      }
    };

    // Simulate scrolled up
    Object.defineProperty(messagesEl, "scrollTop", { value: 100, writable: true });
    Object.defineProperty(messagesEl, "clientHeight", { value: 400, writable: true });
    Object.defineProperty(messagesEl, "scrollHeight", { value: 1000, writable: true });
    await fireEvent.scroll(messagesEl);

    const scrollBtn = screen.getByRole("button", { name: /scroll to bottom/i });
    expect(scrollBtn).toBeInTheDocument();

    await fireEvent.click(scrollBtn);
    expect(messagesEl.scrollTop).toBe(1000);
  });

  it("7. clear chat prompts confirmation dialog before clearing", async () => {
    const { confirm } = await import("../store/confirm.svelte");
    aiState.messages = [{ id: "m1", role: "user", content: "hello" }];
    render(AiChatPanel);

    const clearBtn = screen.getByTitle(/clear this chat/i);
    await fireEvent.click(clearBtn);

    expect(confirm).toHaveBeenCalledTimes(1);
    expect(aiState.messages.length).toBe(0);
  });

  it("8. renders AiModelQuickBar sub-header strip", () => {
    const { container } = render(AiChatPanel);
    expect(container.querySelector(".ai-model-quick-bar")).toBeInTheDocument();
  });

  it("9. renders live ToolStepCard Approve/Deny buttons during onApprovalRequired and updates card in-place on click", async () => {
    const { runAgent } = await import("../lib/agentCore");
    const { createToolStep, transitionToolStep, serializeToolStep } = await import("../lib/aiToolStep");

    vi.mocked(runAgent).mockImplementationOnce(async (_text, _cfg, callbacks) => {
      const approvalId = "live-approval-1";
      let step = createToolStep({
        id: approvalId,
        kind: "event",
        name: "start-instance",
        title: "Start Instance",
        safety: "NORMAL",
        status: "pending_approval",
        rawInput: '{"profile":"default"}',
      });
      callbacks.onMessage({
        id: approvalId,
        role: "system",
        content: serializeToolStep(step),
      });

      const approved = await callbacks.onApprovalRequired(approvalId, '[EVENT_APPROVE: start-instance | {"profile":"default"}]');
      step = transitionToolStep(step, approved ? "running" : "denied");
      callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
      if (approved) {
        step = transitionToolStep(step, "succeeded", { output: "Started default VM" });
        callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
      }
    });

    render(AiChatPanel);

    const textarea = screen.getByPlaceholderText(/type a message/i);
    await fireEvent.input(textarea, { target: { value: "Start my VM" } });
    const sendBtn = screen.getByRole("button", { name: /send message/i });
    await fireEvent.click(sendBtn);

    // Approve button must appear in the live ToolStepCard
    const approveBtn = await screen.findByRole("button", { name: /approve start instance/i });
    expect(approveBtn).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /deny start instance/i })).toBeInTheDocument();

    await fireEvent.click(approveBtn);

    await waitFor(() => {
      expect(screen.getByText("Completed")).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: /approve start instance/i })).not.toBeInTheDocument();
  });

  it("10. derives tool-aware fallback follow-up chips from recent ToolStep turns", async () => {
    const { createToolStep, serializeToolStep } = await import("../lib/aiToolStep");
    const step = createToolStep({
      id: "step-q1",
      kind: "query",
      name: "list-containers",
      title: "List Containers",
      safety: "SAFE",
      status: "succeeded",
      output: "[]",
    });

    aiState.messages = [
      { id: "u1", role: "user", content: "What containers are running?" },
      { id: "step-q1", role: "system", content: serializeToolStep(step) },
      { id: "a1", role: "assistant", content: "You currently have 2 running containers." },
    ];

    render(AiChatPanel);

    expect(screen.getByText("Check CPU & memory")).toBeInTheDocument();
    expect(screen.getByText("View container logs")).toBeInTheDocument();
  });
});

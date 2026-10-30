import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import AiChatPanel from "../components/AiChatPanel.svelte";
import AiModelQuickBar from "../components/ai/AiModelQuickBar.svelte";
import CodeBlock from "../components/ai/CodeBlock.svelte";
import InteractiveChoiceCard from "../components/ai/InteractiveChoiceCard.svelte";
import AiFollowUpBar from "../components/ai/AiFollowUpBar.svelte";
import AiEmptyState from "../components/ai/AiEmptyState.svelte";
import ToolStepCard from "../components/ai/ToolStepCard.svelte";
import { aiState } from "../store/ai.svelte";
import { uiState, dashboardState, openTerminalSession } from "../store.svelte";
import { globalToast } from "../lib/globalToast";
import { aiApi } from "../lib/api";
import { confirm } from "../store/confirm.svelte";
import {
  parseChatMarkdown,
  formatInline,
  isShellCode,
  parseTableRow,
  parseTableDelimiter,
  escapeHtml,
  sanitizeUrl,
} from "../lib/chatMarkdown";
import {
  parseAiTools,
  extractChoicePrompt,
  stripStreamingPartialTags,
  rebuildAssistantTurnContent,
  type ParsedChoicePrompt,
} from "../lib/aiToolParser";
import {
  createToolStep,
  transitionToolStep,
  isValidStatusTransition,
  serializeToolStep,
  parseToolStep,
  generateToolStepSummary,
  type ToolStepPayload,
} from "../lib/aiToolStep";
import {
  getStarterPrompts,
  deriveFallbackSuggestions,
  buildLiveUiContextSnapshot,
} from "../lib/aiSuggestions";

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

vi.mock("../store.svelte", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../store.svelte")>();
  return {
    ...actual,
    openTerminalSession: vi.fn(),
  };
});

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return {
    ...actual,
    aiApi: {
      ...actual.aiApi,
      saveMessage: vi.fn().mockResolvedValue(undefined),
      clearHistory: vi.fn().mockResolvedValue(undefined),
      loadHistory: vi.fn().mockImplementation(async () => aiState.messages),
      listConversations: vi.fn().mockResolvedValue([]),
      listModels: vi.fn().mockResolvedValue("[]"),
      getAppContext: vi.fn().mockResolvedValue(""),
      createConversation: vi.fn().mockResolvedValue("new-conv"),
      renameConversation: vi.fn().mockResolvedValue(undefined),
      deleteConversation: vi.fn().mockResolvedValue(undefined),
    },
  };
});

describe("Tier 5: White-Box Adversarial Coverage Hardening Suite", () => {
  const mockWriteText = vi.fn().mockResolvedValue(undefined);

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
        writeText: mockWriteText,
      },
    });

    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  // =========================================================================
  // 1. Boundary & Malformed Streaming Payloads
  // =========================================================================
  describe("1. Boundary & Malformed Streaming Payloads", () => {
    it("handles unclosed <think> stream blocks emitting streaming: true without freezing", () => {
      const payload = "Preliminary text\n<think>Analyzing VM config and memory layout...\nEvaluating virtiofs mount";
      const blocks = parseChatMarkdown(payload);

      expect(blocks).toHaveLength(2);
      expect(blocks[0]).toEqual({
        type: "paragraph",
        text: "Preliminary text",
      });
      expect(blocks[1]).toEqual({
        type: "think",
        content: "Analyzing VM config and memory layout...\nEvaluating virtiofs mount",
        streaming: true,
      });
    });

    it("handles unclosed empty <think> tags gracefully", () => {
      const payload = "<think>";
      const blocks = parseChatMarkdown(payload);

      expect(blocks).toHaveLength(1);
      expect(blocks[0]).toEqual({
        type: "think",
        content: "",
        streaming: true,
      });
    });

    it("handles empty completed think tags <think></think> correctly", () => {
      const payload = "<think></think>\nFinal answer ready.";
      const blocks = parseChatMarkdown(payload);

      expect(blocks).toHaveLength(2);
      expect(blocks[0]).toEqual({
        type: "think",
        content: "",
        streaming: false,
      });
      expect(blocks[1]).toEqual({
        type: "paragraph",
        text: "Final answer ready.",
      });
    });

    it("handles partial closing tag </thin in streaming payload without crashing", () => {
      const payload = "<think>Working on diagnostics...</thin";
      const blocks = parseChatMarkdown(payload);

      expect(blocks).toHaveLength(1);
      expect(blocks[0].type).toBe("think");
      if (blocks[0].type === "think") {
        expect(blocks[0].streaming).toBe(true);
        expect(blocks[0].content).toContain("Working on diagnostics...</thin");
      }
    });

    it("handles multiple think blocks where the first is closed and second is streaming", () => {
      const payload = "<think>First pass</think>\nInterim answer\n<think>Second pass in progress";
      const blocks = parseChatMarkdown(payload);

      expect(blocks).toHaveLength(3);
      expect(blocks[0]).toEqual({
        type: "think",
        content: "First pass",
        streaming: false,
      });
      expect(blocks[1]).toEqual({
        type: "paragraph",
        text: "Interim answer",
      });
      expect(blocks[2]).toEqual({
        type: "think",
        content: "Second pass in progress",
        streaming: true,
      });
    });

    it("parses table delimiter and row boundaries safely with ragged columns", () => {
      expect(parseTableDelimiter("not-a-table")).toBeNull();
      expect(parseTableDelimiter("|---|:---:|---:|")).toEqual(["none", "center", "right"]);

      const raggedRow = parseTableRow("| col1 | col2 |", 4);
      expect(raggedRow).toHaveLength(4);
      expect(raggedRow[2]).toBe("");
      expect(raggedRow[3]).toBe("");

      const surplusRow = parseTableRow("| c1 | c2 | c3 | c4 | c5 |", 2);
      expect(surplusRow).toHaveLength(2);
      expect(surplusRow).toEqual(["c1", "c2"]);
    });

    it("returns null for truncated or broken tool step envelopes", () => {
      expect(parseToolStep("<!--AI_TOOL_STEP:{\"version\":1,\"kind\":\"comm")).toBeNull();
      expect(parseToolStep("<!--AI_TOOL_STEP:not-json-->")).toBeNull();
      expect(parseToolStep("<!--AI_TOOL_STEP:{}-->")).toBeNull();
      expect(parseToolStep("")).toBeNull();
      expect(parseToolStep("Plain prose with no tool steps")).toBeNull();
    });

    it("rejects tool steps with schema violations (invalid status, safety, or version)", () => {
      const invalidStatus = JSON.stringify({
        version: 1,
        kind: "command",
        name: "test",
        title: "Test",
        safety: "SAFE",
        status: "illegal_status",
      });
      expect(parseToolStep(`<!--AI_TOOL_STEP:${invalidStatus}-->`)).toBeNull();

      const invalidSafety = JSON.stringify({
        version: 1,
        kind: "command",
        name: "test",
        title: "Test",
        safety: "CRITICAL_HAZARD",
        status: "succeeded",
      });
      expect(parseToolStep(`<!--AI_TOOL_STEP:${invalidSafety}-->`)).toBeNull();

      const invalidVersion = JSON.stringify({
        version: 2,
        kind: "command",
        name: "test",
        title: "Test",
        safety: "SAFE",
        status: "succeeded",
      });
      expect(parseToolStep(`<!--AI_TOOL_STEP:${invalidVersion}-->`)).toBeNull();
    });

    it("properly serializes and recovers tool step payload containing comment delimiters (-->)", () => {
      const payload: ToolStepPayload = {
        version: 1,
        kind: "command",
        name: "docker_run",
        title: "Run Container",
        safety: "NORMAL",
        status: "succeeded",
        rawInput: "docker run -e REDIRECT='--> target' alpine",
        output: "Completed --> exit 0",
      };

      const serialized = serializeToolStep(payload);
      // Delimiter in rawInput must be unicode escaped in envelope
      expect(serialized).not.toContain("--> target");
      expect(serialized).toContain("--\\u003e target");

      const recovered = parseToolStep(serialized);
      expect(recovered).not.toBeNull();
      expect(recovered?.name).toBe("docker_run");
      expect(recovered?.rawInput).toBe("docker run -e REDIRECT='--> target' alpine");
      expect(recovered?.output).toBe("Completed --> exit 0");
    });

    it("enforces tool step state machine transitions strictly", () => {
      expect(isValidStatusTransition("pending_approval", "running")).toBe(true);
      expect(isValidStatusTransition("pending_approval", "denied")).toBe(true);
      expect(isValidStatusTransition("pending_approval", "failed")).toBe(true);
      expect(isValidStatusTransition("running", "succeeded")).toBe(true);
      expect(isValidStatusTransition("running", "failed")).toBe(true);
      expect(isValidStatusTransition("running", "running")).toBe(true);

      // Terminal states cannot transition to any other status
      expect(isValidStatusTransition("succeeded", "running")).toBe(false);
      expect(isValidStatusTransition("denied", "running")).toBe(false);
      expect(isValidStatusTransition("failed", "succeeded")).toBe(false);

      const step = createToolStep({
        kind: "command",
        name: "start_vm",
        title: "Start Colima VM",
        safety: "NORMAL",
        status: "pending_approval",
      });

      const runningStep = transitionToolStep(step, "running");
      expect(runningStep.status).toBe("running");

      expect(() => transitionToolStep(runningStep, "succeeded")).not.toThrow();

      const succeededStep = transitionToolStep(runningStep, "succeeded");
      expect(() => transitionToolStep(succeededStep, "running")).toThrowError(
        /Invalid tool step status transition/
      );
    });

    it("generates markdown fallback summaries across all tool step status values", () => {
      const base: ToolStepPayload = {
        version: 1,
        kind: "command",
        name: "test",
        title: "Test Tool",
        safety: "SAFE",
        status: "pending_approval",
        rawInput: "echo hello",
      };
      expect(generateToolStepSummary(base)).toContain("⏳ Action Required: **Test Tool**");

      expect(generateToolStepSummary({ ...base, status: "running" })).toContain("⟳ Executing: **Test Tool**");
      expect(generateToolStepSummary({ ...base, status: "succeeded", output: "ok" })).toContain("✓ Completed: **Test Tool**");
      expect(generateToolStepSummary({ ...base, status: "denied" })).toContain("✕ Action Denied: **Test Tool**");
      expect(generateToolStepSummary({ ...base, status: "failed", warning: "timeout" })).toContain("✕ Failed: **Test Tool** — timeout");
    });

    it("parses multiple concurrent AI tool tags and strips them cleanly", () => {
      const rawText = [
        "Let me run diagnostics and configure the environment.",
        "[QUERY: app_state]",
        "[DIAGNOSE]",
        "[RUN: docker ps -a]",
        "[SUGGEST: Inspect Containers | Check CPU and memory usage]",
        "[SUGGEST: View Logs]",
      ].join("\n");

      const parsed = parseAiTools(rawText);
      expect(parsed.hasTools).toBe(true);
      expect(parsed.hasDiagnose).toBe(true);
      expect(parsed.queries).toHaveLength(1);
      expect(parsed.runs).toHaveLength(1);
      expect(parsed.suggests).toHaveLength(2);
      expect(parsed.cleanText).toBe("Let me run diagnostics and configure the environment.");
    });

    it("rebuilds assistant turn content for SQLite persistence without execution tags", () => {
      const cleanProse = "Diagnosed VM state and generated configuration recommendations.";
      const suggests = [["Inspect Memory", "colima status mem"]];
      const choice: ParsedChoicePrompt = {
        question: "Select VM size",
        multi: false,
        options: [{ label: "Standard", value: "std" }],
      };

      const rebuilt = rebuildAssistantTurnContent(cleanProse, suggests, choice);
      expect(rebuilt).toContain(cleanProse);
      expect(rebuilt).toContain("[ASK_CHOICE:");
      expect(rebuilt).toContain("[SUGGEST: Inspect Memory | colima status mem]");
    });

    it("handles malformed choice JSON by falling through safely without throw", () => {
      const malformedJson = "[ASK_CHOICE: {\"question\": \"Pick one\", \"options\": [ broken json ]]";
      const res = extractChoicePrompt(malformedJson);

      // Gracefully falls back to pipe parser or returns null prompt while cleaning text
      expect(res.cleanText).toBe("");
      expect(res.choicePrompt).toBeNull();
    });

    it("handles truncated pipe choice syntax without crashing", () => {
      const truncatedPipe = "[ASK_CHOICE: Single question with no options]";
      const res = extractChoicePrompt(truncatedPipe);
      expect(res.choicePrompt).toBeNull();
      expect(res.cleanText).toBe("");
    });

    it("parses multi choice syntax with case-insensitive variants [multi] and (multi)", () => {
      const multiSquare = "[ASK_CHOICE: [multi] Select tools | Docker | Colima | Kubernetes]";
      const res1 = extractChoicePrompt(multiSquare);
      expect(res1.choicePrompt?.multi).toBe(true);
      expect(res1.choicePrompt?.question).toBe("Select tools");
      expect(res1.choicePrompt?.options).toHaveLength(3);

      const multiParen = "[ASK_CHOICE: (multi) Select features | CPU | RAM | Disk]";
      const res2 = extractChoicePrompt(multiParen);
      expect(res2.choicePrompt?.multi).toBe(true);
      expect(res2.choicePrompt?.question).toBe("Select features");
      expect(res2.choicePrompt?.options).toHaveLength(3);
    });

    it("strips partial streaming tags with complex bracket nesting without corrupting prose", () => {
      // Bracket inside JSON string must NOT prematurely close tag check
      const closedJson = "[QUERY: {\"query\": \"SELECT [all] FROM containers\"}]";
      const stripped1 = stripStreamingPartialTags(closedJson);
      expect(stripped1.pendingTagKind).toBeNull();

      // Streaming tag cut off mid-payload
      const cutOff = "Here is the response [RUN: docker run -d --name test";
      const stripped2 = stripStreamingPartialTags(cutOff);
      expect(stripped2.cleanText).toBe("Here is the response");
      expect(stripped2.pendingTagKind).toBe("run");

      // Distinguishes [QUERY_APP_STATE from [QUERY
      const cutOffAppState = "Current status: [QUERY_APP_STATE";
      const stripped3 = stripStreamingPartialTags(cutOffAppState);
      expect(stripped3.cleanText).toBe("Current status:");
      expect(stripped3.pendingTagKind).toBe("diagnose");

      // Preserves normal markdown links without false positive tag suppression
      const mdLink = "Refer to [Colima Docs](https://github.com/abiosoft/colima) for setup.";
      const stripped4 = stripStreamingPartialTags(mdLink);
      expect(stripped4.cleanText).toBe(mdLink);
      expect(stripped4.pendingTagKind).toBeNull();
    });
  });

  // =========================================================================
  // 2. Shell Command Payload Injection, Escaping, Quotes & Terminal Targeting
  // =========================================================================
  describe("2. Shell Command Payload Injection, Escaping, Quotes & Terminal Targeting", () => {
    it("identifies shell commands across varied languages, shebangs, prompts, and prefixes", () => {
      expect(isShellCode("bash", "echo hello")).toBe(true);
      expect(isShellCode("zsh", "ls -la")).toBe(true);
      expect(isShellCode("docker", "FROM alpine")).toBe(true);
      expect(isShellCode("kubectl", "get pods -A")).toBe(true);

      // Generic/empty language fallback to prefix detection
      expect(isShellCode("", "colima start --cpu 4 --memory 8")).toBe(true);
      expect(isShellCode("text", "docker compose -f docker-compose.yml up -d")).toBe(true);
      expect(isShellCode("", "# Configure cluster\nkubectl apply -f app.yaml")).toBe(true);
      expect(isShellCode("", "$ colima status")).toBe(true);
      expect(isShellCode("", "> limactl list")).toBe(true);
      expect(isShellCode("", "sudo nerdctl run -d nginx")).toBe(true);

      // Non-shell code blocks
      expect(isShellCode("json", '{"name": "colima"}')).toBe(false);
      expect(isShellCode("python", "import os\nos.system('ls')")).toBe(false);
      expect(isShellCode("", "const x = 42;")).toBe(false);
    });

    it("renders shell CodeBlock with complex quotes, heredocs, and subshells and copies verbatim", async () => {
      const complexScript = [
        "#!/usr/bin/env bash",
        "set -euo pipefail",
        "cat << 'EOF' > /tmp/colima-test.json",
        '{ "target": "vm", "env": "$PATH", "quotes": "\'single\' \\"double\\"" }',
        "EOF",
        "colima start --cpu $(sysctl -n hw.ncpu) --memory 8 --mount $HOME:w",
      ].join("\n");

      render(CodeBlock, {
        code: complexScript,
        language: "bash",
        isShell: true,
      });

      const termBtn = screen.getByRole("button", { name: /terminal/i });
      expect(termBtn).toBeInTheDocument();

      await fireEvent.click(termBtn);

      // 1. Verbatim script copied to clipboard without corruption
      expect(mockWriteText).toHaveBeenCalledWith(complexScript);

      // 2. Open terminal session dispatched with target profile
      expect(openTerminalSession).toHaveBeenCalledWith({
        kind: "colima",
        profile: "default",
      });

      // 3. Toast displayed with record: false
      expect(globalToast).toHaveBeenCalledWith(
        "info",
        expect.stringContaining("Command copied"),
        { record: false }
      );
    });

    it("resolves active VM target correctly when non-default VM is running", async () => {
      dashboardState.colimaInstances = [
        { name: "default", status: "Stopped" } as never,
        { name: "k8s-cluster", status: "Running" } as never,
      ];

      render(CodeBlock, {
        code: "kubectl get nodes",
        isShell: true,
      });

      const termBtn = screen.getByRole("button", { name: /terminal/i });
      await fireEvent.click(termBtn);

      expect(openTerminalSession).toHaveBeenCalledWith({
        kind: "colima",
        profile: "k8s-cluster",
      });
    });

    it("normalizes 'colima' VM name to 'default' profile", async () => {
      dashboardState.colimaInstances = [
        { name: "colima", status: "running" } as never,
      ];

      render(CodeBlock, {
        code: "docker ps",
        isShell: true,
      });

      const termBtn = screen.getByRole("button", { name: /terminal/i });
      await fireEvent.click(termBtn);

      expect(openTerminalSession).toHaveBeenCalledWith({
        kind: "colima",
        profile: "default",
      });
    });

    it("handles clipboard write failure gracefully in handleCopy and handleOpenTerminal", async () => {
      mockWriteText.mockRejectedValueOnce(new Error("Clipboard permission denied"));

      render(CodeBlock, {
        code: "docker system prune -a",
        isShell: true,
      });

      const copyBtn = screen.getByRole("button", { name: /copy/i });
      await fireEvent.click(copyBtn);

      expect(globalToast).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("Failed to copy")
      );
    });

    it("invokes onInsert callback when Insert button is pressed", async () => {
      const onInsert = vi.fn();
      render(CodeBlock, {
        code: "colima restart",
        isShell: true,
        onInsert,
      });

      const insertBtn = screen.getByRole("button", { name: /insert/i });
      await fireEvent.click(insertBtn);

      expect(onInsert).toHaveBeenCalledWith("colima restart");
    });
  });

  // =========================================================================
  // 3. Rapid State Transitions and Concurrent UI Events
  // =========================================================================
  describe("3. Rapid State Transitions and Concurrent UI Events", () => {
    it("handles rapid popover toggling and live context switching in AiModelQuickBar", async () => {
      const onSelectModel = vi.fn();
      const onToggleLiveContext = vi.fn();

      const { container } = render(AiModelQuickBar, {
        activeProviderId: "ollama-local",
        activeModel: "llama3.2",
        liveContextEnabled: true,
        currentPage: "instances",
        onSelectModel,
        onToggleLiveContext,
      });

      // Rapidly toggle live context chip
      const contextChip = container.querySelector(".ai-context-chip") as HTMLButtonElement;
      expect(contextChip).toBeInTheDocument();
      await fireEvent.click(contextChip);
      await fireEvent.click(contextChip);
      await fireEvent.click(contextChip);

      expect(onToggleLiveContext).toHaveBeenCalledTimes(3);
      expect(onToggleLiveContext).toHaveBeenLastCalledWith(false);

      // Rapidly toggle model selector popover
      const triggerBtn = container.querySelector(".ai-model-pill") as HTMLButtonElement;
      expect(triggerBtn).toBeInTheDocument();
      await fireEvent.click(triggerBtn);
      expect(container.querySelector(".ai-model-popover")).toBeInTheDocument();

      await fireEvent.click(triggerBtn);
      expect(container.querySelector(".ai-model-popover")).not.toBeInTheDocument();

      await fireEvent.click(triggerBtn);
      expect(container.querySelector(".ai-model-popover")).toBeInTheDocument();
    });

    it("retains typed model input in AiModelQuickBar during concurrent external prop updates", async () => {
      const { container, rerender } = render(AiModelQuickBar, {
        activeProviderId: "ollama-local",
        activeModel: "llama3.2",
        liveContextEnabled: true,
        currentPage: "dashboard",
      });

      const triggerBtn = container.querySelector(".ai-model-pill") as HTMLButtonElement;
      expect(triggerBtn).toBeInTheDocument();
      await fireEvent.click(triggerBtn);

      const input = screen.getByPlaceholderText(/enter model name/i) as HTMLInputElement;
      await fireEvent.input(input, { target: { value: "my-experimental-model" } });
      expect(input.value).toBe("my-experimental-model");

      // Rerender with identical prop updates (simulating background store poller)
      await rerender({
        activeProviderId: "ollama-local",
        activeModel: "llama3.2",
        liveContextEnabled: true,
        currentPage: "dashboard",
      });

      // Typed value must NOT be rolled back
      expect(input.value).toBe("my-experimental-model");
    });

    it("handles InteractiveChoiceCard keyboard navigation, selection, and turn locking", async () => {
      const onSelect = vi.fn();
      const onInsert = vi.fn();
      const choice: ParsedChoicePrompt = {
        question: "Select VM Architecture",
        multi: false,
        options: [
          { label: "x86_64", description: "Intel/AMD Emulation", value: "x86_64" },
          { label: "aarch64", description: "Native Apple Silicon", value: "aarch64" },
        ],
      };

      const { container, rerender } = render(InteractiveChoiceCard, {
        choice,
        isLatest: true,
        onSelect,
        onInsert,
      });

      const optionsContainer = container.querySelector(".choice-options") as HTMLDivElement;

      // Navigate down and select via keyboard
      await fireEvent.keyDown(optionsContainer, { key: "ArrowDown" });
      await fireEvent.keyDown(optionsContainer, { key: "Enter" });

      expect(onSelect).toHaveBeenCalledWith("aarch64");

      // When isLatest is false, interactions are locked
      await rerender({
        choice,
        isLatest: false,
        onSelect,
        onInsert,
      });

      onSelect.mockClear();
      const optionRows = container.querySelectorAll(".choice-option-row");
      await fireEvent.click(optionRows[0]);

      expect(onSelect).not.toHaveBeenCalled();
    });

    it("caps AiFollowUpBar at 3 chips and handles keyboard Enter vs Shift+Enter safely", async () => {
      const onSend = vi.fn();
      const onInsert = vi.fn();

      const suggestions: [string, string][] = [
        ["Inspect CPU", "colima status cpu"],
        ["Check RAM", "colima status mem"],
        ["View Logs", "colima logs"],
        ["Surplus Chip 4", "surplus 4"],
        ["Surplus Chip 5", "surplus 5"],
      ];

      render(AiFollowUpBar, {
        suggestions,
        disabled: false,
        onSend,
        onInsert,
      });

      const sendButtons = screen.getAllByRole("button", { name: /send suggestion/i });
      expect(sendButtons).toHaveLength(3);

      // Press Enter to send
      await fireEvent.keyDown(sendButtons[0], { key: "Enter" });
      expect(onSend).toHaveBeenCalledWith("colima status cpu");

      // Press Shift+Enter to insert
      await fireEvent.keyDown(sendButtons[1], { key: "Enter", shiftKey: true });
      expect(onInsert).toHaveBeenCalledWith("colima status mem");
    });

    it("deduplicates duplicate suggestion chips with compound index keys without crashing", () => {
      const duplicateSuggestions: [string, string][] = [
        ["Check Status", "colima status"],
        ["Check Status", "colima status"],
        ["Check Status", "colima status"],
      ];

      expect(() => {
        render(AiFollowUpBar, {
          suggestions: duplicateSuggestions,
          disabled: false,
        });
      }).not.toThrow();

      const sendButtons = screen.getAllByRole("button", { name: /send suggestion/i });
      expect(sendButtons).toHaveLength(3);
    });

    it("prioritizes system warnings in AiEmptyState and triggers callbacks properly", async () => {
      const onSelectPrompt = vi.fn();
      const onInsertPrompt = vi.fn();
      const onUseOllama = vi.fn();
      const onOpenSettings = vi.fn();

      const prompts = getStarterPrompts({
        currentPage: "instances",
        instances: [{ name: "default", status: "Stopped" } as never],
        containers: [{ id: "c1", Names: ["/nginx-web"], State: "exited" } as never],
      });

      expect(prompts).toHaveLength(4);
      expect(prompts[0].category).toBe("warning");
      expect(prompts[1].category).toBe("warning");

      render(AiEmptyState, {
        starterPrompts: prompts,
        isConfigured: false,
        onSelectPrompt,
        onInsertPrompt,
        onUseOllama,
        onOpenSettings,
      });

      // Onboarding alert buttons
      const ollamaBtn = screen.getByRole("button", { name: /use local ollama/i });
      await fireEvent.click(ollamaBtn);
      expect(onUseOllama).toHaveBeenCalledTimes(1);

      const settingsBtn = screen.getByRole("button", { name: /configure provider/i });
      await fireEvent.click(settingsBtn);
      expect(onOpenSettings).toHaveBeenCalledTimes(1);

      // Click first warning prompt card
      const promptCards = screen.getAllByRole("button", { name: /starter prompt/i });
      await fireEvent.click(promptCards[0]);
      expect(onSelectPrompt).toHaveBeenCalledWith(prompts[0].prompt);
    });

    it("renders ToolStepCard and handles Approve and Deny callbacks in-place", async () => {
      const onApprove = vi.fn();
      const onDeny = vi.fn();

      const stepPayload: ToolStepPayload = {
        version: 1,
        kind: "command",
        name: "prune_volumes",
        title: "Prune Docker Volumes",
        safety: "DANGEROUS",
        status: "pending_approval",
        rawInput: JSON.stringify({ force: true }),
      };

      const { container } = render(ToolStepCard, {
        step: stepPayload,
        onApprove,
        onDeny,
      });

      // Header and safety badge
      expect(container.querySelector(".safety-badge.safety-dangerous")).toBeInTheDocument();
      expect(screen.getByText("Prune Docker Volumes")).toBeInTheDocument();

      // Approve button
      const approveBtn = screen.getByRole("button", { name: /approve prune docker volumes/i });
      await fireEvent.click(approveBtn);
      expect(onApprove).toHaveBeenCalledWith("prune_volumes");

      // Deny button
      const denyBtn = screen.getByRole("button", { name: /deny prune docker volumes/i });
      await fireEvent.click(denyBtn);
      expect(onDeny).toHaveBeenCalledWith("prune_volumes");

      // Toggle payload details
      const payloadToggle = container.querySelector(".tool-payload-toggle") as HTMLButtonElement;
      await fireEvent.click(payloadToggle);
      expect(container.querySelector(".tool-payload-body")).toBeInTheDocument();
    });

    it("derives contextual fallback suggestions with choice card suppression", () => {
      // When choice card is present, fallback suggestions are suppressed
      const withChoice = deriveFallbackSuggestions({
        currentPage: "containers",
        hasChoiceCard: true,
      });
      expect(withChoice).toHaveLength(0);

      // Tab-specific fallback for containers
      const containerFallbacks = deriveFallbackSuggestions({
        currentPage: "containers",
      });
      expect(containerFallbacks.length).toBeGreaterThanOrEqual(2);

      // Tool-specific fallback for logs
      const logFallbacks = deriveFallbackSuggestions({
        toolsExecuted: ["docker logs nginx"],
      });
      expect(logFallbacks.some(([label]) => label.toLowerCase().includes("log") || label.toLowerCase().includes("error"))).toBe(true);
    });

    it("generates concise live UI context snapshots for prompt injection", () => {
      const snapshot = buildLiveUiContextSnapshot({
        currentPage: "containers",
        language: "en",
        instances: [{ name: "default", status: "Running", cpus: 4, memory: 4 } as never],
        containers: [
          { Id: "c1", State: "running" } as never,
          { Id: "c2", State: "exited" } as never,
        ],
        k8sStatus: { connected: true, pods: 12, namespaces: 3 },
      });

      expect(snapshot).toContain("[LIVE_UI_CONTEXT]");
      expect(snapshot).toContain("Current Page: containers");
      expect(snapshot).toContain("Colima VMs: default: Running (4 CPU, 4GiB)");
      expect(snapshot).toContain("Containers: 1 running, 1 stopped");
      expect(snapshot).toContain("Kubernetes: connected (12 pods, 3 namespaces)");
    });
  });

  // =========================================================================
  // 4. Extreme Input Sizes & ReDoS Immunity
  // =========================================================================
  describe("4. Extreme Input Sizes & ReDoS Immunity", () => {
    it("parses 100,000+ character markdown documents with 5,000 lines in < 300ms", () => {
      const lineTypes = [
        "# Section Header",
        "This is a paragraph with **bold** and *italic* and `code` formatting.",
        "> Important blockquote with details.",
        "- List item with [link](https://colima.run)",
        "| Header 1 | Header 2 | Header 3 |",
        "|:---|:---:|---:|",
        "| Cell 1 | Cell 2 | Cell 3 |",
        "```bash\ndocker run -d alpine\n```",
      ];

      const lines: string[] = [];
      for (let i = 0; i < 5000; i++) {
        lines.push(lineTypes[i % lineTypes.length]);
      }
      const massiveDoc = lines.join("\n");
      expect(massiveDoc.length).toBeGreaterThan(100000);

      const startTime = performance.now();
      const blocks = parseChatMarkdown(massiveDoc);
      const elapsed = performance.now() - startTime;

      expect(blocks.length).toBeGreaterThan(1000);
      expect(elapsed).toBeLessThan(300);
    });

    it("exhibits ReDoS immunity under deeply nested markdown patterns in < 50ms", () => {
      // 5,000 consecutive asterisks
      const asterisks = "*".repeat(5000);
      const t1 = performance.now();
      const formattedAsterisks = formatInline(asterisks);
      const e1 = performance.now() - t1;
      expect(formattedAsterisks).toBeDefined();
      expect(e1).toBeLessThan(50);

      // 5,000 consecutive unclosed brackets
      const brackets = "[".repeat(5000);
      const t2 = performance.now();
      const formattedBrackets = formatInline(brackets);
      const e2 = performance.now() - t2;
      expect(formattedBrackets).toBeDefined();
      expect(e2).toBeLessThan(50);

      // 5,000 consecutive pipes
      const pipes = "|".repeat(5000);
      const t3 = performance.now();
      const choicePrompt = extractChoicePrompt(`[ASK_CHOICE: Q ${pipes}]`);
      const e3 = performance.now() - t3;
      expect(choicePrompt).toBeDefined();
      expect(e3).toBeLessThan(50);

      // Long unclosed streaming tag prefix
      const longTail = "A".repeat(50000) + " [QUERY: ";
      const t4 = performance.now();
      const stripped = stripStreamingPartialTags(longTail);
      const e4 = performance.now() - t4;
      expect(stripped.pendingTagKind).toBe("query");
      expect(e4).toBeLessThan(50);
    });

    it("sanitizes malicious URLs and escapes dangerous HTML payloads", () => {
      expect(sanitizeUrl("javascript:alert(1)")).toBe("#");
      expect(sanitizeUrl("data:text/html,<script>alert(1)</script>")).toBe("#");
      expect(sanitizeUrl("vbscript:msgbox(1)")).toBe("#");
      expect(sanitizeUrl("https://github.com/abiosoft/colima")).toBe("https://github.com/abiosoft/colima");

      const dangerousHtml = "<script>alert('XSS')</script>&<img src=x onerror=alert(1)>";
      const escaped = escapeHtml(dangerousHtml);
      expect(escaped).not.toContain("<script>");
      expect(escaped).toContain("&lt;script&gt;");
      expect(escaped).toContain("&amp;");
      expect(escaped).toContain("&#039;XSS&#039;");
    });
  });

  // =========================================================================
  // 5. Confirmation-Gated Chat Clear Under Populated vs Empty States
  // =========================================================================
  describe("5. Confirmation-Gated Chat Clear Under Populated vs Empty States", () => {
    it("returns immediately without confirmation dialog or clear history when messages are empty", async () => {
      aiState.messages = [];

      const { container } = render(AiChatPanel);
      const clearBtn = container.querySelector("button[title*='Clear this chat']") as HTMLButtonElement;
      expect(clearBtn).toBeInTheDocument();

      await fireEvent.click(clearBtn);

      // Confirmation dialog must NOT be invoked
      expect(confirm).not.toHaveBeenCalled();
      // Backend clearHistory must NOT be called
      expect(aiApi.clearHistory).not.toHaveBeenCalled();
    });

    it("prompts confirmation when messages are populated, aborting if user cancels", async () => {
      aiState.messages = [
        { id: "m1", role: "user", content: "How do I start Colima?" },
        { id: "m2", role: "assistant", content: "Run `colima start`." },
      ];

      vi.mocked(confirm).mockResolvedValueOnce(false);

      const { container } = render(AiChatPanel);
      const clearBtn = container.querySelector("button[title*='Clear this chat']") as HTMLButtonElement;
      await fireEvent.click(clearBtn);

      // Confirmation dialog was shown with danger styling
      expect(confirm).toHaveBeenCalledWith(
        expect.objectContaining({
          variant: "danger",
        })
      );

      // Since user cancelled, clearHistory was NOT called
      expect(aiApi.clearHistory).not.toHaveBeenCalled();
      // Messages remain intact
      expect(aiState.messages).toHaveLength(2);
    });

    it("clears chat history when messages are populated and user confirms", async () => {
      aiState.messages = [
        { id: "m1", role: "user", content: "Check memory usage" },
      ];

      vi.mocked(confirm).mockResolvedValueOnce(true);

      const { container } = render(AiChatPanel);
      const clearBtn = container.querySelector("button[title*='Clear this chat']") as HTMLButtonElement;
      await fireEvent.click(clearBtn);

      expect(confirm).toHaveBeenCalledTimes(1);
      expect(aiApi.clearHistory).toHaveBeenCalledTimes(1);
    });

    it("falls back to window.confirm when confirm dialog rejects", async () => {
      aiState.messages = [
        { id: "m1", role: "user", content: "Test question" },
      ];

      vi.mocked(confirm).mockRejectedValueOnce(new Error("Dialog mount failure"));
      const windowConfirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

      const { container } = render(AiChatPanel);
      const clearBtn = container.querySelector("button[title*='Clear this chat']") as HTMLButtonElement;
      await fireEvent.click(clearBtn);

      expect(windowConfirmSpy).toHaveBeenCalled();
      expect(aiApi.clearHistory).not.toHaveBeenCalled();

      windowConfirmSpy.mockRestore();
    });
  });
});

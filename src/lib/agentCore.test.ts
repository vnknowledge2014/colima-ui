import { describe, expect, it, vi } from "vitest";
import * as aiEventBus from "./aiEventBus";
import * as llmProviders from "./llmProviders";
import {
  buildCleanConversationWindow,
  capToolOutput,
  isApprovalRequiredError,
  MAX_SEARCH_ROUNDS,
  normalizeToolCallKey,
  runAgent,
  type AgentCallbacks,
} from "./agentCore";
import { parseToolStep } from "./aiToolStep";

describe("agentCore helpers (DeepSeek-Harness & Harness2 patterns)", () => {
  it("increases MAX_SEARCH_ROUNDS to 5 for multi-step diagnosis and remediation", () => {
    expect(MAX_SEARCH_ROUNDS).toBe(5);
  });

  it("capToolOutput preserves short outputs verbatim and head+tail prunes oversized outputs", () => {
    const shortText = "container nginx is running on port 8080";
    expect(capToolOutput(shortText, 200)).toBe(shortText);

    const head = "HEAD_MARKER_" + "A".repeat(120);
    const middle = "M".repeat(500);
    const tail = "B".repeat(40) + "_TAIL_MARKER";
    const oversized = head + middle + tail;

    const pruned = capToolOutput(oversized, 100);
    expect(pruned.startsWith("HEAD_MARKER_")).toBe(true);
    expect(pruned.endsWith("_TAIL_MARKER")).toBe(true);
    expect(pruned).toContain("chars omitted (head+tail pruned)");
    expect(pruned.length).toBeLessThan(oversized.length);
  });

  it("normalizeToolCallKey collapses whitespace, normalizes kind case, and canonicalizes JSON key order", () => {
    expect(normalizeToolCallKey("run", "  docker   ps   -a  ")).toBe("RUN:docker ps -a:");
    expect(normalizeToolCallKey("query", "docker:containers", ' {"all":true} ')).toBe(
      'QUERY:docker:containers:{"all":true}',
    );
    expect(
      normalizeToolCallKey("query", "docker:logs", '{"tail": 50, "id": "abc"}'),
    ).toBe(normalizeToolCallKey("query", "docker:logs", '{"id":"abc","tail":50}'));
  });

  it("isApprovalRequiredError detects shell_sandbox NeedsApproval rejections", () => {
    expect(
      isApprovalRequiredError(
        new Error("Command 'colima' requires user approval. Use execute_approved_command after confirmation."),
      ),
    ).toBe(true);
    expect(isApprovalRequiredError("Command 'kubectl' requires user approval.")).toBe(true);
    expect(isApprovalRequiredError(new Error("Command contains disallowed shell operators"))).toBe(
      false,
    );
  });

  it("runAgent clears seenToolCalls after state-mutating approval and preserves cleanText without raw tool tags", async () => {
    let round = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, _hist, _endpoint, onChunk) => {
        round += 1;
        if (round === 1) {
          onChunk(
            'Checking & starting container...\n[QUERY: docker:containers | {"all": true}]\n[EVENT_APPROVE: docker:start | {"id": "c1"}]',
          );
        } else if (round === 2) {
          // Re-queries the exact same SAFE event after state mutation, with no prose (only tool tag)
          onChunk('[QUERY: docker:containers | {"all":true}]');
        } else {
          onChunk("Container c1 is now running.");
        }
      },
    );

    vi.spyOn(aiEventBus, "getCategory").mockImplementation((evt) =>
      evt === "docker:containers" ? "SAFE" : "NORMAL",
    );
    const executeSpy = vi
      .spyOn(aiEventBus, "executeEvent")
      .mockImplementation(async (evt) => {
        if (evt === "docker:containers") {
          return round === 1 ? "[]" : '[{"id":"c1","state":"running"}]';
        }
        return "started c1";
      });

    const messages = new Map<string, string>();
    const deletedIds = new Set<string>();
    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: (m) => {
        if (m.role === "assistant") messages.set(m.id, m.content);
      },
      onMessageUpdate: (id, content) => {
        messages.set(id, content);
      },
      onMessageDelete: (id) => {
        deletedIds.add(id);
        messages.delete(id);
      },
      onApprovalRequired: async () => true,
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Start c1 and verify",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    // docker:containers was executed in Round 1 AND allowed to re-execute in Round 2 after docker:start
    const containerQueries = executeSpy.mock.calls.filter((c) => c[0] === "docker:containers");
    expect(containerQueries.length).toBe(2);

    // Round 2 had empty cleanText (`[QUERY: ...]`), so its streamId was deleted and NEVER resurrected by a late onMessageUpdate
    expect(deletedIds.size).toBe(1);
    for (const id of deletedIds) {
      expect(messages.has(id)).toBe(false);
    }

    // Round 1 assistant message kept cleanText without raw `[QUERY:` or `[EVENT_APPROVE:` tags
    const assistantContents = [...messages.values()];
    expect(assistantContents).toContain("Checking & starting container...");
    expect(assistantContents).toContain("Container c1 is now running.");
    for (const text of assistantContents) {
      expect(text).not.toContain("[QUERY:");
      expect(text).not.toContain("[EVENT_APPROVE:");
    }

    vi.restoreAllMocks();
  });

  it("runAgent persists [LEARN], [LEARN_REASONING], and [REMEMBER_PREFERENCE] via knowledgeBankApi without wasting an extra LLM round", async () => {
    const { knowledgeBankApi } = await import("./api/knowledgeBank");
    const learnSpy = vi.spyOn(knowledgeBankApi, "learn").mockResolvedValue(1);
    const addMemorySpy = vi.spyOn(knowledgeBankApi, "addMemory").mockResolvedValue("Memory added");
    vi.spyOn(knowledgeBankApi, "query").mockResolvedValue({ context_text: "" });
    vi.spyOn(knowledgeBankApi, "searchMemory").mockResolvedValue([]);

    let llmCalls = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, _hist, _endpoint, onChunk) => {
        llmCalls += 1;
        onChunk(
          "Got it, I will remember that.\n[LEARN: vz disk lock | pkill limactl usernet]\n[LEARN_REASONING: high memory usage | reduce VM memory to 4GB]\n[REMEMBER_PREFERENCE: Always use dark terminal theme]",
        );
      },
    );

    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: () => {},
      onMessageUpdate: () => {},
      onMessageDelete: () => {},
      onApprovalRequired: async () => true,
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Remember my preference and those lessons",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    // Side-effect memory tags must not trigger a second empty [Tool Results] LLM round
    expect(llmCalls).toBe(1);
    expect(learnSpy).toHaveBeenCalledWith("vz disk lock", "pkill limactl usernet");
    expect(learnSpy).toHaveBeenCalledWith("high memory usage", "reduce VM memory to 4GB");
    expect(addMemorySpy).toHaveBeenCalledWith(
      "reasoning",
      "high memory usage | reduce VM memory to 4GB",
    );
    expect(addMemorySpy).toHaveBeenCalledWith(
      "preference",
      "Always use dark terminal theme",
    );

    vi.restoreAllMocks();
  });

  it("runAgent emits a single in-place ToolStep envelope for [EVENT_APPROVE] across pending_approval -> running -> succeeded and denied", async () => {
    let round = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, _hist, _endpoint, onChunk) => {
        round += 1;
        if (round === 1) {
          onChunk(
            'Proposing two actions...\n[EVENT_APPROVE: start-instance | {"profile":"default"}]\n[EVENT_APPROVE: stop-instance | {"profile":"dev"}]',
          );
        } else {
          onChunk("Handled both actions.");
        }
      },
    );

    vi.spyOn(aiEventBus, "getCategory").mockImplementation((evt) =>
      evt === "stop-instance" ? "DANGEROUS" : "NORMAL",
    );
    vi.spyOn(aiEventBus, "executeEvent").mockResolvedValue("instance default started");

    const systemSteps = new Map<string, string[]>();
    const approvalIds: string[] = [];

    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: (m) => {
        if (m.role === "system") {
          systemSteps.set(m.id, [m.content]);
        }
      },
      onMessageUpdate: (id, content) => {
        const history = systemSteps.get(id);
        if (history) history.push(content);
      },
      onMessageDelete: () => {},
      onApprovalRequired: async (id) => {
        approvalIds.push(id);
        // Approve the first (start-instance), deny the second (stop-instance)
        return approvalIds.length === 1;
      },
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Manage VMs",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    expect(approvalIds).toHaveLength(2);
    // Exactly 2 system messages created (one per tool step, updated in-place — no extra spam bubbles!)
    expect(systemSteps.size).toBe(2);

    const firstTransitions = systemSteps.get(approvalIds[0])!.map((c) => parseToolStep(c));
    expect(firstTransitions.map((s) => s?.status)).toEqual([
      "pending_approval",
      "running",
      "succeeded",
    ]);
    expect(firstTransitions[0]?.id).toBe(approvalIds[0]);
    expect(firstTransitions[2]?.output).toContain("instance default started");

    const secondTransitions = systemSteps.get(approvalIds[1])!.map((c) => parseToolStep(c));
    expect(secondTransitions.map((s) => s?.status)).toEqual([
      "pending_approval",
      "denied",
    ]);
    expect(secondTransitions[0]?.safety).toBe("DANGEROUS");
    expect(secondTransitions[0]?.warning).toBeTruthy();

    vi.restoreAllMocks();
  });
});

describe("buildCleanConversationWindow (Milestone 3)", () => {
  it("deduplicates userMessage if caller already pushed it to the end of chatHistory", () => {
    const history = [
      { role: "user", content: "hello" },
      { role: "assistant", content: "Hi! How can I help?" },
      { role: "user", content: "show containers" },
    ];
    const result = buildCleanConversationWindow(history, "show containers", 10);
    expect(result).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: "Hi! How can I help?" },
      { role: "user", content: "show containers" },
    ]);
  });

  it("appends userMessage if caller did not push it to chatHistory", () => {
    const history = [
      { role: "user", content: "hello" },
      { role: "assistant", content: "Hi! How can I help?" },
    ];
    const result = buildCleanConversationWindow(history, "show containers", 10);
    expect(result).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: "Hi! How can I help?" },
      { role: "user", content: "show containers" },
    ]);
  });

  it("merges consecutive messages with the same role using \\n\\n for strict alternation", () => {
    const history = [
      { role: "user", content: "part 1" },
      { role: "user", content: "part 2" },
      { role: "assistant", content: "ans 1" },
      { role: "assistant", content: "ans 2" },
      { role: "user", content: "part 3" },
    ];
    const result = buildCleanConversationWindow(history, "part 3", 10);
    expect(result).toEqual([
      { role: "user", content: "part 1\n\npart 2" },
      { role: "assistant", content: "ans 1\n\nans 2" },
      { role: "user", content: "part 3" },
    ]);
  });

  it("removes leading assistant messages after slicing so first message is guaranteed to have role user (Anthropic requirement)", () => {
    const history = [
      { role: "user", content: "U1" },
      { role: "assistant", content: "A1" },
      { role: "user", content: "U2" },
      { role: "assistant", content: "A2" },
      { role: "user", content: "U3" },
      { role: "assistant", content: "A3" },
      { role: "user", content: "U4" },
      { role: "assistant", content: "A4" },
    ];

    // maxTurns = 4: Slicing 4 turns would yield [A3, U4, A4, U5].
    // Leading assistant message A3 must be removed so that the first message sent to Anthropic is U4!
    const result = buildCleanConversationWindow(history, "U5", 4);
    expect(result[0].role).toBe("user");
    expect(result).toEqual([
      { role: "user", content: "U4" },
      { role: "assistant", content: "A4" },
      { role: "user", content: "U5" },
    ]);
  });

  it("skips system messages, empty messages, and cleans old tool tags from assistant turns", () => {
    const history = [
      { role: "system", content: "System context" },
      { role: "user", content: "start container" },
      { role: "assistant", content: "Starting it now.\n[SUGGEST: view logs | show logs]\n[ASK_CHOICE: Which one? | a | b]" },
      { role: "assistant", content: "[QUERY: docker:ps]" }, // only tool tag -> cleanText empty -> skipped
      { role: "user", content: "   " }, // empty user message -> skipped
      { role: "assistant", content: "Done!" },
    ];
    const result = buildCleanConversationWindow(history, "thanks", 10);
    expect(result).toEqual([
      { role: "user", content: "start container" },
      { role: "assistant", content: "Starting it now.\n\nDone!" },
      { role: "user", content: "thanks" },
    ]);
  });

  it("returns single user turn if history is empty or only had assistant messages", () => {
    const resultEmpty = buildCleanConversationWindow([], "hello", 10);
    expect(resultEmpty).toEqual([{ role: "user", content: "hello" }]);

    const resultOnlyAssistant = buildCleanConversationWindow(
      [{ role: "assistant", content: "Welcome to Colima AI!" }],
      "help me",
      10,
    );
    expect(resultOnlyAssistant).toEqual([{ role: "user", content: "help me" }]);
  });
});

describe("agentCore intermediate draft bubble revocation (Milestone 3)", () => {
  it("revokes intermediate draft bubble from read-only tool round when subsequent round produces prose", async () => {
    let round = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, _hist, _endpoint, onChunk) => {
        round += 1;
        if (round === 1) {
          // Read-only query round with draft prose
          onChunk("Inspecting docker environment...\n[QUERY: docker:containers | {}]");
        } else {
          // Round 2 produces real prose response
          onChunk("Found 2 running containers.");
        }
      },
    );

    vi.spyOn(aiEventBus, "getCategory").mockImplementation(() => "SAFE");
    vi.spyOn(aiEventBus, "executeEvent").mockResolvedValue('[{"id":"c1"},{"id":"c2"}]');

    const messages = new Map<string, string>();
    const deletedIds = new Set<string>();
    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: (m) => {
        messages.set(m.id, m.content);
      },
      onMessageUpdate: (id, content) => {
        messages.set(id, content);
      },
      onMessageDelete: (id) => {
        deletedIds.add(id);
        messages.delete(id);
      },
      onApprovalRequired: async () => true,
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Check containers",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    // Round 1 draft streamId was deleted when Round 2 emitted real prose
    expect(deletedIds.size).toBe(1);
    const remainingContents = [...messages.values()];
    // The speculative text was revoked and replaced
    expect(remainingContents).not.toContain("Inspecting docker environment...");
    expect(remainingContents).toContain("Found 2 running containers.");

    vi.restoreAllMocks();
  });

  it("preserves pending draft bubble when intermediate round has only tool tags, and revokes when prose arrives", async () => {
    let round = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, _hist, _endpoint, onChunk) => {
        round += 1;
        if (round === 1) {
          // Round 1: prose + read-only tool
          onChunk("Checking status...\n[QUERY: docker:containers | {}]");
        } else if (round === 2) {
          // Round 2: ONLY tool tag, no prose at all
          onChunk("[QUERY: docker:containers | {}]");
        } else {
          // Round 3: real prose arrives
          onChunk("All containers healthy.");
        }
      },
    );

    vi.spyOn(aiEventBus, "getCategory").mockImplementation(() => "SAFE");
    vi.spyOn(aiEventBus, "executeEvent").mockResolvedValue("[]");

    const messages = new Map<string, string>();
    const deletedIds = new Set<string>();
    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: (m) => {
        messages.set(m.id, m.content);
      },
      onMessageUpdate: (id, content) => {
        messages.set(id, content);
      },
      onMessageDelete: (id) => {
        deletedIds.add(id);
        messages.delete(id);
      },
      onApprovalRequired: async () => true,
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Check status",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    // Round 2's empty streamId was deleted immediately because it had no prose.
    // Round 1's draft was NOT overwritten with Round 2's deleted id, so Round 1's draft
    // was correctly deleted when Round 3 emitted prose!
    expect(deletedIds.size).toBe(2);
    const remainingContents = [...messages.values()];
    expect(remainingContents).not.toContain("Checking status...");
    expect(remainingContents).toContain("All containers healthy.");

    vi.restoreAllMocks();
  });

  it("does not revoke draft message when round awaited user approval", async () => {
    let round = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, _hist, _endpoint, onChunk) => {
        round += 1;
        if (round === 1) {
          // Round 1: awaits approval
          onChunk('I will delete container c1 now.\n[EVENT_APPROVE: docker:remove | {"id":"c1"}]');
        } else {
          // Round 2: final confirmation
          onChunk("Container c1 was deleted successfully.");
        }
      },
    );

    vi.spyOn(aiEventBus, "getCategory").mockImplementation(() => "NORMAL");
    vi.spyOn(aiEventBus, "executeEvent").mockResolvedValue("deleted c1");

    const messages = new Map<string, string>();
    const deletedIds = new Set<string>();
    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: (m) => {
        messages.set(m.id, m.content);
      },
      onMessageUpdate: (id, content) => {
        messages.set(id, content);
      },
      onMessageDelete: (id) => {
        deletedIds.add(id);
        messages.delete(id);
      },
      onApprovalRequired: async () => true,
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Delete c1",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    // Round 1 required approval, so its assistant text is an interactive prompt and must NOT be revoked!
    expect(deletedIds.size).toBe(0);
    const remainingContents = [...messages.values()];
    expect(remainingContents).toContain("I will delete container c1 now.");
    expect(remainingContents).toContain("Container c1 was deleted successfully.");

    vi.restoreAllMocks();
  });

  it("instructs subsequent round to provide a self-contained answer and revokes interactive-only read-only draft bubbles", async () => {
    const capturedHistories: unknown[][] = [];
    let round = 0;
    vi.spyOn(llmProviders, "chatStream").mockImplementation(
      async (_prov, _model, _key, hist, _endpoint, onChunk) => {
        capturedHistories.push(JSON.parse(JSON.stringify(hist)));
        round += 1;
        if (round === 1) {
          onChunk("[SUGGEST: Check logs | docker logs]\n[QUERY: docker:containers | {}]");
        } else {
          onChunk("Complete self-contained summary of 2 running containers.");
        }
      },
    );

    vi.spyOn(aiEventBus, "getCategory").mockImplementation(() => "SAFE");
    vi.spyOn(aiEventBus, "executeEvent").mockResolvedValue('[{"id":"c1"},{"id":"c2"}]');

    const messages = new Map<string, string>();
    const deletedIds = new Set<string>();
    const callbacks: AgentCallbacks = {
      onStatus: () => {},
      onMessage: (m) => {
        messages.set(m.id, m.content);
      },
      onMessageUpdate: (id, content) => {
        messages.set(id, content);
      },
      onMessageDelete: (id) => {
        deletedIds.add(id);
        messages.delete(id);
      },
      onApprovalRequired: async () => true,
      onNavigate: () => {},
      onTimerSchedule: () => {},
      onCronSchedule: () => {},
      onScheduleCancel: () => {},
    };

    await runAgent(
      "Check containers",
      { provider: "ollama-local", model: "qwen2.5", apiKey: "", endpoint: "http://127.0.0.1:11434" },
      callbacks,
      [],
      "",
      [],
    );

    expect(deletedIds.size).toBe(1);
    const round2History = capturedHistories[1] as Array<{ role: string; content: string }>;
    const lastPrompt = round2History[round2History.length - 1];
    expect(lastPrompt.content).toContain("self-contained response");

    vi.restoreAllMocks();
  });
});

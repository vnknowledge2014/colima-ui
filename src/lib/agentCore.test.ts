import { describe, expect, it, vi } from "vitest";
import * as aiEventBus from "./aiEventBus";
import * as llmProviders from "./llmProviders";
import {
  capToolOutput,
  isApprovalRequiredError,
  MAX_SEARCH_ROUNDS,
  normalizeToolCallKey,
  runAgent,
  type AgentCallbacks,
} from "./agentCore";

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
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  cleanToolSyntaxRemnants,
  stripStreamingPartialTags,
  parseAiTools,
} from "../lib/aiToolParser";
import { buildCleanConversationWindow } from "../lib/agentCore";
import { parseChatMarkdown } from "../lib/chatMarkdown";

vi.mock("@tauri-apps/plugin-http", () => {
  throw new Error("not available in tests");
});

function createMockStreamResponse(chunks: string[]): Response {
  const encoder = new TextEncoder();
  let index = 0;
  const body = {
    getReader() {
      return {
        async read() {
          if (index < chunks.length) {
            const chunk = chunks[index++];
            return { done: false, value: encoder.encode(chunk) };
          }
          return { done: true, value: undefined };
        },
        async cancel() {},
      };
    },
  };
  return {
    ok: true,
    status: 200,
    body,
  } as unknown as Response;
}

describe("Adversarial Stress Testing: Empirical Verification", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.resetModules();
  });

  // =========================================================================
  // 1. Tool Syntax Cleaning & Streaming Partial Tags
  // =========================================================================
  describe("1. Tool Syntax Cleaning (cleanToolSyntaxRemnants & stripStreamingPartialTags)", () => {
    it("1.1 Code blocks: preserves standard triple backticks and tilde fences", () => {
      // Standard triple backticks with empty backticks and code inside
      const standardCodeBlock = [
        "Here is documentation with code:",
        "```javascript",
        "const x = ``;",
        "const y = `- not a bullet`;",
        "```",
        "End of documentation.",
      ].join("\n");

      const cleaned = cleanToolSyntaxRemnants(standardCodeBlock);
      expect(cleaned).toContain("const x = ``;");
      expect(cleaned).toContain("const y = `- not a bullet`;");
      expect(cleaned).toContain("```javascript");
      expect(cleaned).toContain("End of documentation.");

      // Triple tildes enclosing triple backticks and empty backticks
      const tildeEnclosed = [
        "Example config:",
        "~~~yaml",
        "command: ```bash colima status```",
        "empty_backtick: ``",
        "diff_marker: -",
        "~~~",
        "Post config note.",
      ].join("\n");

      const cleanedTilde = cleanToolSyntaxRemnants(tildeEnclosed);
      expect(cleanedTilde).toContain("empty_backtick: ``");
      expect(cleanedTilde).toContain("command: ```bash colima status```");
      expect(cleanedTilde).toContain("diff_marker: -");
    });

    it("1.2 Markdown tables: handles empty cells, backticks and list-like pipes safely", () => {
      const tableWithBackticks = [
        "| Service | Status | Backtick | Diff Marker |",
        "|---------|--------|----------|-------------|",
        "| docker  | ``     | ``       | -           |",
        "| k8s     | active | ``       | +           |",
      ].join("\n");

      const cleaned = cleanToolSyntaxRemnants(tableWithBackticks);
      // Empty backticks in table cells should be removed cleanly without corrupting the table row structure
      expect(cleaned).toContain("| docker  |");
      expect(cleaned).toContain("| k8s     |");
      // Table pipes and diff signs inside table cells must NOT be stripped
      expect(cleaned).toContain("| -           |");
      expect(cleaned).toContain("| +           |");
    });

    it("1.3 Git diffs with `- ` markers: preserves diffs inside fences, cleans orphaned markers outside", () => {
      // Git diff inside code block
      const diffInsideCode = [
        "```diff",
        "--- a/colima.ts",
        "+++ b/colima.ts",
        "@@ -1,4 +1,4 @@",
        "- const oldConfig = true;",
        "+ const newConfig = false;",
        "-",
        "```",
      ].join("\n");

      const cleanedDiff = cleanToolSyntaxRemnants(diffInsideCode);
      expect(cleanedDiff).toBe(diffInsideCode);

      // Git diff outside code block: content lines preserved, bare '-' line cleaned
      const diffOutsideCode = [
        "Diff summary:",
        "- const oldConfig = true;",
        "+ const newConfig = false;",
        "-",
        "End of diff.",
      ].join("\n");

      const cleanedOutside = cleanToolSyntaxRemnants(diffOutsideCode);
      expect(cleanedOutside).toContain("- const oldConfig = true;");
      expect(cleanedOutside).toContain("+ const newConfig = false;");
      // The standalone '-' orphaned bullet is removed
      const lines = cleanedOutside.split("\n").map((l) => l.trim());
      expect(lines).not.toContain("-");
    });

    it("1.4 Unclosed tags: stripStreamingPartialTags cleans tail across multiple tag kinds", () => {
      const kinds = [
        { raw: "[QUERY: colima status --json", expectedKind: "query" },
        { raw: "[RUN: docker compose up -d", expectedKind: "run" },
        { raw: '[ASK_CHOICE: {"question": "Restart now?"', expectedKind: "choice" },
        { raw: "[SUGGEST: Check container logs", expectedKind: "suggest" },
        { raw: "[DIAGNOSE", expectedKind: "diagnose" },
        { raw: "[SECURITY_VULN_SCAN: colima-vm", expectedKind: "security" },
        { raw: "[SCHEDULE_TIMER: 5000 | remind me", expectedKind: "schedule" },
        { raw: "[NAVIGATE: /containers", expectedKind: "navigate" },
        { raw: "[LEARN: user prefers vz vmType", expectedKind: "memory" },
      ];

      for (const { raw, expectedKind } of kinds) {
        const input = `Processing step:\n- \`${raw}`;
        const res = stripStreamingPartialTags(input);
        expect(res.pendingTagKind).toBe(expectedKind);
        // Trailing bullet and trailing opening backtick are cleanly stripped to prevent flicker
        expect(res.cleanText).toBe("Processing step:");
      }
    });

    it("1.5 Empty backticks: eliminates empty pairs and whitespace backticks outside code", () => {
      const input = [
        "Here is empty: `` and spaces: `   ` and inline: `valid code`.",
        "- ``",
        "* ` `",
        "1. ``",
        "Normal text continuation.",
      ].join("\n");

      const cleaned = cleanToolSyntaxRemnants(input);
      expect(cleaned).toContain("inline: `valid code`.");
      expect(cleaned).not.toContain("``");
      expect(cleaned).not.toContain("`   `");
      // Orphaned bullets with empty backticks are fully eliminated
      expect(cleaned).not.toMatch(/^\s*[-*+]\s*$/m);
      expect(cleaned).not.toMatch(/^\s*\d+\.\s*$/m);
    });

    it("1.6 Multiple tags on one line: parses all tags and leaves clean prose", () => {
      const input = [
        "Executing tasks: [QUERY: colima status] [QUERY: docker ps] [RUN: colima start]",
        "- `[QUERY: k8s get nodes]` [SUGGEST: Quick restart]",
        "Completed all commands.",
      ].join("\n");

      const parsed = parseAiTools(input);
      expect(parsed.queries.length).toBe(3);
      expect(parsed.runs.length).toBe(1);
      expect(parsed.suggests.length).toBe(1);
      expect(parsed.cleanText).toContain("Executing tasks:");
      expect(parsed.cleanText).toContain("Completed all commands.");
      // No stray brackets or empty backticks remaining
      expect(parsed.cleanText).not.toContain("[QUERY:");
      expect(parsed.cleanText).not.toContain("[RUN:");
      expect(parsed.cleanText).not.toContain("``");
    });
  });

  // =========================================================================
  // 2. SSE Stream & <think> Reasoning Blocks
  // =========================================================================
  describe("2. SSE Stream & <think> Reasoning Blocks", () => {
    it("2.1 Fragmented chunks: thinking block split across 10 single-character packets", async () => {
      // 10 single-character packets inside reasoning_content for OpenAI format
      const charPackets = ["T", "h", "i", "n", "k", "i", "n", "g", "!", "."];
      const sseEvents = charPackets.map(
        (c) => `data: {"choices":[{"delta":{"reasoning_content":${JSON.stringify(c)}}}]}\n\n`
      );
      // Followed by answer content
      sseEvents.push('data: {"choices":[{"delta":{"content":"Done!"}}]}\n\n');
      sseEvents.push("data: [DONE]\n\n");

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(sseEvents))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "openai",
        "gpt-4o-mini",
        "mock-key",
        [{ role: "user", content: "test" }],
        "https://api.openai.com/v1",
        (c) => chunks.push(c)
      );

      const fullOutput = chunks.join("");
      expect(fullOutput).toBe("<think>Thinking!.</think>\n\nDone!");
      // Verified: <think> was opened only once, and </think>\n\n closed it before content
      expect(chunks.filter((c) => c === "<think>").length).toBe(1);
      expect(chunks.filter((c) => c === "</think>\n\n").length).toBe(1);
    });

    it("2.2 Wire-level fragmented bytes: line buffer handles JSON split byte-by-byte", async () => {
      const fullLine = 'data: {"choices":[{"delta":{"content":"Byte stream OK"}}]}\n\n';
      // Split into 1-character chunks at the transport stream level
      const singleByteChunks = fullLine.split("");

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(singleByteChunks))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "openai",
        "gpt-4o-mini",
        "mock-key",
        [{ role: "user", content: "test" }],
        "https://api.openai.com/v1",
        (c) => chunks.push(c)
      );

      expect(chunks.join("")).toBe("Byte stream OK");
    });

    it("2.3 Thinking block followed immediately by text in same response", async () => {
      const events = [
        'data: {"choices":[{"delta":{"reasoning":"Analyzing memory..."}}]}\n\n',
        'data: {"choices":[{"delta":{"content":"Memory is stable at 4GB."}}]}\n\n',
        "data: [DONE]\n\n",
      ];

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(events))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "openai",
        "gpt-4o-mini",
        "mock-key",
        [{ role: "user", content: "test" }],
        "https://api.openai.com/v1",
        (c) => chunks.push(c)
      );

      expect(chunks.join("")).toBe(
        "<think>Analyzing memory...</think>\n\nMemory is stable at 4GB."
      );
    });

    it("2.4 Thinking block without text: closes thinking cleanly at stream end", async () => {
      const events = [
        'data: {"choices":[{"delta":{"reasoning_content":"Silent calculation."}}]}\n\n',
        "data: [DONE]\n\n",
      ];

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(events))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "openai",
        "gpt-4o-mini",
        "mock-key",
        [{ role: "user", content: "test" }],
        "https://api.openai.com/v1",
        (c) => chunks.push(c)
      );

      expect(chunks.join("")).toBe("<think>Silent calculation.</think>\n\n");
    });

    it("2.5 Text without thinking block: emits content without any think tags", async () => {
      const events = [
        'data: {"choices":[{"delta":{"content":"Direct answer."}}]}\n\n',
        "data: [DONE]\n\n",
      ];

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(events))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "openai",
        "gpt-4o-mini",
        "mock-key",
        [{ role: "user", content: "test" }],
        "https://api.openai.com/v1",
        (c) => chunks.push(c)
      );

      expect(chunks.join("")).toBe("Direct answer.");
      expect(chunks.some((c) => c.includes("think"))).toBe(false);
    });

    it("2.6 Ollama streaming thinking & content integration", async () => {
      const ollamaEvents = [
        '{"message":{"thinking":"Examining Docker socket"}}\n',
        '{"message":{"content":"Socket is responsive"}}\n',
      ];

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(ollamaEvents))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "ollama-local",
        "llama3",
        "",
        [{ role: "user", content: "test" }],
        "http://localhost:11434",
        (c) => chunks.push(c)
      );

      expect(chunks.join("")).toBe(
        "<think>Examining Docker socket</think>\n\nSocket is responsive"
      );
    });

    it("2.7 Claude 3.7 thinking delta integration", async () => {
      const anthropicEvents = [
        'data: {"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"Claude internal reasoning"}}\n\n',
        'data: {"type":"content_block_delta","delta":{"text":"Plan is solid."}}\n\n',
      ];

      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue(createMockStreamResponse(anthropicEvents))
      );

      const { chatStream } = await import("../lib/llmProviders");
      const chunks: string[] = [];

      await chatStream(
        "anthropic",
        "claude-3-7-sonnet-latest",
        "anthropic-test-key",
        [{ role: "user", content: "test" }],
        "",
        (c) => chunks.push(c)
      );

      expect(chunks.join("")).toBe(
        "<think>Claude internal reasoning</think>\n\nPlan is solid."
      );
    });

    it("2.8 Markdown AST correctly parses finished and streaming think blocks", () => {
      // Completed think block
      const completed = "<think>\nStep 1: Check CPU\nStep 2: Check RAM\n</think>\n\nResult OK";
      const ast = parseChatMarkdown(completed);
      expect(ast[0]).toEqual({
        type: "think",
        content: "Step 1: Check CPU\nStep 2: Check RAM",
        streaming: false,
      });
      expect(ast[1]).toMatchObject({
        type: "paragraph",
        text: "Result OK",
      });

      // Streaming in-flight think block (unclosed <think>)
      const inFlight = "<think>\nCurrently evaluating status...";
      const astInFlight = parseChatMarkdown(inFlight);
      expect(astInFlight[0]).toEqual({
        type: "think",
        content: "Currently evaluating status...",
        streaming: true,
      });
    });
  });

  // =========================================================================
  // 3. Conversation Window & Dedup
  // =========================================================================
  describe("3. Conversation Window & Dedup (buildCleanConversationWindow)", () => {
    it("3.1 100 consecutive user messages: collapses consecutive user turns cleanly and limits maxTurns", () => {
      const history = Array.from({ length: 100 }, (_, i) => ({
        role: "user" as const,
        content: `User turn ${i + 1}`,
      }));

      const window = buildCleanConversationWindow(history, "Final question", 10);
      // All consecutive user turns are merged into a single user turn to maintain strict alternation
      expect(window.length).toBe(1);
      expect(window[0].role).toBe("user");
      expect(window[0].content).toContain("User turn 1");
      expect(window[0].content).toContain("User turn 100");
      expect(window[0].content).toContain("Final question");
    });

    it("3.2 Alternating messages starting with assistant: shifts leading assistant to guarantee leading user", () => {
      const history = [
        { role: "assistant" as const, content: "Initial greeting from previous session" },
        { role: "user" as const, content: "First user request" },
        { role: "assistant" as const, content: "First assistant reply" },
      ];

      // Window with maxTurns = 10
      const window = buildCleanConversationWindow(history, "Second user request", 10);
      expect(window[0].role).toBe("user");
      expect(window[0].content).toBe("First user request");
      expect(window[1].role).toBe("assistant");
      expect(window[1].content).toBe("First assistant reply");
      expect(window[2].role).toBe("user");
      expect(window[2].content).toBe("Second user request");

      // Window sliced tightly with maxTurns = 2:
      // Merged would be [U1, A1, U2]. Slicing last 2 gives [A1, U2].
      // Anthropic rule must shift A1 so leading turn is U2!
      const slicedWindow = buildCleanConversationWindow(history, "Second user request", 2);
      expect(slicedWindow[0].role).toBe("user");
      expect(slicedWindow[0].content).toBe("Second user request");
      expect(slicedWindow.length).toBe(1);
    });

    it("3.3 Empty strings and whitespace-only: filters out blank messages and merges cleanly", () => {
      const history = [
        { role: "user" as const, content: "   " },
        { role: "assistant" as const, content: "" },
        { role: "system" as const, content: "system prompt" },
        { role: "user" as const, content: "Real message" },
        { role: "assistant" as const, content: "Assistant reply" },
      ];

      const window = buildCleanConversationWindow(history, "Next message", 10);
      expect(window.length).toBe(3);
      expect(window[0]).toEqual({ role: "user", content: "Real message" });
      expect(window[1]).toEqual({ role: "assistant", content: "Assistant reply" });
      expect(window[2]).toEqual({ role: "user", content: "Next message" });

      // Empty userMessage does not append an empty turn
      const emptyUserWindow = buildCleanConversationWindow(history, "", 10);
      expect(emptyUserWindow.length).toBe(2);
      expect(emptyUserWindow[0].content).toBe("Real message");
      expect(emptyUserWindow[1].content).toBe("Assistant reply");
    });

    it("3.4 Embedded tool tags in assistant messages: cleans execution tags while preserving text", () => {
      const history = [
        { role: "user" as const, content: "Check status" },
        {
          role: "assistant" as const,
          content: [
            "I checked your status:",
            "[QUERY: colima status --json]",
            "[ASK_CHOICE: Restart? | Yes | No]",
            "[SUGGEST: View logs]",
          ].join("\n"),
        },
      ];

      const window = buildCleanConversationWindow(history, "Thanks", 10);
      expect(window.length).toBe(3);
      expect(window[1].role).toBe("assistant");
      // Execution and suggestion tags are stripped from the history sent to the LLM
      expect(window[1].content).toBe("I checked your status:");
      expect(window[1].content).not.toContain("[QUERY:");
      expect(window[1].content).not.toContain("[ASK_CHOICE:");
      expect(window[1].content).not.toContain("[SUGGEST:");
    });

    it("3.5 Duplicate userMessage deduplication: prevents identical duplicate user messages at tail", () => {
      const history = [
        { role: "user" as const, content: "How much RAM is allocated?" },
      ];

      // Caller already pushed userMessage to history
      const window = buildCleanConversationWindow(
        history,
        "How much RAM is allocated?",
        10
      );
      expect(window.length).toBe(1);
      expect(window[0].content).toBe("How much RAM is allocated?");
      expect(window[0].content).not.toContain("\n\nHow much RAM is allocated?");
    });

    it("3.6 maxTurns boundary: maxTurns=0 returns full alternating history starting with user", () => {
      const history = [
        { role: "assistant" as const, content: "Hello" },
        { role: "user" as const, content: "Q1" },
        { role: "assistant" as const, content: "A1" },
      ];

      const window = buildCleanConversationWindow(history, "Q2", 0);
      expect(window[0].role).toBe("user");
      expect(window[0].content).toBe("Q1");
      expect(window[1].role).toBe("assistant");
      expect(window[1].content).toBe("A1");
      expect(window[window.length - 1].content).toBe("Q2");
      expect(window.length).toBe(3);
    });
  });
});

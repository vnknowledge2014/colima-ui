import { beforeEach, expect, it, vi } from "vitest";

// The Tauri HTTP plugin resolves in this environment, so getFetch() would pick
// it over the stubbed global. Forcing the import to fail sends it down the
// browser-fetch branch the stub actually replaces.
vi.mock("@tauri-apps/plugin-http", () => {
  throw new Error("not available in tests");
});

beforeEach(() => {
  vi.restoreAllMocks();
  // getFetch() caches _fetchFn at module scope, so each case has to reload the
  // module for its own stub to take effect.
  vi.resetModules();
});

it("names the provider and the setting when the endpoint is unreachable", async () => {
  vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
  const { chatStream } = await import("./llmProviders");
  await expect(
    chatStream("ollama-local", "llama3", "", [], "http://ollama.con", () => {})
  ).rejects.toThrow(/Endpoint URL in Settings/);
});

it("rethrows an abort unchanged so the caller can still detect it", async () => {
  const controller = new AbortController();
  controller.abort();
  const abortErr = new DOMException("Aborted", "AbortError");
  vi.stubGlobal("fetch", vi.fn().mockRejectedValue(abortErr));
  const { chatStream } = await import("./llmProviders");
  await expect(
    chatStream("ollama-local", "llama3", "", [], "", () => {}, controller.signal)
  ).rejects.toBe(abortErr);
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

it("streams Gemini SSE with alt=sse and thought part emitting <think>...</think>", async () => {
  let requestedUrl = "";
  let requestHeaders: Record<string, string> = {};

  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((url: string, init?: RequestInit) => {
      requestedUrl = url;
      requestHeaders = (init?.headers as Record<string, string>) || {};
      return Promise.resolve(
        createMockStreamResponse([
          'data: {"candidates":[{"content":{"parts":[{"thought":true,"text":"Analyzing colima setup..."}]}}]}\n\n',
          'data: {"candidates":[{"content":{"parts":[{"thought":true,"text":" checking CPU profile."}]}}]}\n\n',
          'data: {"candidates":[{"content":{"parts":[{"text":"Colima is healthy."}]}}]}\n\n',
        ])
      );
    })
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "gemini",
    "gemini-2.0-flash",
    "test-gemini-key",
    [{ role: "user", content: "status" }],
    "",
    (c) => chunks.push(c)
  );

  expect(requestedUrl).toContain(
    "models/gemini-2.0-flash:streamGenerateContent?alt=sse"
  );
  expect(requestHeaders["x-goog-api-key"]).toBe("test-gemini-key");

  const fullText = chunks.join("");
  expect(fullText).toBe(
    "<think>Analyzing colima setup... checking CPU profile.</think>\n\nColima is healthy."
  );
});

it("extracts DeepSeek reasoning_content into <think>...</think> block", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      createMockStreamResponse([
        'data: {"choices":[{"delta":{"reasoning_content":"Inspecting socket connection..."}}]}\n\n',
        'data: {"choices":[{"delta":{"reasoning_content":" Socket found."}}]}\n\n',
        'data: {"choices":[{"delta":{"content":"Connected to daemon."}}]}\n\n',
        'data: [DONE]\n\n',
      ])
    )
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "openai-compatible",
    "deepseek-r1",
    "sk-test",
    [{ role: "user", content: "check" }],
    "http://localhost:11434/v1",
    (c) => chunks.push(c)
  );

  const fullText = chunks.join("");
  expect(fullText).toBe(
    "<think>Inspecting socket connection... Socket found.</think>\n\nConnected to daemon."
  );
});

it("extracts Ollama message.thinking into <think>...</think> block", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      createMockStreamResponse([
        '{"message":{"role":"assistant","thinking":"Examining lima vm..."}}\n',
        '{"message":{"role":"assistant","thinking":" Memory allocation: 4GB."}}\n',
        '{"message":{"role":"assistant","content":"VM running smoothly."}}\n',
      ])
    )
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "ollama-local",
    "deepseek-r1:7b",
    "",
    [{ role: "user", content: "status" }],
    "",
    (c) => chunks.push(c)
  );

  const fullText = chunks.join("");
  expect(fullText).toBe(
    "<think>Examining lima vm... Memory allocation: 4GB.</think>\n\nVM running smoothly."
  );
});

it("handles multi-packet thinking chunk continuity across network splits", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      createMockStreamResponse([
        'data: {"choices":[{"delta":{"reasoning_content":"Step 1: ',
        'Verifying bridge network..."}}]}\n\n',
        'data: {"choices":[{"delta":{"reasoning_content":" Step 2: Checking ',
        'DNS forwarder."}}]}\n\n',
        'data: {"choices":[{"delta":{"content":"Bridge is ',
        'operational."}}]}\n\n',
        'data: [DONE]\n\n',
      ])
    )
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "openai",
    "gpt-4o-mini",
    "sk-test",
    [{ role: "user", content: "net" }],
    "",
    (c) => chunks.push(c)
  );

  const fullText = chunks.join("");
  expect(fullText).toBe(
    "<think>Step 1: Verifying bridge network... Step 2: Checking DNS forwarder.</think>\n\nBridge is operational."
  );
});

it("handles Claude 3.7 thinking_delta delta structure", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      createMockStreamResponse([
        'data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Claude is deliberating..."}}\n\n',
        'data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" considering alternative."}}\n\n',
        'data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Recommended action is restart."}}\n\n',
      ])
    )
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "anthropic",
    "claude-3-7-sonnet-latest",
    "sk-ant",
    [{ role: "user", content: "advice" }],
    "",
    (c) => chunks.push(c)
  );

  const fullText = chunks.join("");
  expect(fullText).toBe(
    "<think>Claude is deliberating... considering alternative.</think>\n\nRecommended action is restart."
  );
});

it("closes <think> block if stream finishes while thinking", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      createMockStreamResponse([
        'data: {"choices":[{"delta":{"reasoning_content":"Thinking only without final content..."}}]}\n\n',
      ])
    )
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "openai",
    "gpt-4o",
    "sk-test",
    [],
    "",
    (c) => chunks.push(c)
  );

  const fullText = chunks.join("");
  expect(fullText).toBe(
    "<think>Thinking only without final content...</think>\n\n"
  );
});

it("uses real SOTA default models without hallucinated identifiers", async () => {
  let capturedBody: Record<string, unknown> | null = null;
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((_url: string, init?: RequestInit) => {
      capturedBody = JSON.parse((init?.body as string) || "{}") as Record<string, unknown>;
      return Promise.resolve(createMockStreamResponse([]));
    })
  );

  const { chatStream, DEFAULT_MODELS, POPULAR_MODELS } = await import("./llmProviders");

  expect(DEFAULT_MODELS.anthropic).toBe("claude-3-5-sonnet-latest");
  expect(DEFAULT_MODELS.openai).toBe("gpt-4o-mini");
  expect(DEFAULT_MODELS.gemini).toBe("gemini-2.0-flash");
  expect(POPULAR_MODELS.anthropic).toContain("claude-3-7-sonnet-latest");

  // Anthropic empty model fallback
  await chatStream("anthropic", "", "key", [], "", () => {});
  expect((capturedBody as Record<string, unknown> | null)?.model).toBe("claude-3-5-sonnet-latest");

  // OpenAI empty model fallback
  await chatStream("openai", "", "key", [], "", () => {});
  expect((capturedBody as Record<string, unknown> | null)?.model).toBe("gpt-4o-mini");
});

it("strips redundant inner <think> tags inside dedicated reasoning_content fields to prevent nested <think><think>", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(
      createMockStreamResponse([
        'data: {"choices":[{"delta":{"reasoning_content":"<think>Already wrapped reasoning</think>"}}]}\n\n',
        'data: {"choices":[{"delta":{"content":"Clean final answer."}}]}\n\n',
        "data: [DONE]\n\n",
      ])
    )
  );

  const { chatStream } = await import("./llmProviders");
  const chunks: string[] = [];

  await chatStream(
    "openai",
    "gpt-4o",
    "sk-test",
    [],
    "",
    (c) => chunks.push(c)
  );

  expect(chunks.join("")).toBe(
    "<think>Already wrapped reasoning</think>\n\nClean final answer."
  );
});

it("throws on mid-stream JSON error frames across Ollama, Gemini, OpenAI, and Anthropic", async () => {
  const fetchSpy = vi.fn();
  vi.stubGlobal("fetch", fetchSpy);
  const { chatStream } = await import("./llmProviders");

  // 1. Ollama error object
  fetchSpy.mockResolvedValueOnce(
    createMockStreamResponse([
      '{"message":{"role":"assistant","content":"Partial "}}\n',
      '{"error":"model \\"llama99\\" not found"}\n',
    ])
  );
  await expect(
    chatStream("ollama-local", "llama99", "", [{ role: "user", content: "hi" }], "", () => {})
  ).rejects.toThrow(/LLM stream error:.*llama99/);

  // 2. OpenAI SSE error frame
  fetchSpy.mockResolvedValueOnce(
    createMockStreamResponse([
      'data: {"error":{"message":"Rate limit exceeded mid-stream","type":"rate_limit_error"}}\n\n',
    ])
  );
  await expect(
    chatStream("openai", "gpt-4o", "sk-test", [{ role: "user", content: "hi" }], "", () => {})
  ).rejects.toThrow(/LLM stream error:.*Rate limit exceeded mid-stream/);

  // 3. Gemini SSE error frame
  fetchSpy.mockResolvedValueOnce(
    createMockStreamResponse([
      'data: {"error":{"code":429,"message":"Quota exceeded"}}\n\n',
    ])
  );
  await expect(
    chatStream("gemini", "gemini-2.0-flash", "gem-key", [{ role: "user", content: "hi" }], "", () => {})
  ).rejects.toThrow(/LLM stream error:.*Quota exceeded/);

  // 4. Anthropic SSE type="error" frame
  fetchSpy.mockResolvedValueOnce(
    createMockStreamResponse([
      'data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}\n\n',
    ])
  );
  await expect(
    chatStream("anthropic", "claude-3-5-sonnet-latest", "sk-ant", [{ role: "user", content: "hi" }], "", () => {})
  ).rejects.toThrow(/LLM stream error:.*Overloaded/);
});

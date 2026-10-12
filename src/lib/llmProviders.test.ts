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

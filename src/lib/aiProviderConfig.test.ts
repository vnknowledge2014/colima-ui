import { describe, it, expect, beforeEach, vi } from "vitest";

const store: Record<string, string> = {};

vi.mock("./settingsStore.svelte", () => ({
  getAppSetting: (key: string, fallback = "") => (store[key] !== undefined ? store[key] : fallback),
  setAppSetting: (key: string, value: string) => {
    store[key] = value;
  },
}));

const {
  activateProvider,
  getProviderField,
  setProviderField,
  normalizeOpenAiBaseUrl,
  normalizeOllamaBaseUrl,
  findProvider,
} = await import("./aiProviderConfig");

beforeEach(() => {
  for (const key of Object.keys(store)) delete store[key];
});

describe("per-provider AI credentials", () => {
  it("keeps each provider's key, endpoint and model apart", () => {
    store["ai_provider"] = "anthropic";
    setProviderField("anthropic", "api_key", "sk-ant");
    setProviderField("anthropic", "model", "claude-sonnet-4");

    activateProvider("openai");
    expect(getProviderField("openai", "api_key")).toBe("");
    expect(getProviderField("openai", "model")).toBe("");
    // The active mirror every AI feature reads must follow the switch.
    expect(store["ai_api_key"]).toBe("");
    expect(store["ai_model"]).toBe("");
  });

  it("restores a provider's own settings when switching back", () => {
    store["ai_provider"] = "anthropic";
    setProviderField("anthropic", "api_key", "sk-ant");

    activateProvider("openai");
    setProviderField("openai", "api_key", "sk-oai");

    activateProvider("anthropic");
    expect(store["ai_api_key"]).toBe("sk-ant");

    activateProvider("openai");
    expect(store["ai_api_key"]).toBe("sk-oai");
  });

  it("does not leak an ollama endpoint into a cloud provider", () => {
    store["ai_provider"] = "ollama-cloud";
    setProviderField("ollama-cloud", "endpoint", "https://ollama.internal");

    activateProvider("gemini");
    expect(store["ai_endpoint"]).toBe("");
  });

  it("adopts pre-scoping settings for the provider that wrote them", () => {
    store["ai_provider"] = "anthropic";
    store["ai_api_key"] = "legacy-key";

    expect(getProviderField("anthropic", "api_key")).toBe("legacy-key");
    expect(getProviderField("openai", "api_key")).toBe("");
  });

  it("keeps a cleared field cleared instead of resurrecting the legacy value", () => {
    store["ai_provider"] = "anthropic";
    store["ai_api_key"] = "legacy-key";

    setProviderField("anthropic", "api_key", "");
    expect(getProviderField("anthropic", "api_key")).toBe("");
  });
});

describe("normalizeOpenAiBaseUrl", () => {
  // The same table is asserted in `ai_chat.rs::tests`. The streaming path uses
  // this one and the non-streaming path uses that one, off a single setting.
  it("reduces every form of a proxy URL to one base", () => {
    for (const input of [
      "http://localhost:4000",
      "http://localhost:4000/",
      "http://localhost:4000/v1",
      "http://localhost:4000/v1/",
      "http://localhost:4000/v1/chat/completions",
      "  http://localhost:4000/v1  ",
    ]) {
      expect(normalizeOpenAiBaseUrl(input)).toBe("http://localhost:4000/v1");
    }
  });

  it("keeps the path of a gateway mounted under one", () => {
    expect(normalizeOpenAiBaseUrl("https://openrouter.ai/api/v1")).toBe("https://openrouter.ai/api/v1");
    expect(normalizeOpenAiBaseUrl("https://api.groq.com/openai/v1/chat/completions")).toBe(
      "https://api.groq.com/openai/v1",
    );
  });

  it("leaves an empty endpoint empty so the caller picks its default", () => {
    expect(normalizeOpenAiBaseUrl("")).toBe("");
    expect(normalizeOpenAiBaseUrl("   ")).toBe("");
  });
});

describe("normalizeOllamaBaseUrl", () => {
  // Mirrored by `ai_chat.rs::tests::an_ollama_host_keeps_one_api_path…`.
  it("strips the /api path Ollama's docs quote, however it was pasted", () => {
    for (const input of [
      "https://ollama.com",
      "https://ollama.com/",
      "https://ollama.com/api",
      "https://ollama.com/api/chat",
      "https://ollama.com/api/tags",
      "  https://ollama.com/api/chat  ",
    ]) {
      expect(normalizeOllamaBaseUrl(input)).toBe("https://ollama.com");
    }
  });

  it("treats a port as part of the host, not a path", () => {
    expect(normalizeOllamaBaseUrl("http://192.168.1.10:11434/api/chat")).toBe(
      "http://192.168.1.10:11434",
    );
    expect(normalizeOllamaBaseUrl("http://localhost:11434")).toBe("http://localhost:11434");
  });

  it("leaves an empty endpoint empty so the caller decides", () => {
    expect(normalizeOllamaBaseUrl("")).toBe("");
    expect(normalizeOllamaBaseUrl("   ")).toBe("");
  });
});

describe("proxy presets", () => {
  it("offers the documented gateways under openai-compatible", () => {
    const spec = findProvider("openai-compatible");
    const labels = spec?.presets?.map((p) => p.label) ?? [];
    expect(labels).toContain("LiteLLM");
    expect(labels).toContain("9router");
    // Every preset must already be in normal form, or picking one and saving
    // would rewrite the field under the user.
    for (const preset of spec?.presets ?? []) {
      expect(normalizeOpenAiBaseUrl(preset.endpoint)).toBe(preset.endpoint);
    }
  });

  it("offers a real host for Ollama Cloud, already in normal form", () => {
    const spec = findProvider("ollama-cloud");
    const endpoints = spec?.presets?.map((p) => p.endpoint) ?? [];
    expect(endpoints).toContain("https://ollama.com");
    for (const endpoint of endpoints) {
      expect(normalizeOllamaBaseUrl(endpoint)).toBe(endpoint);
    }
  });
});

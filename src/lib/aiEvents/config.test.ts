import { beforeEach, describe, expect, it, vi } from "vitest";
import { uiState, VALID_PAGES } from "../../store.svelte";
import { settingsState } from "../settingsStore.svelte";

const mockGetAll = vi.fn();
const mockSet = vi.fn().mockResolvedValue(undefined);
const mockGetResolutions = vi.fn();
const mockCall = vi.fn();

vi.mock("../api", () => ({
  modelsApi: {
    listModels: vi.fn().mockResolvedValue([]),
    pullModel: vi.fn().mockResolvedValue(undefined),
    deleteModel: vi.fn().mockResolvedValue(undefined),
    serveModel: vi.fn().mockResolvedValue(undefined),
  },
  settingsApi: {
    getAll: (...args: unknown[]) => mockGetAll(...args),
    set: (...args: unknown[]) => mockSet(...args),
    get: vi.fn().mockResolvedValue(null),
  },
  environmentApi: {
    getResolutions: (...args: unknown[]) => mockGetResolutions(...args),
  },
  colimaApi: {},
  dockerApi: {},
  volumesApi: {},
  networksApi: {},
  composeApi: {},
  k8sApi: {},
  kindApi: {},
  limaApi: {},
  securityApi: {},
  systemApi: {},
  sysMethods: {},
}));

vi.mock("../api/client", () => ({
  call: (...args: unknown[]) => mockCall(...args),
  isTauri: () => false,
}));

describe("aiEvents config & system registries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    uiState.currentPage = "dashboard";
    for (const k of Object.keys(settingsState)) {
      delete settingsState[k];
    }
  });

  it("ai-config-status redacts ai_api_key, ai_api_key.openai, and any token/secret/password keys", async () => {
    mockGetAll.mockResolvedValue({
      ai_provider: "openai",
      ai_model: "gpt-4o",
      ai_api_key: "sk-live-super-secret-root-key",
      "ai_api_key.openai": "sk-proj-openai-secret-999",
      "ai_api_key.anthropic": "",
      mcp_bearer_token: "tok-123456",
      webhook_secret: "whsec_abcdef",
      registry_password: "p@ssw0rd-value",
      empty_password: "",
      theme: "dark",
    });

    const { executeEvent } = await import("../aiEventBus");
    const rawJson = await executeEvent("ai-config-status", {});

    expect(mockGetAll).toHaveBeenCalledTimes(1);
    expect(rawJson).not.toContain("sk-live-super-secret-root-key");
    expect(rawJson).not.toContain("sk-proj-openai-secret-999");
    expect(rawJson).not.toContain("tok-123456");
    expect(rawJson).not.toContain("whsec_abcdef");
    expect(rawJson).not.toContain("p@ssw0rd-value");

    const parsed = JSON.parse(rawJson) as Record<string, string>;
    expect(parsed.ai_provider).toBe("openai");
    expect(parsed.ai_model).toBe("gpt-4o");
    expect(parsed.theme).toBe("dark");
    expect(parsed.ai_api_key).toBe("[REDACTED]");
    expect(parsed["ai_api_key.openai"]).toBe("[REDACTED]");
    expect(parsed["ai_api_key.anthropic"]).toBe("");
    expect(parsed.mcp_bearer_token).toBe("[REDACTED]");
    expect(parsed.webhook_secret).toBe("[REDACTED]");
    expect(parsed.registry_password).toBe("[REDACTED]");
    expect(parsed.empty_password).toBe("");
  });

  it("ai-update-config updates reactive settingsState via setAppSetting", async () => {
    const { executeEvent } = await import("../aiEventBus");
    await executeEvent("ai-update-config", {
      provider: "anthropic",
      model: "claude-3-7-sonnet-latest",
      api_key: "sk-ant-key",
      settings: { polling_interval: 15 },
    });

    expect(settingsState.ai_provider).toBe("anthropic");
    expect(settingsState.ai_model).toBe("claude-3-7-sonnet-latest");
    expect(settingsState["ai_model.anthropic"]).toBe("claude-3-7-sonnet-latest");
    expect(settingsState.ai_api_key).toBe("sk-ant-key");
    expect(settingsState["ai_api_key.anthropic"]).toBe("sk-ant-key");
    expect(settingsState.polling_interval).toBe("15");
    expect(mockSet).toHaveBeenCalledWith("ai_provider", "anthropic");
  });

  it("list-presets, save-preset, and delete-preset route through dual-mode call()", async () => {
    mockCall.mockResolvedValueOnce({ dev: '{"cpus":4}' });
    mockCall.mockResolvedValueOnce("saved");
    mockCall.mockResolvedValueOnce("deleted");

    const { executeEvent } = await import("../aiEventBus");

    const listed = await executeEvent("list-presets", {});
    expect(JSON.parse(listed)).toEqual({ dev: '{"cpus":4}' });
    expect(mockCall).toHaveBeenNthCalledWith(1, "get_all_presets", undefined, "GET", "/api/presets");

    const payload = { id: "dev", cpus: 4 };
    const configJson = JSON.stringify(payload);
    const saved = await executeEvent("save-preset", payload);
    expect(saved).toContain("Preset 'dev' saved");
    expect(mockCall).toHaveBeenNthCalledWith(
      2,
      "save_preset",
      { id: "dev", configJson },
      "POST",
      "/api/presets",
      undefined,
      { id: "dev", config_json: configJson, configJson },
    );

    const deleted = await executeEvent("delete-preset", { id: "dev" });
    expect(deleted).toContain("Preset 'dev' deleted");
    expect(mockCall).toHaveBeenNthCalledWith(
      3,
      "delete_preset",
      { id: "dev" },
      "POST",
      "/api/presets/delete",
      undefined,
      { id: "dev" },
    );
  });

  it("navigate validates against VALID_PAGES and updates uiState.currentPage directly", async () => {
    const { executeEvent } = await import("../aiEventBus");

    expect(VALID_PAGES.length).toBe(17);

    const okMsg = await executeEvent("navigate", { page: "environment" });
    expect(okMsg).toBe("Navigated to environment tab.");
    expect(uiState.currentPage).toBe("environment");

    const errMsg = await executeEvent("navigate", { page: "evil-script-route" });
    expect(errMsg).toContain("Error: Invalid page");
    expect(uiState.currentPage).toBe("environment");
  });

  it("registers environment-resolutions as SAFE (100 total events) and returns formatted report", async () => {
    mockGetResolutions.mockResolvedValue({
      envDuality: [],
      rows: [{ key: "colima-home", rule: "default", chosen: "/Users/me/.colima", detail: "", health: "exists" }],
      shellSnapshotError: null,
    });

    const { executeEvent, getCategory, listEvents } = await import("../aiEventBus");

    expect(listEvents().length).toBe(100);
    expect(getCategory("environment-resolutions")).toBe("SAFE");

    const out = await executeEvent("environment-resolutions", {});
    expect(mockGetResolutions).toHaveBeenCalledTimes(1);
    expect(JSON.parse(out).rows[0].key).toBe("colima-home");
  });
});

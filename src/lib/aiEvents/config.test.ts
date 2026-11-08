import { beforeEach, describe, expect, it, vi } from "vitest";
import { uiState, VALID_PAGES } from "../../store.svelte";
import { aiState } from "../../store/ai.svelte";
import { isRunningInTauri } from "../env";
import { settingsState } from "../settingsStore.svelte";

const mockGetAll = vi.fn();
const mockSet = vi.fn().mockResolvedValue(undefined);
const mockGetResolutions = vi.fn();
const mockCall = vi.fn();

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return {
    ...actual,
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
  };
});

vi.mock("../api/client", () => ({
  call: (...args: unknown[]) => mockCall(...args),
  isTauri: () => false,
  getApiToken: vi.fn().mockResolvedValue(""),
  resolveApiBase: vi.fn().mockResolvedValue("http://127.0.0.1:19876"),
}));

describe("aiEvents 130-event bus parity & handlers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    uiState.currentPage = "dashboard";
    uiState.aiPanelOpen = false;
    uiState.settingsSection = null;
    aiState.isOpen = false;
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

  it("navigate delegates to applyUiNavigatePayload supporting VALID_PAGES, ai-chat, section, and invalid-page rejection", async () => {
    const { executeEvent } = await import("../aiEventBus");

    expect(VALID_PAGES.length).toBe(17);

    const okMsg = await executeEvent("navigate", { page: "environment" });
    expect(okMsg).toBe("Navigated to environment tab.");
    expect(uiState.currentPage).toBe("environment");

    const chatMsg = await executeEvent("navigate", { page: "ai-chat" });
    expect(chatMsg).toBe("Navigated to ai-chat tab.");
    expect(aiState.isOpen).toBe(true);
    expect(uiState.aiPanelOpen).toBe(true);

    const settingsMsg = await executeEvent("navigate", { page: "settings", section: "ai" });
    expect(settingsMsg).toBe("Navigated to settings tab.");
    expect(uiState.currentPage).toBe("settings");
    expect(uiState.settingsSection).toBe("ai");

    const errMsg = await executeEvent("navigate", { page: "evil-script-route" });
    expect(errMsg).toContain("Error: Invalid page 'evil-script-route'");
    expect(errMsg).toContain("ai-chat");
    expect(uiState.currentPage).toBe("settings");
  });

  it("cli-exec, build-image, compose-build, and compose-pull execute runSandboxed even when isRunningInTauri() is false", async () => {
    expect(isRunningInTauri()).toBe(false);

    mockCall
      .mockResolvedValueOnce({ stdout: "container-1\n", stderr: "", exit_code: 0 })
      .mockResolvedValueOnce({ stdout: "Successfully built abc123", stderr: "", exit_code: 0 })
      .mockResolvedValueOnce({ stdout: "Compose built", stderr: "", exit_code: 0 })
      .mockResolvedValueOnce({ stdout: "Compose pulled", stderr: "", exit_code: 0 })
      .mockRejectedValueOnce(new Error("banned: command chaining"));

    const { executeEvent } = await import("../aiEventBus");

    const cliOut = await executeEvent("cli-exec", { command: "docker", args: ["ps", "-q"] });
    expect(cliOut).toBe("container-1");
    expect(cliOut).not.toContain("[SIMULATED]");
    expect(mockCall).toHaveBeenNthCalledWith(
      1,
      "sandbox_execute_approved",
      { command: "docker ps -q" },
      "POST",
      "/api/sandbox/execute-approved",
      undefined,
      { command: "docker ps -q" },
    );

    const buildOut = await executeEvent("build-image", { tag: "demo:1", file: "Dockerfile", dir: "./app" });
    expect(buildOut).toBe("Successfully built abc123");
    expect(buildOut).not.toContain("[SIMULATED]");
    expect(mockCall).toHaveBeenNthCalledWith(
      2,
      "sandbox_execute_approved",
      { command: "docker build -t demo:1 -f Dockerfile ./app" },
      "POST",
      "/api/sandbox/execute-approved",
      undefined,
      { command: "docker build -t demo:1 -f Dockerfile ./app" },
    );

    const composeBuildOut = await executeEvent("compose-build", { file: "compose.yaml", dir: "./stack" });
    expect(composeBuildOut).toBe("Compose built");
    expect(composeBuildOut).not.toContain("[SIMULATED]");

    const composePullOut = await executeEvent("compose-pull", { dir: "./stack" });
    expect(composePullOut).toBe("Compose pulled");
    expect(composePullOut).not.toContain("[SIMULATED]");

    const cliErr = await executeEvent("cli-exec", { command: "docker", args: ["ps"] });
    expect(cliErr).toBe("Error executing command: banned: command chaining");
  });

  it("registers all 130 unique events (62 SAFE + 68 mutating) with zero duplicate keys across sub-registries", async () => {
    mockGetResolutions.mockResolvedValue({
      envDuality: [],
      rows: [{ key: "colima-home", rule: "default", chosen: "/Users/me/.colima", detail: "", health: "exists" }],
      shellSnapshotError: null,
    });

    const [
      { executeEvent, getCategory, listEvents },
      { colimaRegistry },
      { dockerRegistry },
      { volumesRegistry },
      { systemRegistry },
      { composeRegistry },
      { k8sRegistry },
      { limaRegistry },
      { observabilityRegistry },
      { securityRegistry },
      { configRegistry },
    ] = await Promise.all([
      import("../aiEventBus"),
      import("./colima"),
      import("./docker"),
      import("./volumes"),
      import("./system"),
      import("./compose"),
      import("./k8s"),
      import("./lima"),
      import("./observability"),
      import("./security"),
      import("./config"),
    ]);

    const subRegistries = [
      colimaRegistry,
      dockerRegistry,
      volumesRegistry,
      systemRegistry,
      composeRegistry,
      k8sRegistry,
      limaRegistry,
      observabilityRegistry,
      securityRegistry,
      configRegistry,
    ];
    const sumSubKeys = subRegistries.reduce((acc, r) => acc + Object.keys(r).length, 0);
    const events = listEvents();

    expect(sumSubKeys).toBe(130);
    expect(events.length).toBe(130);
    expect(events.filter((e) => e.category === "SAFE").length).toBe(62);
    expect(getCategory("environment-resolutions")).toBe("SAFE");

    const out = await executeEvent("environment-resolutions", {});
    expect(mockGetResolutions).toHaveBeenCalledTimes(1);
    expect(JSON.parse(out).rows[0].key).toBe("colima-home");
  });

  it("wires colima config/diagnostics, k8s-pods, compose autofix/lifecycle, observability, and security events", async () => {
    const { executeEvent, getCategory } = await import("../aiEventBus");

    // Verify categories of all 30 newly added events
    const expectedSafe = [
      "diagnostic-logs",
      "colima-config-get",
      "k8s-pods",
      "compose-ps",
      "compose-diagnose",
      "compose-autofix-history",
      "topology-get",
      "activity-feed",
      "alerts-rules",
      "alerts-events",
      "self-heal-rules",
      "self-heal-log",
      "security-history",
      "security-rules",
      "security-alternatives",
      "falco-state",
      "falco-events",
      "detonation-list",
    ];
    for (const name of expectedSafe) {
      expect(getCategory(name)).toBe("SAFE");
    }

    const expectedNormal = [
      "colima-config-apply",
      "compose-start",
      "compose-stop",
      "compose-autofix-propose",
      "compose-autofix-apply",
      "compose-autofix-undo",
      "security-scan",
      "security-sbom",
      "security-audit",
      "security-triage",
      "detonation-cancel",
    ];
    for (const name of expectedNormal) {
      expect(getCategory(name)).toBe("NORMAL");
    }
    expect(getCategory("detonation-start")).toBe("DANGEROUS");

    // Colima diagnostic-logs & colima-config-apply (auto-fetching mtime when omitted)
    mockCall
      .mockResolvedValueOnce("vm log line")
      .mockResolvedValueOnce({ profile: "default", values: { cpu: 2 }, mtime: 1700000000 })
      .mockResolvedValueOnce({ changes: [{ field: "cpu", from: "2", to: "4", requiresRestart: true }], issues: [], backupPath: "/tmp/colima.yaml.bak", mtime: 1700000001 });

    expect(await executeEvent("diagnostic-logs", { profile: "dev" })).toBe("vm log line");
    const applyOut = JSON.parse(await executeEvent("colima-config-apply", { profile: "default", changes: { cpu: 4 } }));
    expect(applyOut.mtime).toBe(1700000001);
    expect(mockCall).toHaveBeenNthCalledWith(
      3,
      "apply_colima_config",
      { profile: "default", changes: { cpu: 4 }, expectedMtime: 1700000000 },
      "POST",
      "/api/instances/config/apply",
      undefined,
      { profile: "default", changes: { cpu: 4 }, expectedMtime: 1700000000 },
    );

    // k8s-pods
    mockCall.mockResolvedValueOnce([{ name: "web-0", status: "Running" }]);
    const podsOut = JSON.parse(await executeEvent("k8s-pods", { namespace: "prod" }));
    expect(podsOut[0].name).toBe("web-0");

    // Compose autofix & diagnose
    mockCall
      .mockResolvedValueOnce({ validation: { valid: true, raw_error: "", signature: "", category: "" }, kb_solutions: [], kb_anti_patterns: [], llm_payload_preview: "", needs_llm: false })
      .mockResolvedValueOnce({ fix_id: "fix-1", file_path: "/app/compose.yaml", applied_at: "now", explanation: "bump port", undone: false });
    const diagOut = JSON.parse(await executeEvent("compose-diagnose", { filePath: "/app/compose.yaml" }));
    expect(diagOut.validation.valid).toBe(true);
    const fixOut = JSON.parse(
      await executeEvent("compose-autofix-apply", {
        filePath: "/app/compose.yaml",
        newContent: "services: {}\n",
        explanation: "bump port",
        declaredRemovals: ["services.old"],
      }),
    );
    expect(fixOut.fix_id).toBe("fix-1");

    // Observability
    mockCall
      .mockResolvedValueOnce({ nodes: [{ id: "c1", kind: "container", label: "api", status: "running", meta: {} }], edges: [], warnings: [] })
      .mockResolvedValueOnce({ items: [], partial: [] });
    const topoOut = JSON.parse(await executeEvent("topology-get", {}));
    expect(topoOut.nodes[0].id).toBe("c1");
    const feedOut = JSON.parse(await executeEvent("activity-feed", { kind: "lifecycle", limit: 10 }));
    expect(feedOut.items).toEqual([]);

    // Security & Detonation
    const fakeAudit = {
      scan: { imageRef: "nginx:1.27", imageDigest: "sha256:111", findings: [], scanner: "trivy", scannerVersion: "0.58", scannedAt: 1 },
      evaluation: { results: [], skipped: [] },
      score: {
        vulnerabilities: { earned: 40, max: 40, failedRules: [] },
        hardening: { earned: 20, max: 30, failedRules: ["H1"] },
        provenance: { earned: 15, max: 15, failedRules: [] },
        freshness: { earned: 15, max: 15, failedRules: [] },
        total: 90,
        inputs: { packVersion: "1", engineVersion: "0.3.3", scanner: "trivy", scannerVersion: "0.58", level: "l1" },
      },
    };
    mockCall
      .mockResolvedValueOnce(fakeAudit)
      .mockResolvedValueOnce({ items: [], currentScore: 90, potentialScore: 100, llmPayloadPreview: "" })
      .mockResolvedValueOnce("det-session-1")
      .mockResolvedValueOnce(true);

    const triageOut = JSON.parse(await executeEvent("security-triage", { imageRef: "nginx:1.27", level: "l2" }));
    expect(triageOut.currentScore).toBe(90);

    const detId = await executeEvent("detonation-start", { image: "alpine:3.20", timeoutSecs: 60 });
    expect(detId).toBe("det-session-1");

    const detCancel = JSON.parse(await executeEvent("detonation-cancel", { sessionId: "det-session-1" }));
    expect(detCancel).toEqual({ cancelled: true, sessionId: "det-session-1" });
  });
});

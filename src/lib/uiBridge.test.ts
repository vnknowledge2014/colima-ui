import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const hoisted = vi.hoisted(() => ({
  isTauri: false,
  apiToken: "test-secret-token",
  apiBase: "http://127.0.0.1:11420",
  tauriListeners: new Map<string, (e: { payload: unknown }) => void>(),
  unlistenSpies: [] as Array<ReturnType<typeof vi.fn>>,
  listenGate: null as (() => Promise<void>) | null,
  savedSettings: new Map<string, string>(),
  aiApiMocks: {
    listConversations: vi.fn(),
    loadHistory: vi.fn(),
    saveMessage: vi.fn(),
    clearHistory: vi.fn(),
    createConversation: vi.fn(),
    renameConversation: vi.fn(),
    deleteConversation: vi.fn(),
  },
}));

vi.mock("./env", () => ({
  isRunningInTauri: () => hoisted.isTauri,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (topic: string, cb: (e: { payload: unknown }) => void) => {
    if (hoisted.listenGate) {
      await hoisted.listenGate();
    }
    hoisted.tauriListeners.set(topic, cb);
    const unlisten = vi.fn(() => {
      hoisted.tauriListeners.delete(topic);
    });
    hoisted.unlistenSpies.push(unlisten);
    return unlisten;
  }),
}));

vi.mock("./api", () => ({
  getApiToken: vi.fn(async () => hoisted.apiToken),
  resolveApiBase: vi.fn(async () => hoisted.apiBase),
  aiApi: hoisted.aiApiMocks,
}));

vi.mock("./settingsStore.svelte", () => ({
  appState: { isSettingsLoaded: true },
  getAppSetting: vi.fn((key: string, fallback = "") => {
    return hoisted.savedSettings.has(key)
      ? (hoisted.savedSettings.get(key) ?? fallback)
      : fallback;
  }),
  setAppSetting: vi.fn((key: string, value: string) => {
    hoisted.savedSettings.set(key, value);
    return Promise.resolve();
  }),
}));

vi.mock("./globalToast", () => ({
  globalToast: vi.fn(),
}));

import { VALID_PAGES, uiState } from "../store.svelte";
import {
  A2A_DEFAULT_SESSION_ID as STORE_A2A_SESSION_ID,
  DEFAULT_CONVERSATION_ID,
  aiState,
  initAiHistory,
  refreshConversations,
  switchConversation,
} from "../store/ai.svelte";
import { globalToast } from "./globalToast";
import { setAppSetting } from "./settingsStore.svelte";
import {
  A2A_DEFAULT_SESSION_ID,
  EXTERNAL_CHAT_TURN_EVENT,
  UI_NAVIGATE_EVENT,
  applyUiNavigatePayload,
  handleExternalChatTurn,
  shouldMirrorExternalChatTurn,
  startUiBridge,
  type ExternalChatTurnPayload,
} from "./uiBridge";

class MockEventSource {
  static instances: MockEventSource[] = [];
  url: string;
  closed = false;
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  private listeners = new Map<string, Set<(e: MessageEvent) => void>>();

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  addEventListener(type: string, listener: (e: MessageEvent) => void) {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener);
  }

  emit(type: string, data: string) {
    const set = this.listeners.get(type);
    if (!set) return;
    const event = { data } as MessageEvent;
    for (const fn of set) {
      fn(event);
    }
  }

  close() {
    this.closed = true;
  }
}

describe("uiBridge & A2A Session Isolation", () => {
  const originalEventSource = globalThis.EventSource;

  beforeEach(() => {
    hoisted.isTauri = false;
    hoisted.apiToken = "test-secret-token";
    hoisted.apiBase = "http://127.0.0.1:11420";
    hoisted.tauriListeners.clear();
    hoisted.unlistenSpies = [];
    hoisted.listenGate = null;
    hoisted.savedSettings.clear();

    hoisted.aiApiMocks.listConversations.mockResolvedValue([]);
    hoisted.aiApiMocks.loadHistory.mockResolvedValue([]);
    hoisted.aiApiMocks.saveMessage.mockResolvedValue(undefined);

    uiState.currentPage = "dashboard";
    uiState.aiPanelOpen = false;
    uiState.settingsSection = null;

    aiState.messages = [];
    aiState.conversations = [];
    aiState.activeConversationId = DEFAULT_CONVERSATION_ID;
    aiState.isOpen = false;
    aiState.isProcessing = false;

    MockEventSource.instances = [];
    globalThis.EventSource = MockEventSource as unknown as typeof EventSource;
    vi.clearAllMocks();
  });

  afterEach(() => {
    globalThis.EventSource = originalEventSource;
    vi.useRealTimers();
  });

  describe("A2A_DEFAULT_SESSION_ID constant", () => {
    it("exports colima-a2a-default-session from both ai.svelte.ts and uiBridge.ts", () => {
      expect(A2A_DEFAULT_SESSION_ID).toBe("colima-a2a-default-session");
      expect(STORE_A2A_SESSION_ID).toBe("colima-a2a-default-session");
    });
  });

  describe("shouldMirrorExternalChatTurn", () => {
    it("strictly rejects colima-a2a-default-session when mirror_ui is false or undefined", () => {
      expect(
        shouldMirrorExternalChatTurn(
          { session_id: A2A_DEFAULT_SESSION_ID, phase: "start", prompt: "status" },
          "default",
        ),
      ).toBe(false);

      expect(
        shouldMirrorExternalChatTurn(
          {
            session_id: A2A_DEFAULT_SESSION_ID,
            mirror_ui: false,
            phase: "complete",
            reply: "ok",
          },
          "default",
        ),
      ).toBe(false);

      // Even if activeConversationId were somehow equal to A2A_DEFAULT_SESSION_ID,
      // the strict isolation rule MUST still reject when mirror_ui !== true.
      expect(
        shouldMirrorExternalChatTurn(
          { session_id: `  ${A2A_DEFAULT_SESSION_ID}  `, mirror_ui: false },
          A2A_DEFAULT_SESSION_ID,
        ),
      ).toBe(false);
    });

    it("accepts colima-a2a-default-session when mirror_ui is explicitly true", () => {
      expect(
        shouldMirrorExternalChatTurn(
          {
            session_id: A2A_DEFAULT_SESSION_ID,
            mirror_ui: true,
            phase: "start",
            prompt: "show running containers",
          },
          "default",
        ),
      ).toBe(true);
    });

    it("accepts personal session turns matching activeConversationId and rejects foreign sessions when mirror_ui is false", () => {
      expect(
        shouldMirrorExternalChatTurn(
          { session_id: "conv-personal-1", mirror_ui: false, phase: "start", prompt: "hi" },
          "conv-personal-1",
        ),
      ).toBe(true);

      expect(
        shouldMirrorExternalChatTurn(
          { session_id: "  conv-personal-1  ", phase: "complete", reply: "hello" },
          "conv-personal-1",
        ),
      ).toBe(true);

      expect(
        shouldMirrorExternalChatTurn(
          { session_id: "conv-foreign-other", mirror_ui: false, phase: "start", prompt: "hi" },
          "conv-personal-1",
        ),
      ).toBe(false);

      expect(
        shouldMirrorExternalChatTurn(
          { session_id: "conv-foreign-other", phase: "start", prompt: "hi" },
          "conv-personal-1",
        ),
      ).toBe(false);

      // Foreign session with explicit mirror_ui: true is permitted to mirror
      expect(
        shouldMirrorExternalChatTurn(
          { session_id: "conv-foreign-other", mirror_ui: true, phase: "start", prompt: "hi" },
          "conv-personal-1",
        ),
      ).toBe(true);
    });

    it("rejects nullish, empty, or whitespace-only session_id payloads", () => {
      expect(shouldMirrorExternalChatTurn({}, "default")).toBe(false);
      expect(shouldMirrorExternalChatTurn({ session_id: "" }, "default")).toBe(false);
      expect(shouldMirrorExternalChatTurn({ session_id: "   ", mirror_ui: true }, "default")).toBe(
        false,
      );
      expect(
        shouldMirrorExternalChatTurn(
          null as unknown as ExternalChatTurnPayload,
          "default",
        ),
      ).toBe(false);
    });
  });

  describe("applyUiNavigatePayload", () => {
    it("navigates to all 17 valid pages (including case/whitespace normalization)", () => {
      expect(VALID_PAGES).toHaveLength(17);

      for (const page of VALID_PAGES) {
        uiState.currentPage = "dashboard";
        const ok = applyUiNavigatePayload({ page: `  ${page.toUpperCase()}  ` });
        expect(ok).toBe(true);
        expect(uiState.currentPage).toBe(page);
      }
    });

    it("opens aiState.isOpen and uiState.aiPanelOpen on 'ai-chat' without changing currentPage", () => {
      uiState.currentPage = "containers";
      aiState.isOpen = false;
      uiState.aiPanelOpen = false;

      const ok = applyUiNavigatePayload({ page: "  AI-Chat  " });
      expect(ok).toBe(true);
      expect(aiState.isOpen).toBe(true);
      expect(uiState.aiPanelOpen).toBe(true);
      expect(uiState.currentPage).toBe("containers");
    });

    it("calls openSettingsSection when navigating to 'settings' with a section", () => {
      const openSectionSpy = vi.fn();
      const mockUi = { currentPage: "dashboard", aiPanelOpen: false };
      const mockAi = { isOpen: false };

      const ok = applyUiNavigatePayload(
        { page: "settings", section: "  ai  " },
        {
          uiState: mockUi,
          aiState: mockAi,
          openSettingsSection: openSectionSpy,
        },
      );

      expect(ok).toBe(true);
      expect(mockUi.currentPage).toBe("settings");
      expect(openSectionSpy).toHaveBeenCalledTimes(1);
      expect(openSectionSpy).toHaveBeenCalledWith("ai");

      // Also verify real store openSettingsSection integration
      applyUiNavigatePayload({ page: "settings", section: "security" });
      expect(uiState.currentPage).toBe("settings");
      expect(uiState.settingsSection).toBe("security");
    });

    it("triggers globalToast with 'info' when toast text is present", () => {
      const ok = applyUiNavigatePayload({
        page: "kubernetes",
        toast: "  Switched to Kubernetes cluster view  ",
      });

      expect(ok).toBe(true);
      expect(uiState.currentPage).toBe("kubernetes");
      expect(globalToast).toHaveBeenCalledTimes(1);
      expect(globalToast).toHaveBeenCalledWith("info", "Switched to Kubernetes cluster view");

      vi.clearAllMocks();
      applyUiNavigatePayload({ page: "volumes", toast: "   " });
      expect(globalToast).not.toHaveBeenCalled();
    });

    it("safely rejects unknown, empty, and prototype-pollution page names", () => {
      uiState.currentPage = "instances";
      aiState.isOpen = false;

      for (const bad of ["__proto__", "constructor", "toString", "invalid-tab", "", "   "]) {
        const ok = applyUiNavigatePayload({ page: bad, toast: "Should not fire" });
        expect(ok).toBe(false);
      }

      expect(applyUiNavigatePayload({})).toBe(false);
      expect(uiState.currentPage).toBe("instances");
      expect(aiState.isOpen).toBe(false);
      expect(globalToast).not.toHaveBeenCalled();
    });
  });

  describe("External chat turn mirroring, deduplication & session isolation", () => {
    it("completely ignores non-mirrored colima-a2a-default-session turns without hijacking activeConversationId", () => {
      aiState.activeConversationId = "conv-user-active";

      const started = handleExternalChatTurn({
        phase: "start",
        session_id: A2A_DEFAULT_SESSION_ID,
        prompt: "Background CLI health check",
      });
      const completed = handleExternalChatTurn({
        phase: "complete",
        session_id: A2A_DEFAULT_SESSION_ID,
        prompt: "Background CLI health check",
        reply: "All VMs healthy.",
      });

      expect(started).toBe(false);
      expect(completed).toBe(false);
      expect(aiState.activeConversationId).toBe("conv-user-active");
      expect(aiState.isOpen).toBe(false);
      expect(aiState.messages).toHaveLength(0);
      expect(setAppSetting).not.toHaveBeenCalled();
    });

    it("mirrors colima-a2a-default-session when mirror_ui is true without hijacking activeConversationId or re-saving to SQLite", () => {
      aiState.activeConversationId = "conv-user-active";

      expect(
        handleExternalChatTurn({
          phase: "start",
          session_id: A2A_DEFAULT_SESSION_ID,
          mirror_ui: true,
          prompt: "Show failing containers",
        }),
      ).toBe(true);

      expect(aiState.isOpen).toBe(true);
      expect(uiState.aiPanelOpen).toBe(true);
      expect(aiState.activeConversationId).toBe("conv-user-active");
      expect(aiState.messages).toHaveLength(1);
      expect(aiState.messages[0].role).toBe("user");
      expect(aiState.messages[0].content).toBe("Show failing containers");

      expect(
        handleExternalChatTurn({
          phase: "complete",
          session_id: A2A_DEFAULT_SESSION_ID,
          mirror_ui: true,
          prompt: "Show failing containers",
          reply: "Container redis-cache exited with code 137.",
        }),
      ).toBe(true);

      // Deduplicated: exactly 1 user bubble + 1 assistant bubble, 0 SQLite writes, 0 session hijacks
      expect(aiState.messages).toHaveLength(2);
      expect(aiState.messages.map((m) => ({ role: m.role, content: m.content }))).toEqual([
        { role: "user", content: "Show failing containers" },
        { role: "assistant", content: "Container redis-cache exited with code 137." },
      ]);
      expect(aiState.activeConversationId).toBe("conv-user-active");
      expect(hoisted.aiApiMocks.saveMessage).not.toHaveBeenCalled();
      expect(setAppSetting).not.toHaveBeenCalled();
    });

    it("deduplicates repeated start and complete frames and handles missed start frames cleanly", () => {
      aiState.activeConversationId = "conv-shared";

      // Duplicate start frames -> single user bubble
      handleExternalChatTurn({
        phase: "start",
        session_id: "conv-shared",
        prompt: "Check Colima memory",
      });
      handleExternalChatTurn({
        phase: "start",
        session_id: "conv-shared",
        prompt: "Check Colima memory",
      });
      expect(aiState.messages).toHaveLength(1);

      // Duplicate complete frames -> single assistant bubble
      handleExternalChatTurn({
        phase: "complete",
        session_id: "conv-shared",
        prompt: "Check Colima memory",
        reply: "4 GiB allocated.",
      });
      handleExternalChatTurn({
        phase: "complete",
        session_id: "conv-shared",
        prompt: "Check Colima memory",
        reply: "4 GiB allocated.",
      });
      expect(aiState.messages).toHaveLength(2);

      // Missed start frame on next turn -> complete appends both user + assistant
      handleExternalChatTurn({
        phase: "complete",
        session_id: "conv-shared",
        prompt: "What about disk?",
        reply: "60 GiB allocated.",
      });
      expect(aiState.messages.map((m) => ({ role: m.role, content: m.content }))).toEqual([
        { role: "user", content: "Check Colima memory" },
        { role: "assistant", content: "4 GiB allocated." },
        { role: "user", content: "What about disk?" },
        { role: "assistant", content: "60 GiB allocated." },
      ]);
    });

    it("prevents initAiHistory, switchConversation, and refreshConversations from persisting or listing colima-a2a-default-session", async () => {
      hoisted.savedSettings.set("ai_active_conversation", A2A_DEFAULT_SESSION_ID);
      hoisted.aiApiMocks.listConversations.mockResolvedValue([
        {
          id: A2A_DEFAULT_SESSION_ID,
          title: "Automated A2A",
          updated_at: 200,
          message_count: 5,
          preview: "ping",
        },
        {
          id: DEFAULT_CONVERSATION_ID,
          title: "",
          updated_at: 100,
          message_count: 1,
          preview: "hello",
        },
      ]);

      await refreshConversations();
      expect(aiState.conversations.map((c) => c.id)).toEqual([DEFAULT_CONVERSATION_ID]);

      await initAiHistory();
      expect(aiState.activeConversationId).toBe(DEFAULT_CONVERSATION_ID);
      expect(hoisted.savedSettings.get("ai_active_conversation")).toBe(DEFAULT_CONVERSATION_ID);

      await switchConversation(A2A_DEFAULT_SESSION_ID);
      expect(aiState.activeConversationId).toBe(DEFAULT_CONVERSATION_ID);
      expect(hoisted.savedSettings.get("ai_active_conversation")).toBe(DEFAULT_CONVERSATION_ID);
    });
  });

  describe("startUiBridge transport subscriptions", () => {
    it("connects via Browser EventSource with non-encoded repeated topics params, dispatches events, and tears down cleanly", async () => {
      vi.useFakeTimers();
      const externalSpy = vi.fn();
      const stop = startUiBridge(externalSpy);

      await vi.waitFor(() => {
        expect(MockEventSource.instances).toHaveLength(1);
      });

      const es = MockEventSource.instances[0];
      // Must NOT contain %2C so backend RawQuery topic parser matches both topics
      expect(es.url).not.toContain("%2C");
      expect(es.url).toContain(`topics=${UI_NAVIGATE_EVENT}`);
      expect(es.url).toContain(`topics=${EXTERNAL_CHAT_TURN_EVENT}`);
      expect(es.url).toContain("token=test-secret-token");

      es.emit(UI_NAVIGATE_EVENT, JSON.stringify({ page: "topology", toast: "Topology view" }));
      expect(uiState.currentPage).toBe("topology");
      expect(globalToast).toHaveBeenCalledWith("info", "Topology view");

      // Malformed JSON frame should be ignored without throwing
      es.emit(UI_NAVIGATE_EVENT, "{bad-json");
      expect(uiState.currentPage).toBe("topology");

      es.emit(
        EXTERNAL_CHAT_TURN_EVENT,
        JSON.stringify({
          phase: "start",
          session_id: A2A_DEFAULT_SESSION_ID,
          mirror_ui: true,
          prompt: "Inspect network",
        }),
      );
      expect(externalSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          phase: "start",
          session_id: A2A_DEFAULT_SESSION_ID,
          mirror_ui: true,
          prompt: "Inspect network",
        }),
      );

      // Reconnects on error with backoff
      es.onerror?.();
      expect(es.closed).toBe(true);
      await vi.advanceTimersByTimeAsync(2000);
      expect(MockEventSource.instances).toHaveLength(2);

      stop();
      expect(MockEventSource.instances[1].closed).toBe(true);
    });

    it("subscribes via Tauri listen and handles race-safe teardown before async listen resolves", async () => {
      hoisted.isTauri = true;
      let releaseGate: (() => void) | undefined;
      const gatePromise = new Promise<void>((resolve) => {
        releaseGate = resolve;
      });
      hoisted.listenGate = () => gatePromise;

      // Tear down immediately before async import() / listen() resolves
      const earlyStop = startUiBridge();
      earlyStop();
      releaseGate?.();

      await vi.waitFor(() => {
        expect(hoisted.unlistenSpies.length).toBeGreaterThanOrEqual(2);
      });
      for (const spy of hoisted.unlistenSpies) {
        expect(spy).toHaveBeenCalledTimes(1);
      }

      // Normal Tauri lifecycle
      hoisted.listenGate = null;
      hoisted.unlistenSpies = [];
      const stop = startUiBridge();

      await vi.waitFor(() => {
        expect(hoisted.tauriListeners.has(UI_NAVIGATE_EVENT)).toBe(true);
        expect(hoisted.tauriListeners.has(EXTERNAL_CHAT_TURN_EVENT)).toBe(true);
      });

      hoisted.tauriListeners.get(UI_NAVIGATE_EVENT)?.({
        payload: { page: "models" },
      });
      expect(uiState.currentPage).toBe("models");

      hoisted.tauriListeners.get(EXTERNAL_CHAT_TURN_EVENT)?.({
        payload: {
          phase: "complete",
          session_id: DEFAULT_CONVERSATION_ID,
          prompt: "List models",
          reply: "llama3.2 is installed.",
        },
      });
      expect(aiState.messages).toHaveLength(2);

      stop();
      expect(hoisted.tauriListeners.size).toBe(0);
    });
  });
});

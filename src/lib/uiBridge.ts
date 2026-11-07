/**
 * Remote UI Control Bridge & A2A Session Isolation Layer.
 *
 * Subscribes once at app level (`App.svelte`) to `"ui-navigate"` and
 * `"agent-external-chat-turn"` across both transports (Tauri desktop IPC
 * events and Browser SSE `/api/events`).
 *
 * Enforces strict isolation for `colima-a2a-default-session`: automated CLI,
 * MCP, and A2A turns never pollute or hijack the user's active conversation
 * unless `mirror_ui: true` is explicitly requested, and even when mirrored
 * they never overwrite `aiState.activeConversationId` or `ai_active_conversation`.
 */

import { resolveApiBase, getApiToken } from "./api";
import { isRunningInTauri } from "./env";
import { globalToast, type ToastOptions, type ToastType } from "./globalToast";
import { newId } from "./ids";
import { uiState, isValidPage, openSettingsSection } from "../store.svelte";
import {
  A2A_DEFAULT_SESSION_ID,
  aiState,
  pushAiMessage,
  refreshConversations,
} from "../store/ai.svelte";
import { closeNotificationPanel } from "../store/notifications.svelte";

export { A2A_DEFAULT_SESSION_ID };

export const UI_NAVIGATE_EVENT = "ui-navigate";
export const EXTERNAL_CHAT_TURN_EVENT = "agent-external-chat-turn";

const BRIDGE_TOPICS = [UI_NAVIGATE_EVENT, EXTERNAL_CHAT_TURN_EVENT] as const;
const INITIAL_RETRY_MS = 2000;
const MAX_RETRY_MS = 30000;

export interface UiNavigatePayload {
  page?: string;
  section?: string | null;
  toast?: string | null;
}

export interface ExternalChatTurnPayload {
  phase?: "start" | "complete";
  session_id?: string;
  mirror_ui?: boolean;
  prompt?: string;
  reply?: string;
  navigated_to?: string | null;
}

export interface UiNavigateDeps {
  uiState?: {
    currentPage: string;
    aiPanelOpen?: boolean;
    settingsSection?: string | null;
  };
  aiState?: {
    isOpen: boolean;
  };
  isValidPage?: (page: string) => boolean;
  openSettingsSection?: (section: string) => void;
  showToast?: (type: ToastType, text: string, options?: ToastOptions) => void;
  closeNotificationPanel?: () => void;
}

/**
 * Decide whether an external agent chat turn should be mirrored onto the UI.
 *
 * Strict Isolation Rule: `colima-a2a-default-session` is strictly rejected
 * whenever `mirror_ui !== true`, even if `activeConversationId` somehow equals
 * `colima-a2a-default-session`.
 */
export function shouldMirrorExternalChatTurn(
  payload: ExternalChatTurnPayload,
  activeConversationId: string,
): boolean {
  if (!payload || typeof payload.session_id !== "string") return false;
  const sessionId = payload.session_id.trim();
  if (!sessionId) return false;

  if (sessionId === A2A_DEFAULT_SESSION_ID && payload.mirror_ui !== true) {
    return false;
  }

  const activeId = typeof activeConversationId === "string" ? activeConversationId.trim() : "";
  return payload.mirror_ui === true || (Boolean(activeId) && sessionId === activeId);
}

/**
 * Apply a validated `"ui-navigate"` payload to UI state.
 *
 * Returns `true` when `payload.page` resolved to `"ai-chat"` or one of the 17
 * `VALID_PAGES`, and `false` for empty, unknown, or prototype-key strings.
 */
export function applyUiNavigatePayload(
  payload: UiNavigatePayload,
  deps: UiNavigateDeps = {},
): boolean {
  if (!payload || typeof payload !== "object") return false;

  const targetUiState = deps.uiState ?? uiState;
  const targetAiState = deps.aiState ?? aiState;
  const checkValidPage = deps.isValidPage ?? isValidPage;
  const targetOpenSettingsSection = deps.openSettingsSection ?? openSettingsSection;
  const showToast = deps.showToast ?? globalToast;
  const closeNotif = deps.closeNotificationPanel ?? closeNotificationPanel;

  const page = typeof payload.page === "string" ? payload.page.trim().toLowerCase() : "";
  if (!page) return false;

  if (page === "ai-chat") {
    targetAiState.isOpen = true;
    closeNotif();
    if ("aiPanelOpen" in targetUiState) {
      targetUiState.aiPanelOpen = true;
    }
  } else if (checkValidPage(page)) {
    targetUiState.currentPage = page;
    const section = typeof payload.section === "string" ? payload.section.trim() : "";
    if (page === "settings" && section) {
      targetOpenSettingsSection(section);
    }
  } else {
    return false;
  }

  const toastText = typeof payload.toast === "string" ? payload.toast.trim() : "";
  if (toastText) {
    showToast("info", toastText);
  }

  return true;
}

/**
 * Mirror an authorized external chat turn into `aiState.messages` without
 * hijacking `aiState.activeConversationId` or re-persisting to SQLite.
 */
export function handleExternalChatTurn(payload: ExternalChatTurnPayload): boolean {
  if (!shouldMirrorExternalChatTurn(payload, aiState.activeConversationId)) {
    return false;
  }

  if (payload.mirror_ui === true) {
    aiState.isOpen = true;
    closeNotificationPanel();
    uiState.aiPanelOpen = true;
  }

  const prompt = typeof payload.prompt === "string" ? payload.prompt.trim() : "";
  const reply = typeof payload.reply === "string" ? payload.reply.trim() : "";
  const effectivePhase =
    payload.phase ?? (reply ? "complete" : prompt ? "start" : undefined);

  if (effectivePhase === "start") {
    if (prompt) {
      const last = aiState.messages[aiState.messages.length - 1];
      if (!(last && last.role === "user" && last.content === prompt)) {
        void pushAiMessage(
          { id: newId("ext-user"), role: "user", content: prompt },
          { persist: false },
        );
      }
    }
    return true;
  }

  if (effectivePhase === "complete") {
    const len = aiState.messages.length;
    const last = len >= 1 ? aiState.messages[len - 1] : undefined;
    const prev = len >= 2 ? aiState.messages[len - 2] : undefined;

    // Idempotency guard against duplicate `"complete"` frames on reconnect.
    if (
      reply &&
      last?.role === "assistant" &&
      last.content === reply &&
      (!prompt || (prev?.role === "user" && prev.content === prompt))
    ) {
      return true;
    }

    if (prompt) {
      const currentLast = aiState.messages[aiState.messages.length - 1];
      if (!(currentLast && currentLast.role === "user" && currentLast.content === prompt)) {
        void pushAiMessage(
          { id: newId("ext-user"), role: "user", content: prompt },
          { persist: false },
        );
      }
    }

    if (reply) {
      const currentLast = aiState.messages[aiState.messages.length - 1];
      if (
        !(currentLast && currentLast.role === "assistant" && currentLast.content === reply)
      ) {
        void pushAiMessage(
          { id: newId("ext-assistant"), role: "assistant", content: reply },
          { persist: false },
        );
      }
    }

    const sessionId = payload.session_id?.trim() ?? "";
    if (sessionId && sessionId !== A2A_DEFAULT_SESSION_ID) {
      void refreshConversations();
    }
    return true;
  }

  return false;
}

function normalizeEventPayload(raw: unknown): unknown {
  if (typeof raw === "string") {
    try {
      return JSON.parse(raw) as unknown;
    } catch {
      return null;
    }
  }
  return raw;
}

/**
 * Subscribe at app level to `"ui-navigate"` and `"agent-external-chat-turn"`.
 * Returns a synchronous teardown function safe to call even before async
 * Tauri `listen()` or Browser token/port discovery settles.
 */
export function startUiBridge(
  onExternalTurn?: (payload: ExternalChatTurnPayload) => void,
): () => void {
  const dispatch = (topic: string, rawPayload: unknown) => {
    const payload = normalizeEventPayload(rawPayload);
    if (!payload || typeof payload !== "object") return;
    if (topic === UI_NAVIGATE_EVENT) {
      applyUiNavigatePayload(payload as UiNavigatePayload);
    } else if (topic === EXTERNAL_CHAT_TURN_EVENT) {
      const turnHandler = onExternalTurn ?? handleExternalChatTurn;
      turnHandler(payload as ExternalChatTurnPayload);
    }
  };

  if (isRunningInTauri()) {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    import("@tauri-apps/api/event")
      .then(async (mod) => {
        for (const topic of BRIDGE_TOPICS) {
          const un = await mod.listen(topic, (e: { payload: unknown }) => {
            if (!disposed) dispatch(topic, e.payload);
          });
          if (disposed) {
            un();
          } else {
            unlisteners.push(un);
          }
        }
      })
      .catch(() => {
        // Ignore missing Tauri event runtime in non-webview environments.
      });

    return () => {
      disposed = true;
      while (unlisteners.length > 0) {
        const un = unlisteners.pop();
        un?.();
      }
    };
  }

  let source: EventSource | null = null;
  let cancelled = false;
  let retryTimeout: ReturnType<typeof setTimeout> | null = null;
  let retryDelay = INITIAL_RETRY_MS;

  async function connect() {
    if (source) {
      source.close();
      source = null;
    }
    if (cancelled) return;

    const token = await getApiToken();
    const base = await resolveApiBase();
    if (cancelled) return;
    if (typeof EventSource === "undefined") return;

    // Repeated `topics` parameters avoid `%2C` encoding: `misc::parse_topics`
    // reads the raw query string without percent-decoding.
    const params = new URLSearchParams();
    for (const topic of BRIDGE_TOPICS) {
      params.append("topics", topic);
    }
    if (token) params.set("token", token);

    const es = new EventSource(`${base}/api/events?${params}`);
    source = es;

    for (const topic of BRIDGE_TOPICS) {
      es.addEventListener(topic, (e: MessageEvent) => {
        if (cancelled) return;
        try {
          dispatch(topic, JSON.parse(e.data) as unknown);
        } catch {
          // Malformed SSE frames are dropped silently.
        }
      });
    }

    es.onopen = () => {
      retryDelay = INITIAL_RETRY_MS;
    };

    es.onerror = () => {
      es.close();
      if (source === es) source = null;
      if (cancelled) return;
      retryTimeout = setTimeout(() => {
        void connect();
      }, retryDelay);
      retryDelay = Math.min(retryDelay * 2, MAX_RETRY_MS);
    };
  }

  void connect();

  return () => {
    cancelled = true;
    if (retryTimeout !== null) {
      clearTimeout(retryTimeout);
      retryTimeout = null;
    }
    if (source) {
      source.close();
      source = null;
    }
  };
}

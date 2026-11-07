import { aiApi, type AiConversation } from "../lib/api";
import { newId } from "../lib/ids";
import { getAppSetting, setAppSetting } from "../lib/settingsStore.svelte";

export interface AiMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
}

/** Matches `DEFAULT_CONVERSATION_ID` in `ai_chat.rs`. */
export const DEFAULT_CONVERSATION_ID = "default";

/** Dedicated isolated conversation ID for automated external CLI / MCP / A2A turns. */
export const A2A_DEFAULT_SESSION_ID = "colima-a2a-default-session";

/**
 * Normalize a candidate conversation ID so transient A2A sessions can never
 * hijack `aiState.activeConversationId` or persist into `ai_active_conversation`.
 */
function sanitizeConversationId(id: string | null | undefined): string {
  if (!id) return DEFAULT_CONVERSATION_ID;
  const trimmed = id.trim();
  if (!trimmed || trimmed === A2A_DEFAULT_SESSION_ID) {
    return DEFAULT_CONVERSATION_ID;
  }
  return trimmed;
}

export const aiState = $state({
  messages: [] as AiMessage[],
  conversations: [] as AiConversation[],
  activeConversationId: DEFAULT_CONVERSATION_ID,
  isOpen: false,
  isProcessing: false,
  errorCount: 0,
});

export async function refreshConversations() {
  try {
    const list = (await aiApi.listConversations()) ?? [];
    aiState.conversations = list.filter(
      (c) => c.id.trim() !== A2A_DEFAULT_SESSION_ID,
    );
  } catch (e) {
    console.error("Failed to list AI conversations:", e);
  }
}

/**
 * Load the thread the user was last in. The active id is persisted so reopening
 * the app lands back where the conversation left off rather than in `default`.
 */
export async function initAiHistory() {
  const rawSaved = getAppSetting("ai_active_conversation", DEFAULT_CONVERSATION_ID);
  const saved = sanitizeConversationId(rawSaved);
  await refreshConversations();
  const exists = aiState.conversations.some((c) => c.id === saved);
  const targetId = exists ? saved : DEFAULT_CONVERSATION_ID;
  aiState.activeConversationId = targetId;
  setAppSetting("ai_active_conversation", targetId);
  try {
    const history = await aiApi.loadHistory(targetId);
    const loaded = ((history as AiMessage[]) ?? []).filter((m) => m.content);
    // Preserve any messages pushed during startup (e.g. fast starter click,
    // auto-error trigger, or pre-seeded state) instead of clobbering them.
    if (aiState.messages.length > 0) {
      const loadedIds = new Set(loaded.map((m) => m.id));
      const newer = aiState.messages.filter((m) => !loadedIds.has(m.id));
      aiState.messages = [...loaded, ...newer];
    } else {
      aiState.messages = loaded;
    }
  } catch (e) {
    console.error("Failed to load AI chat history:", e);
    if (aiState.messages.length === 0) aiState.messages = [];
  }
}

export async function switchConversation(id: string) {
  const targetId = sanitizeConversationId(id);
  aiState.activeConversationId = targetId;
  setAppSetting("ai_active_conversation", targetId);
  try {
    const history = await aiApi.loadHistory(targetId);
    // Empty rows carry no conversation and render as a bare "Colima AI" label.
    // Threads saved before assistant turns were persisted correctly are full of
    // them, so they are dropped on the way in rather than left to accumulate on
    // screen.
    aiState.messages = ((history as AiMessage[]) ?? []).filter((m) => m.content);
  } catch (e) {
    console.error("Failed to load AI chat history:", e);
    aiState.messages = [];
  }
}

export async function createConversation(title = ""): Promise<string> {
  const id = newId("conv");
  try {
    await aiApi.createConversation(id, title);
  } catch (e) {
    console.error("Failed to create AI conversation:", e);
  }
  aiState.messages = [];
  aiState.activeConversationId = id;
  setAppSetting("ai_active_conversation", id);
  await refreshConversations();
  return id;
}

export async function renameConversation(id: string, title: string) {
  try {
    await aiApi.renameConversation(id, title);
    await refreshConversations();
  } catch (e) {
    console.error("Failed to rename AI conversation:", e);
  }
}

export async function deleteConversation(id: string) {
  try {
    await aiApi.deleteConversation(id);
  } catch (e) {
    console.error("Failed to delete AI conversation:", e);
  }
  await refreshConversations();
  // Deleting the thread on screen leaves nothing to show, so fall back to the
  // most recent remaining one (the list is ordered newest-first).
  if (aiState.activeConversationId === id) {
    const next = aiState.conversations[0]?.id ?? DEFAULT_CONVERSATION_ID;
    await switchConversation(next);
  }
}

export async function pushAiMessage(
  msg: AiMessage,
  options: { persist?: boolean } = {},
) {
  // Defence in depth against a duplicate id from any source. The message list
  // is rendered by a keyed `{#each}`, which throws on duplicates, and the
  // backend upserts on id — so a collision would also overwrite the earlier
  // message in the saved history. Re-key rather than drop: losing a message is
  // worse than showing one with a different id.
  const safe = aiState.messages.some((m) => m.id === msg.id)
    ? { ...msg, id: newId() }
    : msg;

  // Captured before the await: the user can switch threads mid-request, and the
  // message belongs to the thread it was sent from.
  const conversationId = aiState.activeConversationId;
  aiState.messages.push(safe);

  // If persistence is disabled for this turn (e.g. In-flight streaming or draft),
  // skip writing to SQLite. Persistence will happen via persistAiMessages at turn end.
  if (options.persist === false) return;

  // An assistant turn is pushed empty and filled by the stream. Saving it here
  // wrote `content = ''` to the history, and nothing ever rewrote it — so every
  // reply came back blank after a reload. Empty messages are persisted by
  // `persistAiMessages` once they have text.
  if (!safe.content) return;

  try {
    await aiApi.saveMessage(safe, conversationId);
  } catch (e) {
    console.error("Failed to save AI chat message:", e);
  }
}

/** Update the text of a message on screen. Persistence happens at turn end. */
export function updateAiMessage(id: string, content: string) {
  const index = aiState.messages.findIndex((m) => m.id === id);
  if (index !== -1) aiState.messages[index].content = content;
}

/**
 * Drop a message from the thread.
 *
 * Nothing is removed from the database because a message is only written once
 * it has settled — a turn discarded before then was never stored.
 */
export function deleteAiMessage(id: string) {
  const index = aiState.messages.findIndex((m) => m.id === id);
  if (index !== -1) aiState.messages.splice(index, 1);
}

/**
 * Write the messages of a finished turn to the history.
 *
 * Called once the agent settles rather than per streamed chunk: the backend
 * upserts on id, so a token-by-token save would be one write per token for the
 * same row. Messages still empty at this point are skipped.
 *
 * Iterates through `aiState.messages` (preserving visual UI display order)
 * and filters by `ids` if provided, ensuring 100% array ordering fidelity.
 */
export async function persistAiMessages(
  ids?: Iterable<string>,
  options?: { conversationId?: string; snapshot?: AiMessage[] },
) {
  const conversationId = options?.conversationId ?? aiState.activeConversationId;
  const source =
    options?.snapshot && aiState.activeConversationId !== conversationId
      ? options.snapshot
      : aiState.messages;
  const idSet = ids ? new Set(ids) : null;
  const toSave = idSet
    ? source.filter((m) => idSet.has(m.id))
    : source;

  for (const message of toSave) {
    if (!message?.content) continue;
    try {
      await aiApi.saveMessage(message, conversationId);
    } catch (e) {
      console.error("Failed to save AI chat message:", e);
    }
  }
}

/** Empties the active thread without removing it from the conversation list. */
export async function clearAiHistory() {
  const id = aiState.activeConversationId;
  aiState.messages = [];
  aiState.errorCount = 0;
  try {
    await aiApi.clearHistory(id);
    await refreshConversations();
  } catch (e) {
    console.error("Failed to clear AI chat history:", e);
  }
}

/**
 * Truncates the active conversation starting from `messageId` onwards.
 * Used for "Retry / Regenerate" (assistant messages) and "Edit & Resend" (user messages).
 *
 * 1. Drops `[messageId, ...]` from in-memory `aiState.messages`.
 * 2. Wipes SQLite messages for active conversation via `aiApi.clearHistory(conversationId)`.
 * 3. Re-saves the remaining messages via `aiApi.saveMessage(m, conversationId)`.
 * 4. Requires 0 SQLite schema changes or migrations in `src-tauri`.
 */
export async function truncateAiMessagesFrom(messageId: string): Promise<void> {
  const index = aiState.messages.findIndex((m) => m.id === messageId);
  if (index === -1) return;

  // Keep all turns strictly before the target message
  aiState.messages = aiState.messages.slice(0, index);

  const conversationId = aiState.activeConversationId;
  try {
    await aiApi.clearHistory(conversationId);
    for (const message of aiState.messages) {
      if (!message.content) continue;
      await aiApi.saveMessage(message, conversationId);
    }
    await refreshConversations();
  } catch (e) {
    console.error("Failed to truncate AI chat history:", e);
  }
}


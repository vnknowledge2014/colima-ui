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
    aiState.conversations = (await aiApi.listConversations()) ?? [];
  } catch (e) {
    console.error("Failed to list AI conversations:", e);
  }
}

/**
 * Load the thread the user was last in. The active id is persisted so reopening
 * the app lands back where the conversation left off rather than in `default`.
 */
export async function initAiHistory() {
  const saved = getAppSetting("ai_active_conversation", DEFAULT_CONVERSATION_ID);
  await refreshConversations();
  const exists = aiState.conversations.some((c) => c.id === saved);
  await switchConversation(exists ? saved : DEFAULT_CONVERSATION_ID);
}

export async function switchConversation(id: string) {
  aiState.activeConversationId = id;
  setAppSetting("ai_active_conversation", id);
  try {
    const history = await aiApi.loadHistory(id);
    aiState.messages = (history as AiMessage[]) ?? [];
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

export async function pushAiMessage(msg: AiMessage) {
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
  try {
    await aiApi.saveMessage(safe, conversationId);
  } catch (e) {
    console.error("Failed to save AI chat message:", e);
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

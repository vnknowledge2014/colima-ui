export interface AiMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
}

export const aiState = $state({
  messages: [] as AiMessage[],
  isOpen: false,
  isProcessing: false,
  errorCount: 0,
});

import { aiApi } from "../lib/api";

export async function initAiHistory() {
  try {
    const history = await aiApi.loadHistory();
    if (Array.isArray(history) && history.length > 0) {
      aiState.messages = history as AiMessage[];
    }
  } catch (e) {
    // Graceful degradation — chat history simply starts empty
    console.warn("AI chat history unavailable (backend may not support it yet):", e);
  }
}

export async function pushAiMessage(msg: AiMessage) {
  aiState.messages.push(msg);
  try {
    await aiApi.saveMessage(msg);
  } catch {
    // Non-fatal — message is shown in UI regardless
  }
}

export async function clearAiHistory() {
  aiState.messages = [];
  aiState.errorCount = 0;
  try {
    await aiApi.clearHistory();
  } catch {
    // Non-fatal
  }
}

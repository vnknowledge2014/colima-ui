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
    if (history && history.length > 0) {
      aiState.messages = history as AiMessage[];
    }
  } catch (e) {
    console.error("Failed to load AI chat history:", e);
  }
}

export async function pushAiMessage(msg: AiMessage) {
  aiState.messages.push(msg);
  try {
    await aiApi.saveMessage(msg);
  } catch (e) {
    console.error("Failed to save AI chat message:", e);
  }
}

export async function clearAiHistory() {
  aiState.messages = [];
  aiState.errorCount = 0;
  try {
    await aiApi.clearHistory();
  } catch (e) {
    console.error("Failed to clear AI chat history:", e);
  }
}

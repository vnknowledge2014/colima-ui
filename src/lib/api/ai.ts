import { call } from "./client";

// ===== AI Chat API =====

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

/** A chat thread in the AI panel. Mirrors `AiConversation` in `ai_chat.rs`. */
export interface AiConversation {
  id: string;
  title: string;
  updated_at: number;
  message_count: number;
  preview: string;
}

export const aiApi = {
  chat: (provider: string, model: string, apiKey: string, messages: ChatMessage[], endpoint = "") =>
    call<string>("ai_chat", {
      request: { provider, model, api_key: apiKey, messages, endpoint }
    }, "POST", "/api/ai/chat", undefined, {
      provider, model, api_key: apiKey, messages, endpoint
    }),
  listModels: (provider: string, apiKey: string, endpoint = "") =>
    call<string>("ai_list_models", {
      provider, api_key: apiKey, endpoint
    }, "POST", "/api/ai/models", undefined, {
      provider, api_key: apiKey, endpoint
    }),
  search: (query: string, instances?: string[], maxResults?: number, timeoutSecs?: number) =>
    call<SearchResult[]>("searxng_search", {
      query, instances, max_results: maxResults, timeout_secs: timeoutSecs
    }, "POST", "/api/ai/search", undefined, {
      query, instances, max_results: maxResults, timeout_secs: timeoutSecs
    }),
  fetchPageMarkdown: (url: string, maxLength?: number, mode?: string) =>
    call<string>("fetch_page_as_markdown", {
      url, max_length: maxLength, mode
    }, "POST", "/api/ai/fetch-page", undefined, {
      url, max_length: maxLength, mode
    }),
  getAppContext: () =>
    call<string>("get_app_context", {}, "GET", "/api/ai/context"),
  loadHistory: (conversationId?: string) =>
    call<ChatMessage[]>("ai_chat_load_history", { conversationId }, "GET", "/api/ai/history",
      conversationId ? { conversationId } : undefined),
  saveMessage: (message: ChatMessage, conversationId?: string) =>
    call<void>("ai_chat_save_message", { message, conversationId }, "POST", "/api/ai/history", undefined, { message, conversationId }),
  // Distinct path from `saveMessage`: both are POSTs and would otherwise share
  // one route, leaving the server to guess which one the caller meant.
  clearHistory: (conversationId?: string) =>
    call<void>("ai_chat_clear_history", { conversationId }, "POST", "/api/ai/history/clear", undefined, { conversationId }),
  listConversations: () =>
    call<AiConversation[]>("ai_chat_list_conversations", {}, "GET", "/api/ai/conversations"),
  createConversation: (id: string, title = "") =>
    call<void>("ai_chat_create_conversation", { id, title }, "POST", "/api/ai/conversations", undefined, { id, title }),
  renameConversation: (id: string, title: string) =>
    call<void>("ai_chat_rename_conversation", { id, title }, "POST", "/api/ai/conversations/rename", undefined, { id, title }),
  deleteConversation: (id: string) =>
    call<void>("ai_chat_delete_conversation", { id }, "POST", "/api/ai/conversations/delete", undefined, { id }),
  readReference: (path: string) =>
    call<string>("read_reference", { path }, "GET", "/api/system/reference", { path }),
};

export interface SearchResult {
  title: string;
  url: string;
  content: string;
  engine: string;
}

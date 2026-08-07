import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== AI Chat API =====

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
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
};

export interface SearchResult {
  title: string;
  url: string;
  content: string;
  engine: string;
}

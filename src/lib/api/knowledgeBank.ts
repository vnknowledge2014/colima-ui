import { call } from "./client";
import type { ColimaInstance, InstanceStatus, StartConfig, DockerContainer, DockerImage, SystemInfo, AiModel, DockerVolume, DockerNetwork } from "./types";

// ===== Knowledge Bank API =====

export interface AgentMemoryItem {
  id: string;
  memory_type: string;
  content: string;
  created_at: string;
}

export const knowledgeBankApi = {
  query: (errorText: string) =>
    call<any>("kb_query", { error_text: errorText }, "POST", "/api/kb/query", undefined, { error_text: errorText }),
  searchMemory: (query: string, limit = 10) =>
    call<string[]>("search_memory", { query, limit }, "POST", "/api/kb/search", undefined, { query, limit }),
  getAllMemories: () =>
    call<AgentMemoryItem[]>("get_all_memories", undefined, "GET", "/api/kb/memories"),
  updateMemory: (id: string, content: string) =>
    call<string>("update_memory", { id, content }, "POST", "/api/kb/memories/update", undefined, { id, content }),
  deleteMemory: (id: string) =>
    call<string>("delete_memory", { id }, "POST", "/api/kb/memories/delete", undefined, { id }),
  collectDiagnosticLogs: (profile: string) =>
    call<string>("collect_diagnostic_logs", { profile }, "GET", "/api/diagnostics/logs", { profile }),
};

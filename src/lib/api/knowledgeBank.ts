import { call } from "./client";

// ===== Knowledge Bank API =====

export interface AgentMemoryItem {
  id: string;
  memory_type: string;
  content: string;
  created_at: number;
}

export const knowledgeBankApi = {
  query: (errorText: string) =>
    call<{ context_text: string }>("kb_query", { errorText }, "POST", "/api/kb/query", undefined, { error_text: errorText }),
  feedback: (solutionId: number, isLike: boolean) =>
    call<string>("kb_feedback", { solutionId, isLike }, "POST", "/api/kb/feedback", undefined, { solutionId, isLike }),
  learn: (errorPattern: string, solutionText: string) =>
    call<number>("kb_learn", { errorPattern, solutionText }, "POST", "/api/kb/learn", undefined, { errorPattern, solutionText }),
  addMemory: (memoryType: string, content: string) =>
    call<string>("add_memory", { memoryType, content }, "POST", "/api/kb/memories/add", undefined, { memoryType, content }),
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

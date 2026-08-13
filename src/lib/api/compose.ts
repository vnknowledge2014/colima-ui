import { call } from "./client";

// ===== Compose API =====

export interface ComposeProject {
  Name: string;
  Status: string;
  ConfigFiles: string;
}

/** Mirrors `ComposeValidation` in `compose_diagnose.rs`. */
export interface ComposeValidation {
  valid: boolean;
  raw_error: string;
  signature: string;
  category: string;
}

/** Mirrors `KBMatch` in `knowledge_bank.rs`. */
export interface ComposeKBSolution {
  id: number;
  error_pattern: string;
  error_category: string;
  solution_text: string;
  root_cause: string;
  commands: string;
  likes: number;
  dislikes: number;
  source: string;
}

/** Mirrors `ComposeDiagnosis` in `compose_diagnose.rs`. */
export interface ComposeDiagnosis {
  validation: ComposeValidation;
  kb_solutions: ComposeKBSolution[];
  kb_anti_patterns: { id: number; error_pattern: string; bad_suggestion: string; reason: string }[];
  /** Already redacted backend-side. Shown to the user before anything is sent. */
  llm_payload_preview: string;
  needs_llm: boolean;
}

/** Mirrors `Patch` in `compose_autofix.rs`. */
export interface ComposePatch {
  id: string;
  unified_diff: string;
  /** The whole file as it would be written. Applying is a straight write. */
  new_content: string;
  explanation: string;
  source: "deterministic" | "kb" | "llm";
  confidence: number;
  /** False when the fix reflows the document and the user's comments are lost. */
  comments_preserved: boolean;
  declared_removals: string[];
  category: string;
}

/** Mirrors `AutofixProposal` in `compose_autofix.rs`. */
export interface ComposeAutofixProposal {
  patches: ComposePatch[];
  resolved: boolean;
  /** What Docker still reports when `resolved` is false. */
  remaining_error: string;
  /** Patches the backend key check refused. Non-empty means a fixer defect. */
  refused: { path: string }[];
}

/** Mirrors `FixRecord` in `compose_autofix_apply.rs`. */
export interface ComposeFixRecord {
  fix_id: string;
  file_path: string;
  applied_at: string;
  explanation: string;
  undone: boolean;
}

export const composeApi = {
  list: async (): Promise<ComposeProject[]> => {
    const raw = await call<unknown>("list_compose_projects", undefined, "GET", "/api/compose");
    if (!raw) return [];

    // Normalize field names: Tauri IPC returns snake_case (name, status, config_files)
    // but TypeScript interface expects PascalCase (Name, Status, ConfigFiles)
    const normalize = (items: unknown[]): ComposeProject[] =>
      items.map((item) => {
        const v = item as Record<string, unknown>;
        return {
          Name: String(v.Name || v.name || ""),
          Status: String(v.Status || v.status || ""),
          ConfigFiles: String(v.ConfigFiles || v.config_files || v.configFiles || ""),
        };
      });

    // Tauri IPC may return parsed array directly
    if (Array.isArray(raw)) return normalize(raw);
    if (typeof raw === 'string') {
      if (!raw.trim()) return [];
      try { return normalize(JSON.parse(raw)); } catch { return []; }
    }
    return [];
  },
  up: (projectDir = "", detach = true) =>
    call<string>("compose_up", { projectDir, detach }, "POST", "/api/compose/up", undefined, { projectDir, detach }),
  down: (projectName: string) =>
    call<string>("compose_down", { projectName }, "POST", "/api/compose/down", undefined, { projectName }),
  restart: (projectName: string) =>
    call<string>("compose_restart", { projectName }, "POST", "/api/compose/restart", undefined, { projectName }),
  logs: (projectName: string, lines = 200) =>
    call<string>("compose_logs", { projectName, lines }, "GET", "/api/compose/logs", { projectName, lines: String(lines) }),
  ps: (projectName: string) =>
    call<string>("compose_ps", { projectName }, "GET", "/api/compose/ps", { projectName }),
  diagnose: (filePath: string) =>
    call<ComposeDiagnosis>("compose_diagnose", { filePath }, "POST", "/api/compose/diagnose", undefined, { filePath }),
  autofixPropose: (filePath: string) =>
    call<ComposeAutofixProposal>("compose_autofix_propose", { filePath }, "POST", "/api/compose/autofix/propose", undefined, { filePath }),
  /**
   * `declaredRemovals` travels with the content because the backend re-runs its
   * key check on apply and cannot tell an intended rename from an accidental
   * deletion without it. Pass the patch's own field, never an empty array.
   */
  autofixApply: (filePath: string, patch: ComposePatch) =>
    call<ComposeFixRecord>(
      "compose_autofix_apply",
      { filePath, newContent: patch.new_content, explanation: patch.explanation, declaredRemovals: patch.declared_removals },
      "POST",
      "/api/compose/autofix/apply",
      undefined,
      { filePath, newContent: patch.new_content, explanation: patch.explanation, declaredRemovals: patch.declared_removals },
    ),
  autofixUndo: (fixId: string) =>
    call<ComposeFixRecord>("compose_autofix_undo", { fixId }, "POST", "/api/compose/autofix/undo", undefined, { fixId }),
  autofixHistory: () =>
    call<ComposeFixRecord[]>("compose_autofix_history", undefined, "GET", "/api/compose/autofix/history"),
};

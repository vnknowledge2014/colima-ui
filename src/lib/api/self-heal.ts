import { call } from "./client";

/**
 * Self-healing rules: what the app may repair without being asked each time.
 *
 * **`autoCapable` is the backend's word, not a UI guess.** Two of the five
 * rules only ever suggest, and the executor has no branch that could run them.
 * The UI reads that flag rather than hard-coding which rules those are, so the
 * two can never disagree about what the machine will do.
 */

export type HealTrigger =
  | "unhealthy"
  | "crash_loop"
  | "disk_full"
  | "oom_killed"
  | "vm_unresponsive";

export type HealAction =
  | "restart_container"
  | "stop_container"
  | "restart_vm"
  | "suggest_prune"
  | "suggest_mem_limit";

export type HealMode = "suggest" | "auto";

/** What became of one firing. Mirrors `HealOutcome` in `self_heal.rs`. */
export type HealOutcome =
  | "executed"
  | "failed"
  | "suggested"
  | "quota_blocked"
  | "switched_off";

export interface HealRule {
  id: number;
  name: string;
  trigger: HealTrigger;
  action: HealAction;
  mode: HealMode;
  /** Minutes unhealthy, deaths per window, or percent of disk — per trigger. */
  threshold: number;
  /** Window for `crash_loop`, in seconds. Ignored by the other triggers. */
  windowSecs: number;
  maxPerHour: number;
  enabled: boolean;
  /** False for the two rules that can only ever advise. */
  autoCapable: boolean;
}

export interface HealLogEntry {
  id: number;
  ts: number;
  ruleId: number;
  ruleName: string;
  containerId: string;
  containerName: string;
  action: HealAction;
  mode: HealMode;
  outcome: HealOutcome;
  /** Never empty: an unexplained entry is the silent action the log exists to prevent. */
  detail: string;
}

export const selfHealApi = {
  listRules: () =>
    call<HealRule[]>("self_heal_list_rules", undefined, "GET", "/api/self-heal/rules"),

  saveRule: (
    id: number,
    mode: HealMode,
    threshold: number,
    maxPerHour: number,
    enabled: boolean,
  ) =>
    call<void>(
      "self_heal_save_rule",
      { id, mode, threshold, maxPerHour, enabled },
      "POST",
      "/api/self-heal/rules",
      undefined,
      { id, mode, threshold, maxPerHour, enabled },
    ),

  recentLog: (limit = 50) =>
    call<HealLogEntry[]>("self_heal_recent_log", { limit }, "GET", "/api/self-heal/log", {
      limit: String(limit),
    }),

  isEnabled: () => call<boolean>("self_heal_is_enabled", undefined, "GET", "/api/self-heal/enabled"),

  setEnabled: (on: boolean) =>
    call<void>("self_heal_set_enabled", { on }, "POST", "/api/self-heal/enabled", undefined, {
      on,
    }),
};

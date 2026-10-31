import { call } from "./client";

// ===== Environment resolution record =====
// "What does the app believe, and why?" — provenance-tagged path decisions
// plus the launchd-vs-login-shell environment duality. Backend:
// commands/resolution.rs (+ HTTP mirror routes/resolution.rs).

export interface EnvVarDuality {
  key: string;
  /** What this process (launchd-spawned) actually received. */
  runtime: string | null;
  /** What a login shell would export — the env the user believes is set. */
  shell: string | null;
  diverges: boolean;
}

export interface EnvironmentRow {
  /** colima-home | lima-home | docker-socket | kubeconfig | colima-profile-config */
  key: string;
  /** pinned | env | auto-resolve | discovered | fallback */
  rule: string;
  chosen: string;
  detail: string | null;
  health: string | null;
}

export interface EnvironmentReport {
  rows: EnvironmentRow[];
  envDuality: EnvVarDuality[];
  /** Non-fatal reason the shell snapshot is incomplete, if it is. */
  shellSnapshotError: string | null;
}

export const environmentApi = {
  getResolutions: () =>
    call<EnvironmentReport>("get_environment_resolutions", undefined, "GET", "/api/environment/resolutions"),
};
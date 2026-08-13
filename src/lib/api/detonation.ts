import { call } from "./client";

/**
 * Detonation sessions: run an untrusted image in a disposable isolated
 * instance and watch what it does.
 *
 * Mirrors `src-tauri/src/commands/detonation.rs`.
 *
 * # What the UI is required to say, and forbidden from saying
 *
 * A Lima VM on macOS is a real boundary, not a malware-analysis boundary. The
 * wording is "observe behaviour in an isolated environment"; anything implying a
 * user may safely detonate real malware is out of bounds. That is a product
 * constraint carried in `plans/260812-1013-security-posture/phase-06-*`, and it
 * is why the panel's warning is a permanent banner rather than a tooltip.
 *
 * # Why this polls instead of subscribing
 *
 * The backend does publish `detonation.event` / `detonation.status` on SSE, and
 * a subscriber would be marginally more responsive. It would also be a third
 * near-copy of the `EventSource` lifecycle in `transferEvents.ts`, which already
 * documents that the stream is lossy and that callers must reconcile against an
 * authoritative snapshot anyway. A session lasts a couple of minutes and carries
 * a few dozen events, so the reconcile *is* the cheap path here: `get()` returns
 * the whole session, and polling it every couple of seconds cannot drift.
 */

export type DetonationEventKind =
  | "lifecycle"
  | "process"
  | "filesystem"
  | "output"
  | "error";

export type DetonationStatus =
  | "preparing"
  | "running"
  | "completed"
  | "timed_out"
  | "cancelled"
  | "failed";

export interface DetonationEvent {
  tsMs: number;
  kind: DetonationEventKind;
  detail: string;
}

export interface DetonationSession {
  id: string;
  image: string;
  status: DetonationStatus;
  startedMs: number;
  exitCode?: number | null;
  error?: string | null;
  timeline: DetonationEvent[];
}

export interface DetonationConfig {
  image: string;
  /** Clamped backend-side to 900s. */
  timeoutSecs?: number;
  /** Replaces the image's own command, for images that exit immediately. */
  cmdOverride?: string;
}

/** A session that has stopped changing, so polling can stop too. */
export function isTerminal(status: DetonationStatus): boolean {
  return (
    status === "completed" ||
    status === "timed_out" ||
    status === "cancelled" ||
    status === "failed"
  );
}

export const detonationApi = {
  /** Returns the session id immediately; the instance is created in the background. */
  start: (config: DetonationConfig) =>
    call<string>(
      "detonation_start",
      { config },
      "POST",
      "/api/detonation/start",
      undefined,
      config
    ),

  /**
   * Stop a running session. Never gated on entitlement — a stop control behind
   * the same gate as the feature it stops leaves a lapsed user with a VM they
   * can see running and cannot reach.
   */
  cancel: (sessionId: string) =>
    call<boolean>(
      "detonation_cancel",
      { sessionId },
      "POST",
      "/api/detonation/cancel",
      undefined,
      { sessionId }
    ),

  get: (sessionId: string) =>
    call<DetonationSession | null>(
      "detonation_get",
      { sessionId },
      "POST",
      "/api/detonation/session",
      undefined,
      { sessionId }
    ),

  list: () =>
    call<DetonationSession[]>("detonation_list", undefined, "GET", "/api/detonation/list"),

  /** Writes the timeline as JSON; the backend confines the path to `dir`. */
  export: (sessionId: string, dir: string, filename: string) =>
    call<string>(
      "detonation_export",
      { sessionId, dir, filename },
      "POST",
      "/api/detonation/export",
      undefined,
      { sessionId, dir, filename }
    ),
};

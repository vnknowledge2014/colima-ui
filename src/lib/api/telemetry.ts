import { call } from "./client";
import { isRunningInTauri } from "../env";

/**
 * Fire-and-forget telemetry recording. Tauri-only (recording is a local concern
 * with no REST surface) and best-effort — a failure never disrupts the action
 * that triggered it. Consent is enforced backend-side, so this is safe to call
 * unconditionally; nothing is stored unless the user opted in.
 */
async function recordEvent(cmd: string, args?: Record<string, unknown>): Promise<void> {
  if (!isRunningInTauri()) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke(cmd, args);
  } catch { /* best effort */ }
}

/** Mirrors `Consent` in `src-tauri/src/telemetry/consent.rs`. */
export type Consent = "unset" | "declined" | "granted";

/** One recorded event, as it would be sent. Shape mirrors `TelemetryEvent`. */
export interface TelemetryEvent {
  event: string;
  [field: string]: unknown;
}

export const telemetryApi = {
  consent: () =>
    call<Consent>("telemetry_consent", undefined, "GET", "/api/telemetry/consent"),
  shouldPrompt: () =>
    call<boolean>("telemetry_should_prompt", undefined, "GET", "/api/telemetry/should-prompt"),
  setConsent: (granted: boolean) =>
    call<unknown>("telemetry_set_consent", { granted }, "POST", "/api/telemetry/consent", undefined, { granted }),
  preview: () =>
    call<TelemetryEvent[]>("telemetry_preview", undefined, "GET", "/api/telemetry/preview"),

  // Funnel events. Args are validated against closed Rust enums server-side.
  recordFeatureUsed: (feature: string) => recordEvent("telemetry_feature_used", { feature }),
  recordProGateReached: (capability: string) => recordEvent("telemetry_pro_gate_reached", { capability }),
  recordCheckoutOpened: () => recordEvent("telemetry_checkout_opened"),
};

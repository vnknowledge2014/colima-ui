import { redactError } from "./redact";

/**
 * Frontend crash reporting, redacted at the source.
 *
 * `window.onerror` and unhandled promise rejections carry free-form messages
 * that can include a request URL with an API key — the same leak the Rust panic
 * hook guards against. Every message passes through `redact()` before it is
 * logged. Transmission is deferred until a telemetry endpoint exists; redacting
 * before it hits the console (and any screenshot of it) is the value now.
 *
 * Install once, early. Safe to call in browser mode.
 */
export function installCrashReporter(): void {
  if (typeof window === "undefined") return;

  window.addEventListener("error", (event) => {
    const safe = redactError(event.error ?? event.message);
    console.error("[crash]", safe);
  });

  window.addEventListener("unhandledrejection", (event) => {
    const safe = redactError(event.reason);
    console.error("[crash:unhandled]", safe);
  });
}

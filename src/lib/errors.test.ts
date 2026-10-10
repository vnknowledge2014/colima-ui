import { describe, it, expect } from "vitest";
import {
  normalizeError,
  toAppException,
  AppErrorException,
  errorTitle,
  errorMessage,
  errorHint,
  type AppError,
} from "./errors";

/** What the Rust `ColimaError` serializes to. */
const backendError = {
  code: "not_running",
  detail: "Cannot connect to the Docker daemon at unix:///var/run/docker.sock",
  command: "colima start --profile dev",
  exit_code: 1,
  hint: "The Colima VM is not running. Start it from the Instances page.",
  doc_id: "start-colima",
};

describe("normalizeError", () => {
  it("maps the backend contract, converting snake_case to camelCase", () => {
    const e = normalizeError(backendError);
    expect(e.code).toBe("not_running");
    expect(e.detail).toContain("Cannot connect to the Docker daemon");
    expect(e.command).toBe("colima start --profile dev");
    expect(e.exitCode).toBe(1);
    expect(e.docId).toBe("start-colima");
  });

  it("produces identical results for both transports", () => {
    // Tauri rejects with the object; HTTP delivers the same object as
    // `json.error`. Divergence here is the regression this phase exists to
    // prevent.
    const fromTauri = normalizeError(backendError);
    const fromHttp = normalizeError(JSON.parse(JSON.stringify(backendError)));
    expect(fromHttp).toEqual(fromTauri);
  });

  it("falls back to unknown for an unrecognised code", () => {
    expect(normalizeError({ code: "wat", detail: "x" }).code).toBe("unknown");
  });

  it("handles plain strings", () => {
    const e = normalizeError("boom");
    expect(e).toEqual({ code: "unknown", detail: "boom", command: undefined, exitCode: undefined, hint: undefined, docId: undefined });
  });

  it("handles Error instances", () => {
    expect(normalizeError(new Error("boom")).detail).toBe("boom");
  });

  it("handles the legacy { type, message } shape", () => {
    const e = normalizeError({ type: "CommandFailed", message: "old style" });
    expect(e.code).toBe("unknown");
    expect(e.detail).toBe("old style");
  });

  it("handles null and undefined without throwing", () => {
    expect(normalizeError(null).code).toBe("unknown");
    expect(normalizeError(undefined).code).toBe("unknown");
  });

  it("redacts secrets in detail", () => {
    const e = normalizeError({
      code: "network",
      detail: "Request failed: https://x.test/v1?key=AIzaSy0123456789abcdefghijkl",
    });
    expect(e.detail).not.toContain("AIzaSy0123456789abcdefghijkl");
  });

  it("is idempotent through the exception wrapper", () => {
    const once = toAppException(backendError);
    expect(normalizeError(once)).toEqual(once.appError);
    expect(toAppException(once)).toBe(once);
  });
});

describe("AppErrorException", () => {
  it("stringifies to readable text, not [object Object]", () => {
    // ~114 existing call sites do globalToast("error", String(e)). If this
    // regresses, all of them start showing [object Object].
    const e = toAppException(backendError);
    const s = String(e);
    expect(s).not.toContain("[object Object]");
    expect(s).toContain("Cannot connect to the Docker daemon");
  });

  it("interpolates the same way in a template literal", () => {
    const e = toAppException(backendError);
    expect(`${e}`).toBe(String(e));
  });

  it("is still an Error, so instanceof checks keep working", () => {
    const e = toAppException("boom");
    expect(e).toBeInstanceOf(Error);
    expect(e).toBeInstanceOf(AppErrorException);
    expect(e.message).toBe("boom");
  });
});

describe("presentation helpers", () => {
  const err: AppError = { code: "not_running", detail: "daemon down" };

  it("gives an English title when no locale entry exists", () => {
    expect(errorTitle(err)).toBe("Colima is not running");
  });

  it("builds a message from title and detail", () => {
    expect(errorMessage(err)).toBe("Colima is not running: daemon down");
  });

  it("does not repeat the detail when it equals the title", () => {
    expect(errorMessage({ code: "not_running", detail: "Colima is not running" })).toBe(
      "Colima is not running",
    );
  });

  it("omits the generic placeholder title for unclassified errors", () => {
    // "Something went wrong: Command k8s_resources not found" buries the useful
    // half behind a placeholder.
    expect(errorMessage({ code: "unknown", detail: "Command k8s_resources not found" })).toBe(
      "Command k8s_resources not found",
    );
  });

  // Hint precedence: locale entry > backend English hint > nothing.
  it("prefers the localized hint over the backend fallback", () => {
    // `not_running` has a hint in src/locales/en.json.
    const hint = errorHint({ ...err, hint: "backend english hint" });
    expect(hint).toBeDefined();
    expect(hint).not.toBe("backend english hint");
  });

  it("falls back to the backend hint when the locale has none", () => {
    // `command_failed` deliberately carries no hint in the locale files —
    // there is no generic advice worth showing for it.
    expect(
      errorHint({ code: "command_failed", detail: "x", hint: "Start it from Instances." }),
    ).toBe("Start it from Instances.");
  });

  it("returns undefined when neither the locale nor the backend has a hint", () => {
    expect(errorHint({ code: "command_failed", detail: "x" })).toBeUndefined();
  });
});

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let tauriMode = false;
const mockInvoke = vi.fn();

vi.mock("../env", () => ({
  isRunningInTauri: () => tauriMode,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe("api/client dual-mode transport & HTTP error hardening", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    tauriMode = false;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("routes HTTP-only endpoints (tauriCmd === '') over HTTP even inside Tauri Desktop mode", async () => {
    tauriMode = true;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "api_token") return "desktop-token";
      throw new Error(`Unexpected Tauri command: ${cmd}`);
    });

    const fetchMock = vi.fn().mockImplementation(async (url: string) => {
      if (url.endsWith("/api/health")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ success: true }),
        };
      }
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify({ success: true, data: { healthy: true } }),
      };
    });
    vi.stubGlobal("fetch", fetchMock);

    const { call } = await import("./client");
    const result = await call<{ healthy: boolean }>("", undefined, "GET", "/api/metrics/health");

    expect(result).toEqual({ healthy: true });
    expect(mockInvoke).not.toHaveBeenCalledWith("", expect.anything());
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:11420/api/metrics/health",
      expect.objectContaining({
        method: "GET",
        headers: expect.objectContaining({
          Authorization: "Bearer desktop-token",
        }),
      }),
    );
  });

  it("throws structured AppErrorException instead of SyntaxError on empty or plain-text HTTP error body", async () => {
    tauriMode = false;

    const fetchMock = vi.fn().mockImplementation(async (url: string) => {
      if (url.endsWith("/api/health")) {
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify({ success: true }),
        };
      }
      if (url.endsWith("/api/containers")) {
        return {
          ok: false,
          status: 502,
          text: async () => "Bad Gateway: upstream closed connection",
        };
      }
      return {
        ok: false,
        status: 500,
        text: async () => "",
      };
    });
    vi.stubGlobal("fetch", fetchMock);

    const { AppErrorException } = await import("../errors");
    const { call } = await import("./client");

    let firstErr: unknown;
    try {
      await call("unused", undefined, "GET", "/api/containers");
    } catch (e) {
      firstErr = e;
    }
    expect(firstErr).toBeInstanceOf(AppErrorException);
    expect((firstErr as Error).message).toContain(
      "HTTP 502 on /api/containers: Bad Gateway: upstream closed connection",
    );

    await expect(call("unused", undefined, "POST", "/api/system/prune")).rejects.toThrow(
      "HTTP 500 on /api/system/prune",
    );
  });
});

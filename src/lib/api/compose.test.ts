import { describe, it, expect, vi, beforeEach } from "vitest";
import { composeApi } from "./compose";
import * as client from "./client";

vi.mock("./client", () => ({
  call: vi.fn(),
}));

describe("composeApi", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("composeApi.start calls compose_start with projectName", async () => {
    vi.mocked(client.call).mockResolvedValue("Started project");

    const result = await composeApi.start("backend-stack");

    expect(client.call).toHaveBeenCalledWith(
      "compose_start",
      { projectName: "backend-stack" },
      "POST",
      "/api/compose/start",
      undefined,
      { projectName: "backend-stack" }
    );
    expect(result).toBe("Started project");
  });

  it("empirically simulates HTTP client execution to verify URL params and JSON body", async () => {
    const realClient = await vi.importActual<typeof import("./client")>("./client");

    const originalFetch = globalThis.fetch;
    const fetchMock = vi.fn().mockResolvedValue({
      status: 200,
      json: async () => ({ success: true, data: "Started project" }),
    });
    globalThis.fetch = fetchMock;

    try {
      await realClient.call(
        "compose_start",
        { projectName: "my-stack" },
        "POST",
        "/api/compose/start",
        undefined,
        { projectName: "my-stack" }
      );

      const composeCall = fetchMock.mock.calls.find(([u]) =>
        String(u).includes("/api/compose/start")
      );
      expect(composeCall).toBeDefined();
      const [calledUrl, calledOpts] = composeCall!;

      expect(calledUrl).toMatch(/\/api\/compose\/start$/);
      expect(calledUrl).not.toContain("?");
      expect(calledOpts.method).toBe("POST");
      expect(calledOpts.headers["Content-Type"]).toBe("application/json");
      expect(JSON.parse(calledOpts.body)).toEqual({ projectName: "my-stack" });
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});


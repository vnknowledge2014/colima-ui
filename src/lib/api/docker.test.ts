import { describe, it, expect, vi, beforeEach } from "vitest";
import { dockerApi } from "./docker";
import * as client from "./client";

vi.mock("./client", () => ({
  call: vi.fn(),
}));

describe("dockerApi.listContainers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("preserves Labels including com.docker.compose.* metadata", async () => {
    const rawContainers = [
      {
        Id: "c1",
        Names: "/web",
        Image: "nginx:alpine",
        Status: "Up 2 hours",
        State: "running",
        Ports: "0.0.0.0:80->80/tcp",
        CreatedAt: "2026-10-29T10:00:00Z",
        Size: "20MB",
        Command: "nginx -g 'daemon off;'",
        Labels: {
          "com.docker.compose.project": "my-app",
          "com.docker.compose.service": "web",
          "com.docker.compose.version": "2.24.0",
        },
      },
      {
        Id: "c2",
        Names: "/db",
        Image: "postgres:16",
        Status: "Up 2 hours",
        State: "running",
        Ports: "5432/tcp",
        CreatedAt: "2026-10-29T10:01:00Z",
        Size: "150MB",
        Command: "postgres",
        Labels: {
          "com.docker.compose.project": "my-app",
          "com.docker.compose.service": "db",
        },
      },
    ];

    vi.mocked(client.call).mockResolvedValue(rawContainers);

    const result = await dockerApi.listContainers(true);

    expect(client.call).toHaveBeenCalledWith(
      "list_containers",
      { all: true },
      "GET",
      "/api/containers",
      { all: "true" }
    );
    expect(result).toHaveLength(2);
    expect(result[0]?.Labels).toEqual({
      "com.docker.compose.project": "my-app",
      "com.docker.compose.service": "web",
      "com.docker.compose.version": "2.24.0",
    });
    expect(result[1]?.Labels).toEqual({
      "com.docker.compose.project": "my-app",
      "com.docker.compose.service": "db",
    });
  });

  it("handles containers with missing or non-object Labels gracefully", async () => {
    const rawContainers = [
      {
        Id: "c3",
        Names: "/standalone",
        Image: "busybox",
        State: "running",
      },
    ];

    vi.mocked(client.call).mockResolvedValue(rawContainers);

    const result = await dockerApi.listContainers(true);
    expect(result[0]?.Labels).toEqual({});
  });

  it("returns empty array when raw response is null or non-array", async () => {
    vi.mocked(client.call).mockResolvedValue(null);
    const result = await dockerApi.listContainers(true);
    expect(result).toEqual([]);
  });

  it("retains multiline labels, diacritics, and special characters without corruption", async () => {
    const rawContainers = [
      {
        Id: "c-unicode",
        Names: "/app-unicode",
        Image: "node:20",
        Status: "Up 1 hour",
        State: "running",
        Labels: {
          "com.docker.compose.project": "dự-án-colima",
          "com.docker.compose.service": "frontend-giao-diện",
          "description": "Dòng 1: Mô tả hệ thống\nDòng 2: Hỗ trợ tiếng Việt có dấu\r\nDòng 3: 🚀 ñ ü é & special chars <xml> 'quotes'",
          "license": "MIT\nCopyright (c) 2026",
          "empty.val": "",
        },
      },
    ];

    vi.mocked(client.call).mockResolvedValue(rawContainers);
    const result = await dockerApi.listContainers(true);

    expect(result).toHaveLength(1);
    const container = result[0];
    expect(container).toBeDefined();
    const labels = container!.Labels;
    expect(labels).toBeDefined();
    expect(labels!["com.docker.compose.project"]).toBe("dự-án-colima");
    expect(labels!["com.docker.compose.service"]).toBe("frontend-giao-diện");
    expect(labels!["description"]).toBe(
      "Dòng 1: Mô tả hệ thống\nDòng 2: Hỗ trợ tiếng Việt có dấu\r\nDòng 3: 🚀 ñ ü é & special chars <xml> 'quotes'"
    );
    expect(labels!["license"]).toBe("MIT\nCopyright (c) 2026");
    expect(labels!["empty.val"]).toBe("");
  });


  it("handles degenerate container objects without throwing", async () => {
    const degenerateContainers = [
      { Id: "c-null-labels", Labels: null },
      { Id: "c-undefined-labels", Labels: undefined },
      { Id: "c-array-labels", Labels: ["unexpected", "array"] },
      { Id: "c-string-labels", Labels: "not-an-object" },
      { Id: "c-number-labels", Labels: 42 },
      { Id: "c-boolean-labels", Labels: false },
      { Id: "c-missing-labels" },
      {},
    ];

    vi.mocked(client.call).mockResolvedValue(degenerateContainers);
    const result = await dockerApi.listContainers(true);

    expect(result).toHaveLength(8);
    for (const container of result) {
      expect(container.Labels).toEqual({});
      expect(typeof container.Id).toBe("string");
      expect(typeof container.Names).toBe("string");
      expect(typeof container.State).toBe("string");
    }
  });
});


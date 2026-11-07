import { describe, expect, it } from "vitest";
import { parseItems } from "./k8sUtils";

describe("k8sUtils.parseItems", () => {
  it("parses flat Rust K8sPod structs returned by Tauri IPC", () => {
    const tauriPods = [
      {
        name: "coredns-5d78c9869d-abcde",
        namespace: "kube-system",
        status: "Running",
        ready: "1/1",
        restarts: "0",
        age: "2d",
      },
      {
        name: "metrics-server-7f8b9",
        namespace: "kube-system",
        status: "Pending",
        ready: "0/1",
        restarts: "2",
        age: "5m",
      },
    ];

    const items = parseItems(tauriPods);
    expect(items).toHaveLength(2);
    expect(items[0]).toMatchObject({
      name: "coredns-5d78c9869d-abcde",
      namespace: "kube-system",
      status: "Running",
      ready: "1/1",
      restarts: "0",
    });
    expect(items[1].status).toBe("Pending");
  });

  it("parses flat Rust K8sNamespace structs returned by Tauri IPC", () => {
    const tauriNamespaces = [
      { name: "default", status: "Active", age: "10d" },
      { name: "kube-system", status: "Active", age: "10d" },
    ];

    const items = parseItems(tauriNamespaces);
    expect(items).toHaveLength(2);
    expect(items.map((i) => i.name)).toEqual(["default", "kube-system"]);
  });

  it("parses raw kubectl -o json string with items array (Browser HTTP mode)", () => {
    const rawKubectl = JSON.stringify({
      apiVersion: "v1",
      kind: "List",
      items: [
        {
          metadata: {
            name: "web-frontend-1",
            namespace: "prod",
            creationTimestamp: "2026-01-01T00:00:00Z",
          },
          status: {
            phase: "Running",
            containerStatuses: [{ ready: true, restartCount: 3 }],
          },
        },
      ],
    });

    const items = parseItems(rawKubectl);
    expect(items).toHaveLength(1);
    expect(items[0].name).toBe("web-frontend-1");
    expect(items[0].namespace).toBe("prod");
    expect(items[0].status).toBe("Running");
    expect(items[0].ready).toBe("1/1");
    expect(items[0].restarts).toBe("3");
  });

  it("returns empty array for empty or malformed inputs", () => {
    expect(parseItems("")).toEqual([]);
    expect(parseItems("not valid json")).toEqual([]);
    expect(parseItems(null)).toEqual([]);
  });
});

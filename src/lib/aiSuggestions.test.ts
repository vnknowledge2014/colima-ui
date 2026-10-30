import { describe, it, expect } from "vitest";
import {
  getStarterPrompts,
  deriveFallbackSuggestions,
  buildLiveUiContextSnapshot,
  createLiveUiContextSnapshot,
} from "./aiSuggestions";
import type { ColimaInstance, DockerContainer } from "./api/types";

describe("aiSuggestions", () => {
  describe("getStarterPrompts", () => {
    it("returns warning prompts first when Colima VM is stopped or unhealthy", () => {
      const instances: ColimaInstance[] = [
        {
          name: "default",
          status: "Stopped",
          arch: "aarch64",
          cpus: 4,
          memory: 8,
          disk: 60,
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
        {
          name: "k8s-vm",
          status: "Running",
          arch: "aarch64",
          cpus: 2,
          memory: 4,
          disk: 40,
          runtime: "containerd",
          address: "127.0.0.1",
          kubernetes: true,
        },
      ];

      const prompts = getStarterPrompts({
        instances,
        instancesLoaded: true,
        currentPage: "instances",
      });

      expect(prompts.length).toBeGreaterThan(0);
      expect(prompts[0].category).toBe("warning");
      expect(prompts[0].icon).toBe("warning");
      expect(prompts[0].id).toBe("warning-instance-default");
      expect(prompts[0].prompt).toContain("Investigate stopped/unhealthy instance default");
    });

    it("returns warning prompts when container has exited state and strips leading slash", () => {
      const containers: DockerContainer[] = [
        {
          Id: "abc1234567890",
          Names: "/web-api-server",
          Image: "node:18",
          Status: "Exited (1) 5 minutes ago",
          State: "exited",
          Ports: "",
          CreatedAt: "2026-10-30",
          Size: "100MB",
          Command: "npm start",
        },
      ];

      const prompts = getStarterPrompts({
        containers,
        currentPage: "containers",
      });

      const warning = prompts.find((p) => p.category === "warning");
      expect(warning).toBeDefined();
      expect(warning?.title).toBe('Container "web-api-server" exited');
      expect(warning?.prompt).toBe("Analyze why container web-api-server exited");
    });

    it("caps warnings at 2 and fills remaining slots with tab prompts up to 4 total", () => {
      const instances: ColimaInstance[] = [
        { name: "vm1", status: "Stopped", arch: "x86_64", cpus: 2, memory: 4, disk: 30, runtime: "docker", address: "", kubernetes: false },
        { name: "vm2", status: "Broken", arch: "x86_64", cpus: 2, memory: 4, disk: 30, runtime: "docker", address: "", kubernetes: false },
        { name: "vm3", status: "Stopped", arch: "x86_64", cpus: 2, memory: 4, disk: 30, runtime: "docker", address: "", kubernetes: false },
      ];
      const containers: DockerContainer[] = [
        { Id: "c1", Names: "/c1", Image: "img", Status: "Exited", State: "exited", Ports: "", CreatedAt: "", Size: "", Command: "" },
      ];

      const prompts = getStarterPrompts({
        instances,
        instancesLoaded: true,
        containers,
        currentPage: "containers",
      });

      expect(prompts.length).toBeLessThanOrEqual(4);
      const warnings = prompts.filter((p) => p.category === "warning");
      expect(warnings.length).toBe(2);
      const contextPrompts = prompts.filter((p) => p.category === "context");
      expect(contextPrompts.length).toBe(2);
    });

    it("does not surface false warnings on cold start when instancesLoaded is false", () => {
      const prompts = getStarterPrompts({
        instances: [],
        instancesLoaded: false,
        currentPage: "dashboard",
      });

      expect(prompts.every((p) => p.category === "context")).toBe(true);
      expect(prompts.some((p) => p.id === "starter-dashboard-overview")).toBe(true);
    });

    it("returns tab-specific prompts for containers, compose, kubernetes, images, security", () => {
      const containersPrompts = getStarterPrompts({ currentPage: "containers" });
      expect(containersPrompts.some((p) => p.id.includes("containers"))).toBe(true);

      const composePrompts = getStarterPrompts({ currentPage: "compose" });
      expect(composePrompts.some((p) => p.id.includes("compose"))).toBe(true);

      const k8sPrompts = getStarterPrompts({ currentPage: "kubernetes" });
      expect(k8sPrompts.some((p) => p.id.includes("k8s"))).toBe(true);

      const secPrompts = getStarterPrompts({ currentPage: "security" });
      expect(secPrompts.some((p) => p.id.includes("security"))).toBe(true);
    });

    it("falls back to dashboard prompts when page is unknown or empty", () => {
      const prompts = getStarterPrompts({});
      expect(prompts.length).toBeGreaterThanOrEqual(3);
      expect(prompts[0].id).toBe("starter-dashboard-overview");
    });

    it("handles Docker Engine API string array Names without throwing", () => {
      const containerWithArrayNames = {
        Id: "1234567890abcdef",
        Names: ["/web-server", "/web-server-alias"],
        Image: "nginx:latest",
        Status: "Exited (1)",
        State: "exited",
        Ports: "",
        CreatedAt: "",
        Size: "",
        Command: "",
      } as unknown as DockerContainer;

      const prompts = getStarterPrompts({ containers: [containerWithArrayNames] });
      const warning = prompts.find((p) => p.category === "warning");
      expect(warning).toBeDefined();
      expect(warning?.title).toBe('Container "web-server" exited');
      expect(warning?.prompt).toBe("Analyze why container web-server exited");
    });

    it("defensively accepts colimaInstances and dockerContainers alias keys", () => {
      const prompts = getStarterPrompts({
        colimaInstances: [
          { name: "test-vm", status: "Stopped" } as ColimaInstance,
        ],
        dockerContainers: [
          { Id: "c-exited", Names: "/api-service", State: "exited" } as DockerContainer,
        ],
      });

      expect(prompts.some((p) => p.id === "warning-instance-test-vm")).toBe(true);
      expect(prompts.some((p) => p.id === "warning-container-c-exited")).toBe(true);
    });

    it("safely falls back to container ID slice when Names is missing or empty array", () => {
      const emptyNamesContainer = {
        Id: "9876543210fedcba",
        Names: [],
        State: "exited",
      } as unknown as DockerContainer;

      const prompts = getStarterPrompts({ containers: [emptyNamesContainer] });
      const warning = prompts.find((p) => p.category === "warning");
      expect(warning).toBeDefined();
      expect(warning?.title).toContain("9876543210fe");
    });
  });

  describe("deriveFallbackSuggestions", () => {
    it("returns empty array if an interactive choice card is active", () => {
      const suggestions = deriveFallbackSuggestions({
        hasChoiceCard: true,
        currentPage: "containers",
      });
      expect(suggestions).toEqual([]);
    });

    it("derives container inspection chips when list-containers was executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: ["list-containers"],
        currentPage: "dashboard",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("Check CPU & memory");
      expect(suggestions[1][0]).toBe("View container logs");
    });

    it("derives troubleshooting chips when container-logs was executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: ["container-logs"],
        currentPage: "containers",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("Analyze errors");
    });

    it("derives compose diagnosis chips when DIAGNOSE or compose tool was executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: ["DIAGNOSE"],
        currentPage: "compose",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("Propose fixes");
    });

    it("derives VM resource chips when instance-status was executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: ["instance-status"],
        currentPage: "instances",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("Inspect VM resources");
    });

    it("derives Kubernetes chips when k8s-get-pods was executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: ["k8s-get-pods"],
        currentPage: "kubernetes",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("Check pod health");
    });

    it("derives security triage chips when security-scan was executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: ["security-scan"],
        currentPage: "security",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("Review CVEs");
    });

    it("falls back to current page suggestions when no tools were executed", () => {
      const suggestions = deriveFallbackSuggestions({
        toolsExecuted: [],
        currentPage: "containers",
      });
      expect(suggestions.length).toBe(3);
      expect(suggestions[0][0]).toBe("List running containers");
    });

    it("guarantees each suggestion is a [label, prompt] tuple with at most 3 items", () => {
      const suggestions = deriveFallbackSuggestions({ currentPage: "volumes" });
      expect(suggestions.length).toBeLessThanOrEqual(3);
      for (const item of suggestions) {
        expect(Array.isArray(item)).toBe(true);
        expect(item.length).toBe(2);
        expect(typeof item[0]).toBe("string");
        expect(typeof item[1]).toBe("string");
      }
    });

    it("handles object tool descriptors in toolsExecuted without throwing", () => {
      const suggestions1 = deriveFallbackSuggestions({
        toolsExecuted: [{ kind: "query", name: "list-containers" }],
        currentPage: "dashboard",
      });
      expect(suggestions1.length).toBe(3);
      expect(suggestions1[0][0]).toBe("Check CPU & memory");

      const suggestions2 = deriveFallbackSuggestions({
        toolsExecuted: [{ kind: "diagnose", name: "DIAGNOSE" }],
        currentPage: "compose",
      });
      expect(suggestions2.length).toBe(3);
      expect(suggestions2[0][0]).toBe("Propose fixes");

      const suggestions3 = deriveFallbackSuggestions({
        toolsExecuted: [{ kind: "diagnose" }],
        currentPage: "compose",
      });
      expect(suggestions3.length).toBe(3);
      expect(suggestions3[0][0]).toBe("Propose fixes");

      const suggestions4 = deriveFallbackSuggestions({
        toolsExecuted: [null as unknown as string, undefined as unknown as string, {}, { name: 123 as unknown as string }],
        currentPage: "containers",
      });
      expect(suggestions4.length).toBe(3);
      expect(suggestions4[0][0]).toBe("List running containers");
    });
  });

  describe("buildLiveUiContextSnapshot & createLiveUiContextSnapshot", () => {
    it("creates structured snapshot and counts running vs stopped containers", () => {
      const snapshot = createLiveUiContextSnapshot({
        currentPage: "containers",
        language: "vi",
        instances: [
          { name: "default", status: "Running", arch: "aarch64", cpus: 4, memory: 8, disk: 60, runtime: "docker", address: "", kubernetes: false },
          { name: "test", status: "Stopped", arch: "aarch64", cpus: 2, memory: 4, disk: 30, runtime: "docker", address: "", kubernetes: false },
        ],
        containers: [
          { Id: "c1", Names: "app", Image: "img", Status: "Up 2 hours", State: "running", Ports: "", CreatedAt: "", Size: "", Command: "" },
          { Id: "c2", Names: "db", Image: "img", Status: "Exited (0)", State: "exited", Ports: "", CreatedAt: "", Size: "", Command: "" },
        ],
        k8sStatus: { connected: true, pods: 5, namespaces: 2, kindClusters: 1 },
      });

      expect(snapshot.currentPage).toBe("containers");
      expect(snapshot.language).toBe("vi");
      expect(snapshot.activeInstances).toEqual(["default"]);
      expect(snapshot.runningContainers).toBe(1);
      expect(snapshot.stoppedContainers).toBe(1);
      expect(snapshot.k8sConnected).toBe(true);
      expect(snapshot.activeContext).toBe("default");
    });

    it("formats concise text snapshot under 250 tokens", () => {
      const text = buildLiveUiContextSnapshot({
        currentPage: "kubernetes",
        language: "en",
        instances: [
          { name: "colima", status: "Running", arch: "aarch64", cpus: 4, memory: 8, disk: 50, runtime: "containerd", address: "", kubernetes: true },
        ],
        containers: [
          { Id: "c1", Names: "k8s-pod", Image: "img", Status: "Up 1h", State: "running", Ports: "", CreatedAt: "", Size: "", Command: "" },
        ],
        k8sStatus: { connected: true, pods: 4, namespaces: 2, kindClusters: 0 },
      });

      expect(text).toContain("[LIVE_UI_CONTEXT]");
      expect(text).toContain("Current Page: kubernetes");
      expect(text).toContain("Colima VMs: colima: Running (4 CPU, 8GiB)");
      expect(text).toContain("Containers: 1 running, 0 stopped");
      expect(text).toContain("Kubernetes: connected (4 pods, 2 namespaces)");
      // Token/character length check: well under 250 tokens (< 1000 chars)
      expect(text.length).toBeLessThan(1000);
    });

    it("handles undefined options gracefully without throwing", () => {
      const text = buildLiveUiContextSnapshot({});
      expect(text).toContain("Current Page: dashboard");
      expect(text).toContain("Colima VMs: none");
      expect(text).toContain("Containers: 0 running, 0 stopped");
      expect(text).toContain("Kubernetes: disconnected");
    });
  });
});

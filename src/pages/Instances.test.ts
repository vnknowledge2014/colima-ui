import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Instances from "./Instances.svelte";
import { dashboardState } from "../store.svelte";
import { colimaApi } from "../lib/api";
import {
  normalizeResourceToGiB,
  normalizeMemoryToGiB,
  normalizeDiskToGiB,
  ONE_GIB_BYTES,
} from "../lib/normalizers";

// Mock API clients and stores
vi.mock("../lib/api/client", () => ({
  call: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  composeApi: {
    list: vi.fn(),
    ps: vi.fn(),
    logs: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    restart: vi.fn(),
    down: vi.fn(),
  },
  colimaApi: {
    listInstances: vi.fn().mockResolvedValue([]),
    startInstance: vi.fn().mockResolvedValue(undefined),
    stopInstance: vi.fn().mockResolvedValue(undefined),
    createWorkerNode: vi.fn().mockResolvedValue(undefined),
  },
  kindApi: {
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    delete: vi.fn(),
  },
  sysMethods: {
    checkTool: vi.fn().mockResolvedValue({ installed: false }),
    hostSpecs: vi.fn().mockResolvedValue({
      cpu_cores: 8,
      memory_gib: 16,
      disk_free_gib: 100,
    }),
  },
  aiApi: {
    chat: vi.fn(),
  },
}));

vi.mock("../lib/dataPoller", () => ({
  refreshManual: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../lib/presetStateManager", () => ({
  handleInstanceStarted: vi.fn().mockResolvedValue(undefined),
  handleInstanceStopping: vi.fn().mockResolvedValue(undefined),
  getCurrentPresetForInstance: vi.fn().mockReturnValue(null),
  BUILT_IN_PRESETS: [],
}));

vi.mock("../lib/settingsStore.svelte", () => ({
  getAppSetting: vi.fn().mockReturnValue(""),
  setAppSetting: vi.fn(),
}));

vi.mock("../lib/topology-link", () => ({
  viewInTopology: vi.fn(),
  consumeFocus: vi.fn().mockReturnValue(null),
}));

vi.mock("../lib/globalToast", () => ({
  globalToast: vi.fn(),
}));

describe("Instances byte-to-GiB normalization & component integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Unit: Exported Resource Normalization Functions", () => {
    it("normalizes byte inputs (>= 1 GiB) to GiB integers", () => {
      // Memory byte conversions
      expect(normalizeMemoryToGiB(8_589_934_592)).toBe(8);
      expect(normalizeMemoryToGiB(17_179_869_184)).toBe(16);
      expect(normalizeMemoryToGiB(ONE_GIB_BYTES)).toBe(1);

      // Disk byte conversions
      expect(normalizeDiskToGiB(64_424_509_440)).toBe(60);
      expect(normalizeDiskToGiB(107_374_182_400)).toBe(100);
      expect(normalizeDiskToGiB(ONE_GIB_BYTES)).toBe(1);
    });

    it("preserves inputs already specified in GiB integers (< 1 GiB)", () => {
      // Memory already in GiB
      expect(normalizeMemoryToGiB(1)).toBe(1);
      expect(normalizeMemoryToGiB(2)).toBe(2);
      expect(normalizeMemoryToGiB(4)).toBe(4);
      expect(normalizeMemoryToGiB(8)).toBe(8);
      expect(normalizeMemoryToGiB(16)).toBe(16);
      expect(normalizeMemoryToGiB(32)).toBe(32);

      // Disk already in GiB
      expect(normalizeDiskToGiB(20)).toBe(20);
      expect(normalizeDiskToGiB(60)).toBe(60);
      expect(normalizeDiskToGiB(100)).toBe(100);
    });

    it("applies appropriate fallbacks on undefined, null, or non-positive values", () => {
      // Memory fallback: 2 GiB
      expect(normalizeMemoryToGiB(undefined)).toBe(2);
      expect(normalizeMemoryToGiB(null)).toBe(2);
      expect(normalizeMemoryToGiB(0)).toBe(2);
      expect(normalizeMemoryToGiB(-1)).toBe(2);

      // Disk fallback: 60 GiB
      expect(normalizeDiskToGiB(undefined)).toBe(60);
      expect(normalizeDiskToGiB(null)).toBe(60);
      expect(normalizeDiskToGiB(0)).toBe(60);
      expect(normalizeDiskToGiB(-10)).toBe(60);

      // Generic normalizer with custom fallback
      expect(normalizeResourceToGiB(0, 4)).toBe(4);
      expect(normalizeResourceToGiB(null, 10)).toBe(10);
    });
  });

  describe("Integration: Instances.svelte Component Verification", () => {
    it("renders and locks buttons across all 4 instance status tiers", () => {
      // 1. Starting status: Start button is disabled with 'Starting...', Stop is enabled
      dashboardState.colimaInstances = [
        {
          name: "test-starting",
          status: "Starting",
          arch: "aarch64",
          cpus: 2,
          memory: 2,
          disk: 60,
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
      ];

      const renderStarting = render(Instances, {
        props: { onRefresh: vi.fn() },
      });
      const startingBtn = screen.getByRole("button", { name: /starting\.\.\./i });
      expect(startingBtn.hasAttribute("disabled")).toBe(true);
      const stopInStarting = screen.getByRole("button", { name: "Stop" });
      expect(stopInStarting.hasAttribute("disabled")).toBe(false);
      renderStarting.unmount();

      // 2. Stopped status: Start button is enabled
      dashboardState.colimaInstances = [
        {
          name: "test-stopped",
          status: "Stopped",
          arch: "aarch64",
          cpus: 2,
          memory: 2,
          disk: 60,
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
      ];
      const renderStopped = render(Instances, {
        props: { onRefresh: vi.fn() },
      });
      const startInStopped = screen.getByRole("button", { name: "Start" });
      expect(startInStopped.hasAttribute("disabled")).toBe(false);
      renderStopped.unmount();

      // 3. Running status: Stop and Restart buttons are enabled, Start button is absent
      dashboardState.colimaInstances = [
        {
          name: "test-running",
          status: "Running",
          arch: "aarch64",
          cpus: 2,
          memory: 2,
          disk: 60,
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
      ];
      const renderRunning = render(Instances, {
        props: { onRefresh: vi.fn() },
      });
      expect(screen.queryByRole("button", { name: "Start" })).toBeNull();
      expect(screen.getByRole("button", { name: "Stop" })).toBeDefined();
      expect(screen.getByRole("button", { name: "Restart" })).toBeDefined();
      renderRunning.unmount();

      // 4. Unhealthy status: Stop and Restart buttons are enabled, Start button is absent
      dashboardState.colimaInstances = [
        {
          name: "test-unhealthy",
          status: "Unhealthy",
          arch: "aarch64",
          cpus: 2,
          memory: 2,
          disk: 60,
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
      ];
      const renderUnhealthy = render(Instances, {
        props: { onRefresh: vi.fn() },
      });
      expect(screen.queryByRole("button", { name: "Start" })).toBeNull();
      expect(screen.getByRole("button", { name: "Stop" })).toBeDefined();
      expect(screen.getByRole("button", { name: "Restart" })).toBeDefined();
      renderUnhealthy.unmount();
    });

    it("normalizes byte inputs (>= 1 GiB) when starting instance via UI", async () => {
      dashboardState.colimaInstances = [
        {
          name: "byte-vm",
          status: "Stopped",
          arch: "aarch64",
          cpus: 4,
          memory: 8_589_934_592, // 8 GiB in bytes
          disk: 64_424_509_440,   // 60 GiB in bytes
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
      ];

      const { unmount } = render(Instances, {
        props: { onRefresh: vi.fn() },
      });

      const startBtn = screen.getByRole("button", { name: "Start" });
      await fireEvent.click(startBtn);

      expect(colimaApi.startInstance).toHaveBeenCalledWith(
        expect.objectContaining({
          profile: "byte-vm",
          cpus: 4,
          memory: 8,
          disk: 60,
        })
      );

      unmount();
    });

    it("preserves GiB integers (< 1 GiB) without downscaling to 1 GiB when starting instance via UI", async () => {
      dashboardState.colimaInstances = [
        {
          name: "preset-vm",
          status: "Stopped",
          arch: "aarch64",
          cpus: 4,
          memory: 4,  // already 4 GiB
          disk: 100,  // already 100 GiB
          runtime: "docker",
          address: "127.0.0.1",
          kubernetes: false,
        },
      ];

      const { unmount } = render(Instances, {
        props: { onRefresh: vi.fn() },
      });

      const startBtn = screen.getByRole("button", { name: "Start" });
      await fireEvent.click(startBtn);

      expect(colimaApi.startInstance).toHaveBeenCalledWith(
        expect.objectContaining({
          profile: "preset-vm",
          cpus: 4,
          memory: 4,
          disk: 100,
        })
      );

      unmount();
    });

    it("tests starting instances from Instances.svelte with nullish allocations falling back safely", async () => {
      dashboardState.colimaInstances = [
        {
          name: "null-spec-vm",
          status: "Stopped",
          arch: "",
          cpus: 0 as unknown as number,
          memory: undefined as unknown as number,
          disk: null as unknown as number,
          runtime: "",
          address: "",
          kubernetes: false,
        },
      ];

      const { unmount } = render(Instances, {
        props: { onRefresh: vi.fn() },
      });

      const startBtn = screen.getByRole("button", { name: "Start" });
      await fireEvent.click(startBtn);

      expect(colimaApi.startInstance).toHaveBeenCalledWith(
        expect.objectContaining({
          profile: "null-spec-vm",
          cpus: 2,
          memory: 2,
          disk: 60,
        })
      );

      unmount();
    });
  });
});

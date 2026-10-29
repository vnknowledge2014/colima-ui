import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import Containers from "./Containers.svelte";
import { dockerState, setEventCooldown, isEventCooldownActive, clearEventCooldown } from "../store.svelte";
import { dockerApi } from "../lib/api";
import type { DockerContainer } from "../lib/api";

vi.mock("../lib/api", () => ({
  dockerApi: {
    listContainers: vi.fn(),
    startContainer: vi.fn().mockResolvedValue(undefined),
    stopContainer: vi.fn().mockResolvedValue(undefined),
    restartContainer: vi.fn().mockResolvedValue(undefined),
    removeContainer: vi.fn().mockResolvedValue(undefined),
    containerLogs: vi.fn().mockResolvedValue(""),
    containerStats: vi.fn().mockResolvedValue(""),
    containerTop: vi.fn().mockResolvedValue(""),
    inspectContainer: vi.fn().mockResolvedValue("{}"),
    containerExec: vi.fn().mockResolvedValue(""),
  },
  sysMethods: {
    getRuntimeInfo: vi.fn().mockResolvedValue("docker"),
  },
}));

vi.mock("../lib/dataPoller", () => ({
  refreshManual: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../lib/presetStateManager", () => ({
  getCurrentPresetForInstance: vi.fn().mockReturnValue(null),
  getContainerPresetMap: vi.fn().mockResolvedValue(new Map()),
  PRESET_LABELS: {},
  PRESET_COLORS: {},
}));

vi.mock("../lib/settingsStore.svelte", () => ({
  getAppSetting: vi.fn().mockReturnValue(""),
  setAppSetting: vi.fn(),
}));

vi.mock("../lib/topology-link", () => ({
  viewInTopology: vi.fn(),
  consumeFocus: vi.fn().mockReturnValue(null),
}));

describe("Containers.svelte component & reactive selectedContainer derivation", () => {
  const c1: DockerContainer = {
    Id: "c-1234567890ab",
    Names: "test-redis",
    Image: "redis:alpine",
    Status: "Up 15 minutes",
    State: "running",
    Ports: "6379/tcp",
    CreatedAt: "2026-10-29T10:00:00Z",
    Size: "32MB",
    Command: "redis-server",
    Labels: {
      "com.docker.compose.project": "cache-stack",
      "com.docker.compose.service": "redis",
    },
  };

  beforeEach(() => {
    vi.clearAllMocks();
    dockerState.containers = [c1];
    clearEventCooldown();
    vi.mocked(dockerApi.listContainers).mockResolvedValue([c1]);
  });

  it("opens modal on row click, updates status reactively via selectedContainer derivation, and closes on removal", async () => {
    const { unmount } = render(Containers);

    const rowTitle = screen.getByText("test-redis");
    expect(rowTitle).toBeDefined();

    await fireEvent.click(rowTitle);

    expect(screen.getByText("Up 15 minutes")).toBeDefined();
    expect(screen.getByRole("heading", { level: 2, name: /test-redis/i })).toBeDefined();

    // Reactively update container state
    const c1Updated: DockerContainer = {
      ...c1,
      Status: "Exited (0) 5 seconds ago",
      State: "exited",
    };
    dockerState.containers = [c1Updated];

    await waitFor(() => {
      expect(screen.getByText("Exited (0) 5 seconds ago")).toBeDefined();
    });

    // Remove container from store
    dockerState.containers = [];

    await waitFor(() => {
      expect(screen.queryByRole("heading", { level: 2, name: /test-redis/i })).toBeNull();
    });

    unmount();
  });

  it("resets event cooldown to 0 when single container start action fails", async () => {
    const stoppedContainer: DockerContainer = {
      ...c1,
      Id: "c-stopped-1",
      Names: "stopped-app",
      Status: "Exited (1) 2 minutes ago",
      State: "exited",
    };

    dockerState.containers = [stoppedContainer];
    vi.mocked(dockerApi.listContainers).mockResolvedValue([stoppedContainer]);
    vi.mocked(dockerApi.startContainer).mockRejectedValueOnce(
      new Error("Docker daemon connection lost")
    );

    const { unmount } = render(Containers);

    const rowTitle = screen.getByText("stopped-app");
    await fireEvent.click(rowTitle);

    const modalStartBtn = screen.getAllByRole("button", { name: "Start" }).find(
      (b) => b.classList.contains("btn-primary")
    );
    expect(modalStartBtn).toBeDefined();

    expect(isEventCooldownActive()).toBe(false);

    await fireEvent.click(modalStartBtn!);

    await waitFor(() => {
      expect(dockerApi.startContainer).toHaveBeenCalledWith("c-stopped-1");
      expect(isEventCooldownActive()).toBe(false);
      expect(dockerState.eventCooldownUntil).toBe(0);
    });

    unmount();
  });

  it("resets event cooldown to 0 when batch action fails", async () => {
    const cA: DockerContainer = {
      Id: "cont-a",
      Names: "app-a",
      Image: "nginx:latest",
      Status: "Exited (0)",
      State: "exited",
      Ports: "",
      CreatedAt: "",
      Size: "",
      Command: "",
      Labels: {},
    };
    const cB: DockerContainer = {
      Id: "cont-b",
      Names: "app-b",
      Image: "redis:latest",
      Status: "Exited (0)",
      State: "exited",
      Ports: "",
      CreatedAt: "",
      Size: "",
      Command: "",
      Labels: {},
    };

    dockerState.containers = [cA, cB];
    vi.mocked(dockerApi.listContainers).mockResolvedValue([cA, cB]);
    vi.mocked(dockerApi.startContainer).mockImplementation(async (id: string): Promise<string> => {
      if (id === "cont-b") throw new Error("Port conflict");
      return "Started";
    });

    const { unmount } = render(Containers);

    await waitFor(() => {
      expect(screen.getByText("app-a")).toBeDefined();
      expect(screen.getByText("app-b")).toBeDefined();
    });

    const selectAllCheckbox = screen.getAllByRole("checkbox")[0];
    await fireEvent.click(selectAllCheckbox);

    const batchStartBtns = screen.getAllByRole("button", { name: /Start Selected/i });
    expect(batchStartBtns.length).toBeGreaterThan(0);
    await fireEvent.click(batchStartBtns[0]);

    const confirmBtn = screen.getByRole("button", { name: "Start All" });
    await fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(dockerApi.startContainer).toHaveBeenCalledWith("cont-a");
      expect(dockerApi.startContainer).toHaveBeenCalledWith("cont-b");
      expect(isEventCooldownActive()).toBe(false);
      expect(dockerState.eventCooldownUntil).toBe(0);
    });

    unmount();
  });

  it("resets event cooldown to 0 when Compose group action fails", async () => {
    const cCompose: DockerContainer = {
      Id: "c-compose-1",
      Names: "web-server",
      Image: "nginx",
      Status: "Exited (0)",
      State: "exited",
      Ports: "",
      CreatedAt: "",
      Size: "",
      Command: "",
      Labels: {
        "com.docker.compose.project": "my-web-project",
        "com.docker.compose.service": "web",
      },
    };

    dockerState.containers = [cCompose];
    vi.mocked(dockerApi.listContainers).mockResolvedValue([cCompose]);
    vi.mocked(dockerApi.startContainer).mockRejectedValueOnce(
      new Error("Mount volume failed")
    );

    const { unmount } = render(Containers);

    await waitFor(() => {
      expect(screen.getByText("my-web-project")).toBeDefined();
    });

    const startButtons = screen.getAllByRole("button", { name: "Start" });
    const groupStartBtn = startButtons.find((b) => b.classList.contains("btn-sm"));
    expect(groupStartBtn).toBeDefined();

    await fireEvent.click(groupStartBtn!);

    await waitFor(() => {
      expect(dockerApi.startContainer).toHaveBeenCalledWith("c-compose-1");
      expect(isEventCooldownActive()).toBe(false);
      expect(dockerState.eventCooldownUntil).toBe(0);
    });

    unmount();
  });

  it("resets event cooldown to 0 when removeContainer action fails in confirmation modal", async () => {
    const targetContainer: DockerContainer = {
      ...c1,
      Id: "c-to-delete",
      Names: "delete-me",
      State: "running",
    };

    dockerState.containers = [targetContainer];
    vi.mocked(dockerApi.listContainers).mockResolvedValue([targetContainer]);
    vi.mocked(dockerApi.removeContainer).mockRejectedValueOnce(
      new Error("Container removal failed: resource busy")
    );

    const { unmount } = render(Containers);

    setEventCooldown(8000);
    expect(isEventCooldownActive()).toBe(true);

    const resultPromise = new Promise((resolve) => {
      window.dispatchEvent(
        new CustomEvent("delete-container", {
          detail: { payload: "c-to-delete", resolve },
        })
      );
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Remove" })).toBeDefined();
    });
    const confirmBtn = screen.getByRole("button", { name: "Remove" });
    await fireEvent.click(confirmBtn);

    await resultPromise;

    await waitFor(() => {
      expect(isEventCooldownActive()).toBe(false);
      expect(dockerState.eventCooldownUntil).toBe(0);
    });

    unmount();
  });
});

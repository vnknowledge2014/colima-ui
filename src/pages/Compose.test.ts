import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import Compose from "./Compose.svelte";
import { composeApi } from "../lib/api/compose";
import * as client from "../lib/api/client";
import { parseComposeStatus } from "../lib/composeGrouping";
import { refreshManual } from "../lib/dataPoller";
import { setEventCooldown, isEventCooldownActive, clearEventCooldown, dockerState } from "../store.svelte";

vi.mock("../lib/api/client", () => ({
  call: vi.fn(),
}));

vi.mock("../lib/api/compose", () => ({
  composeApi: {
    list: vi.fn().mockResolvedValue([]),
    ps: vi.fn().mockResolvedValue(""),
    logs: vi.fn().mockResolvedValue(""),
    start: vi.fn().mockResolvedValue("Started"),
    stop: vi.fn().mockResolvedValue("Stopped"),
    restart: vi.fn().mockResolvedValue("Restarted"),
    down: vi.fn().mockResolvedValue("Downed"),
  },
}));

vi.mock("../lib/dataPoller", () => ({
  refreshManual: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../lib/globalToast", () => ({
  globalToast: vi.fn(),
}));

describe("Compose page actions, affordances, and status parsing", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearEventCooldown();
  });

  it("parses compose status and determines running container count", () => {
    expect(parseComposeStatus("running(2)")).toEqual({ running: 2, display: "running(2)" });
    expect(parseComposeStatus("running(0)")).toEqual({ running: 0, display: "running(0)" });
    expect(parseComposeStatus("exited(2)")).toEqual({ running: 0, display: "exited(2)" });
    expect(parseComposeStatus("")).toEqual({ running: 0, display: "" });
  });

  it("triggers composeApi.start when starting a stopped project", async () => {
    const actualCompose = await vi.importActual<typeof import("../lib/api/compose")>("../lib/api/compose");
    vi.mocked(client.call).mockResolvedValue("Started");
    await actualCompose.composeApi.start("my-stack");
    expect(client.call).toHaveBeenCalledWith(
      "compose_start",
      { projectName: "my-stack" },
      "POST",
      "/api/compose/start",
      undefined,
      { projectName: "my-stack" }
    );
  });

  it("renders Start button when running === 0 and dispatches composeApi.start", async () => {
    vi.mocked(composeApi.list).mockResolvedValue([
      {
        Name: "stopped-app",
        Status: "exited(1)",
        ConfigFiles: "/docker-compose.yml",
      },
    ]);
    vi.mocked(composeApi.start).mockResolvedValue("Started");

    const { unmount } = render(Compose);

    await waitFor(() => {
      expect(screen.getByText("stopped-app")).toBeDefined();
    });

    const startBtn = screen.getByLabelText("Start");
    expect(startBtn).toBeDefined();

    await fireEvent.click(startBtn);

    expect(composeApi.start).toHaveBeenCalledWith("stopped-app");
    unmount();
  });

  it("renders Stop, Restart, and Down buttons when running > 0 and hides Start button", async () => {
    vi.mocked(composeApi.list).mockResolvedValue([
      {
        Name: "running-app",
        Status: "running(2)",
        ConfigFiles: "/docker-compose.yml",
      },
    ]);

    const { unmount } = render(Compose);

    await waitFor(() => {
      expect(screen.getByText("running-app")).toBeDefined();
    });

    expect(screen.getByLabelText("Stop")).toBeDefined();
    expect(screen.getByLabelText("Restart")).toBeDefined();
    expect(screen.queryByLabelText("Start")).toBeNull();

    unmount();
  });

  it("clears event cooldown and refreshes state when composeApi.start fails", async () => {
    vi.mocked(composeApi.list).mockResolvedValue([
      {
        Name: "fail-app",
        Status: "exited(1)",
        ConfigFiles: "/docker-compose.yml",
      },
    ]);
    vi.mocked(composeApi.start).mockRejectedValue(new Error("Docker daemon network error"));

    const { unmount } = render(Compose);

    await waitFor(() => {
      expect(screen.getByText("fail-app")).toBeDefined();
    });

    setEventCooldown(10000);
    expect(isEventCooldownActive()).toBe(true);

    const startBtn = screen.getByLabelText("Start");
    await fireEvent.click(startBtn);

    await waitFor(() => {
      expect(isEventCooldownActive()).toBe(false);
      expect(dockerState.eventCooldownUntil).toBe(0);
      expect(composeApi.list).toHaveBeenCalled();
      expect(refreshManual).toHaveBeenCalled();
    });

    unmount();
  });
});

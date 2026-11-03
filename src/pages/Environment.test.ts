import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Environment from "./Environment.svelte";
import { environmentApi, type EnvironmentReport } from "../lib/api";

vi.mock("../lib/api", () => ({
  environmentApi: {
    getResolutions: vi.fn(),
  },
}));

const sampleReport: EnvironmentReport = {
  envDuality: [
    {
      key: "PATH",
      runtime: "/usr/bin:/bin",
      shell: "/opt/homebrew/bin:/usr/bin:/bin",
      diverges: true,
    },
    {
      key: "COLIMA_HOME",
      runtime: null,
      shell: null,
      diverges: false,
    },
  ],
  rows: [
    {
      key: "colima-home",
      rule: "auto-resolve",
      chosen: "/Users/test/.config/colima",
      detail: "resolution mirrors colima 0.10.x",
      health: "exists",
    },
    {
      key: "docker-socket",
      rule: "discovered",
      chosen: "unix:///Users/test/.config/colima/default/docker.sock",
      detail: "active context: default",
      health: "live",
    },
  ],
  shellSnapshotError: null,
};

describe("Environment page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders environment duality and resolution rows with accessible regions", async () => {
    vi.mocked(environmentApi.getResolutions).mockResolvedValue(sampleReport);
    render(Environment);

    expect(await screen.findByText("/opt/homebrew/bin:/usr/bin:/bin")).toBeTruthy();
    expect(screen.getByText("/usr/bin:/bin")).toBeTruthy();
    expect(screen.getByText("/Users/test/.config/colima")).toBeTruthy();
    expect(screen.getByText("unix:///Users/test/.config/colima/default/docker.sock")).toBeTruthy();
    expect(screen.getAllByRole("region").length).toBe(2);
  });

  it("re-scans when clicking Re-scan and surfaces API errors cleanly", async () => {
    vi.mocked(environmentApi.getResolutions)
      .mockResolvedValueOnce(sampleReport)
      .mockRejectedValueOnce(new Error("probe failed"));

    render(Environment);
    await screen.findByText("colima-home");

    const rescanBtn = screen.getByRole("button", { name: /Re-scan|Quét lại|再スキャン|重新扫描/i });
    await fireEvent.click(rescanBtn);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("probe failed");
  });
});

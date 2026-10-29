import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
// Raw source, not the compiled component: the assertion is about how many times
// the tag appears in the markup, which the compiled output no longer shows.
import appSource from "./App.svelte?raw";
import sidebarSource from "./components/Sidebar.svelte?raw";
import settingsSource from "./pages/Settings.svelte?raw";

vi.mock("./lib/api/client", () => ({
  API_BASE: "http://127.0.0.1:11420",
  isTauri: () => false,
  resolveApiBase: () => Promise.resolve("http://127.0.0.1:11420"),
  call: (cmd: string) => {
    switch (cmd) {
      case "get_colima_config":
        return Promise.resolve({
          profile: "default",
          values: { cpu: 4, memory: 8, disk: 60, runtime: "docker", vmType: "vz", mountType: "virtiofs" },
          mtime: 1,
        });
      case "get_host_specs":
        return Promise.resolve({ cpus: 8, memory_gb: 16 });
      case "telemetry_consent":
        return Promise.resolve("declined");
      case "telemetry_preview":
      case "self_heal_list_rules":
      case "self_heal_log":
      case "get_all_memories":
      case "ai_list_models":
        return Promise.resolve([]);
      case "self_heal_enabled":
        return Promise.resolve(false);
      case "get_all_settings":
        return Promise.resolve({});
      default:
        return Promise.resolve(null);
    }
  },
}));

import Sidebar from "./components/Sidebar.svelte";
import Settings from "./pages/Settings.svelte";
import { uiState } from "./store.svelte";

afterEach(() => {
  uiState.sidebarCollapsed = false;
  cleanup();
});

// Each ToastContainer keeps its own `toasts` state and subscribes to the global
// toast bus independently, so a second instance renders every toast twice. That
// is a structural constraint, not runtime behaviour — checking the source is
// enough and avoids mounting the whole component tree.
it("renders exactly one ToastContainer", () => {
  expect(appSource.match(/<ToastContainer \/>/g)).toHaveLength(1);
});

it("does not mount or import any Sign-in or Account surfaces", () => {
  expect(appSource).not.toMatch(/SignInPanel|account\.svelte/);
  expect(sidebarSource).not.toMatch(/UserBadge|account/);
  expect(settingsSource).not.toMatch(/<Account\b|settings\/Account/);
});

describe("Sidebar and Settings runtime rendering without Sign-in / Account", () => {
  const sampleSystemInfo = {
    colima_installed: true,
    colima_version: "colima version 0.8.1",
    docker_installed: true,
    docker_version: "Docker version 27.5.1",
    lima_installed: true,
    lima_version: "limactl version 1.0.4",
    kubectl_installed: false,
    kubectl_version: "",
  };

  it("renders Sidebar footer in expanded and collapsed rail modes with no Sign-in badge", async () => {
    const { container } = render(Sidebar, {
      props: { systemInfo: sampleSystemInfo, onStartTour: vi.fn() },
    });

    const aside = container.querySelector("aside.sidebar");
    const footer = container.querySelector(".sidebar-footer");
    expect(aside).not.toBeNull();
    expect(aside?.classList.contains("collapsed")).toBe(false);
    expect(footer).not.toBeNull();
    expect(footer?.querySelector(".account-badge")).toBeNull();
    expect(footer?.querySelector(".footer-status")).not.toBeNull();
    expect(footer?.querySelector(".footer-controls")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /sign\s*in|sign\s*out|account/i })).toBeNull();

    const collapseBtn = screen.getByRole("button", { name: /collapse sidebar/i });
    await fireEvent.click(collapseBtn);

    expect(aside?.classList.contains("collapsed")).toBe(true);
    expect(screen.getByRole("button", { name: /expand sidebar/i })).toBeTruthy();
    expect(footer?.querySelector(".account-badge")).toBeNull();
    expect(screen.queryByRole("button", { name: /sign\s*in|sign\s*out|account/i })).toBeNull();
  });

  it("renders Settings sections cleanly without an Account section or Sign-in actions", async () => {
    const { container } = render(Settings, {
      props: { systemInfo: sampleSystemInfo },
    });

    expect(await screen.findByText(/Allow self-healing to act/i)).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByText("Appearance")).toBeTruthy();
    expect(screen.getByText("System Dependencies")).toBeTruthy();
    expect(screen.getByText("About ColimaUI")).toBeTruthy();
    expect(screen.queryByText(/^Account$/)).toBeNull();
    expect(screen.queryByRole("button", { name: /sign\s*in|sign\s*out|manage account/i })).toBeNull();
    expect(container.textContent).not.toMatch(/Sign in to ColimaUI|Signed in with/);
  });
});

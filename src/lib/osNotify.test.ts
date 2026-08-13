import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

/**
 * The restrictions are the feature. An OS notification leaves the app: it lands in
 * a system-wide centre with its own history, and on macOS can be mirrored to other
 * devices. So it fires only when nobody is looking at the window, only for outcomes,
 * and never carries the runtime's text.
 */

const { plugin, settings, tauri } = vi.hoisted(() => ({
  plugin: {
    isPermissionGranted: vi.fn(),
    requestPermission: vi.fn(),
    sendNotification: vi.fn(),
  },
  settings: { getAppSetting: vi.fn() },
  tauri: { value: true },
}));

vi.mock("@tauri-apps/plugin-notification", () => plugin);
vi.mock("./env", () => ({ isRunningInTauri: () => tauri.value }));
vi.mock("./settingsStore.svelte", () => settings);

import { osNotify, _resetOsNotifyForTest } from "./osNotify";

/** The window is in the background unless a test says otherwise. */
function setFocus(focused: boolean) {
  vi.spyOn(document, "hasFocus").mockReturnValue(focused);
}

beforeEach(() => {
  // The plugin mocks live in `vi.hoisted`, so they are the same objects for the
  // whole file — `restoreAllMocks` puts spies back but does not forget their calls.
  vi.clearAllMocks();
  _resetOsNotifyForTest();
  tauri.value = true;
  settings.getAppSetting.mockReturnValue("true");
  plugin.isPermissionGranted.mockResolvedValue(true);
  plugin.requestPermission.mockResolvedValue("granted");
  setFocus(false);
});

afterEach(() => vi.restoreAllMocks());

describe("osNotify", () => {
  it("notifies when the window is in the background", async () => {
    await osNotify("Transfer finished", "alpine:3.19");

    expect(plugin.sendNotification).toHaveBeenCalledWith({
      title: "Transfer finished",
      body: "alpine:3.19",
    });
  });

  it("stays quiet while the user is looking at the app", async () => {
    setFocus(true);
    await osNotify("Transfer finished");

    // They can see the panel; a system notification would be telling them
    // something they are already watching.
    expect(plugin.sendNotification).not.toHaveBeenCalled();
  });

  it("stays quiet when the user turned it off", async () => {
    settings.getAppSetting.mockReturnValue("false");
    await osNotify("Transfer finished");

    expect(plugin.sendNotification).not.toHaveBeenCalled();
  });

  it("does nothing in the browser", async () => {
    tauri.value = false;
    await osNotify("Transfer finished");

    expect(plugin.sendNotification).not.toHaveBeenCalled();
  });

  it("asks for permission only when there is something to show", async () => {
    plugin.isPermissionGranted.mockResolvedValue(false);
    await osNotify("Transfer finished");

    // Not at startup: a prompt with no context attached to it is one the user
    // cannot make an informed decision about.
    expect(plugin.requestPermission).toHaveBeenCalledTimes(1);
    expect(plugin.sendNotification).toHaveBeenCalled();
  });

  it("does not nag after the user says no", async () => {
    plugin.isPermissionGranted.mockResolvedValue(false);
    plugin.requestPermission.mockResolvedValue("denied");

    await osNotify("first");
    await osNotify("second");

    expect(plugin.requestPermission).toHaveBeenCalledTimes(1);
    expect(plugin.sendNotification).not.toHaveBeenCalled();
  });

  it("survives a plugin that is not there", async () => {
    plugin.isPermissionGranted.mockRejectedValue(new Error("no such plugin"));

    // A courtesy on top of a transfer that already resolved on its own terms must
    // not turn into a second failure.
    await expect(osNotify("Transfer finished")).resolves.toBeUndefined();
  });
});

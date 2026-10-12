import { beforeEach, expect, it, vi } from "vitest";
import { handleGlobalKeydown, QUICK_PAGES } from "./keyboardShortcuts";
import { uiState } from "../store.svelte";
import { notificationState } from "../store/notifications.svelte";

vi.mock("./dataPoller", () => ({
  refreshManual: vi.fn(),
  refetchAllResources: vi.fn(),
}));

function press(key: string, opts: Partial<KeyboardEventInit> = {}, target?: HTMLElement) {
  const e = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...opts });
  (target ?? document.body).dispatchEvent(e);
  return e;
}

// Mirrors <svelte:window onkeydown={...}> in App.svelte, so the events below
// travel the same path — bubbling up from the real target — as they do in the app.
beforeEach(() => {
  window.addEventListener("keydown", handleGlobalKeydown);
  return () => window.removeEventListener("keydown", handleGlobalKeydown);
});

beforeEach(() => {
  document.body.innerHTML = "";
  uiState.aiPanelOpen = false;
  uiState.currentPage = "dashboard";
  notificationState.panelOpen = false;
});

it("toggles the AI panel on the meta key + K", () => {
  press("k", { metaKey: true });
  expect(uiState.aiPanelOpen).toBe(true);
  press("k", { ctrlKey: true });
  expect(uiState.aiPanelOpen).toBe(false);
});

it("closes the notification panel when the AI panel opens", () => {
  notificationState.panelOpen = true;
  press("k", { metaKey: true });
  expect(notificationState.panelOpen).toBe(false);
});

it("leaves plain meta+R alone so the window can still reload", async () => {
  const { refreshManual } = await import("./dataPoller");
  const e = press("r", { metaKey: true });
  expect(e.defaultPrevented).toBe(false);
  expect(refreshManual).not.toHaveBeenCalled();
});

it("refreshes on meta+shift+R", async () => {
  const { refreshManual, refetchAllResources } = await import("./dataPoller");
  press("r", { metaKey: true, shiftKey: true });
  expect(refreshManual).toHaveBeenCalled();
  expect(refetchAllResources).toHaveBeenCalled();
});

it("maps meta+1..9 onto real page ids only", () => {
  press("3", { metaKey: true });
  expect(uiState.currentPage).toBe(QUICK_PAGES[2]);
  expect(QUICK_PAGES).not.toContain("ai-chat");
});

it("leaves keys to an input the user is typing in", () => {
  const input = document.createElement("input");
  document.body.appendChild(input);
  press("k", { metaKey: true }, input);
  expect(uiState.aiPanelOpen).toBe(false);
});

it("leaves keys to the terminal, whose textarea xterm owns", () => {
  const wrap = document.createElement("div");
  wrap.className = "xterm";
  const area = document.createElement("textarea");
  wrap.appendChild(area);
  document.body.appendChild(wrap);
  press("k", { metaKey: true }, area);
  expect(uiState.aiPanelOpen).toBe(false);
});

it("blurs the input on the first Escape, and closes a panel on the next", () => {
  const input = document.createElement("input");
  document.body.appendChild(input);
  input.focus();
  uiState.aiPanelOpen = true;

  press("Escape", {}, input);
  expect(document.activeElement).not.toBe(input);
  expect(uiState.aiPanelOpen).toBe(true);

  press("Escape");
  expect(uiState.aiPanelOpen).toBe(false);
});

it("closes one panel per Escape, outermost first", () => {
  uiState.aiPanelOpen = true;
  notificationState.panelOpen = true;
  press("Escape");
  expect(uiState.aiPanelOpen).toBe(false);
  expect(notificationState.panelOpen).toBe(true);
  press("Escape");
  expect(notificationState.panelOpen).toBe(false);
});

// The handler is registered on <svelte:window> in App.svelte, which mounting
// here would require the whole component tree; dispatching straight at it
// covers the logic, so this test only guards the wiring.
it("is wired to the window in App.svelte", async () => {
  const src = (await import("../App.svelte?raw")).default;
  expect(src).toContain("<svelte:window onkeydown={handleGlobalKeydown} />");
});

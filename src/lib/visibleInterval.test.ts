import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setVisibleInterval } from "./visibleInterval";

function setVisibility(state: DocumentVisibilityState) {
  vi.spyOn(document, "visibilityState", "get").mockReturnValue(state);
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it("runs the callback while the window is visible", () => {
  setVisibility("visible");
  const fn = vi.fn();
  const stop = setVisibleInterval(fn, 1000);
  vi.advanceTimersByTime(3000);
  expect(fn).toHaveBeenCalledTimes(3);
  stop();
});

it("skips ticks while the window is hidden, and resumes when it comes back", () => {
  setVisibility("hidden");
  const fn = vi.fn();
  const stop = setVisibleInterval(fn, 1000);
  vi.advanceTimersByTime(3000);
  expect(fn).not.toHaveBeenCalled();

  setVisibility("visible");
  vi.advanceTimersByTime(1000);
  expect(fn).toHaveBeenCalledTimes(1);
  stop();
});

it("stops firing once the returned teardown runs", () => {
  setVisibility("visible");
  const fn = vi.fn();
  setVisibleInterval(fn, 1000)();
  vi.advanceTimersByTime(5000);
  expect(fn).not.toHaveBeenCalled();
});

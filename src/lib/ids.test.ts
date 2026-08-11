import { describe, it, expect, vi, afterEach } from "vitest";
import { newId } from "./ids";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("newId", () => {
  it("does not collide inside a single millisecond", () => {
    // The regression: `Date.now().toString()` returned the same value for every
    // item created in the same tick, which both broke the keyed `{#each}` and
    // silently overwrote persisted chat messages.
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-11T00:00:00.000Z"));

    const ids = Array.from({ length: 1000 }, () => newId());
    expect(new Set(ids).size).toBe(ids.length);

    vi.useRealTimers();
  });

  it("applies the prefix when given", () => {
    expect(newId("cron")).toMatch(/^cron-/);
    expect(newId()).not.toMatch(/^cron-/);
  });

  it("stays collision-free on the fallback path", () => {
    // Older webviews, or a non-secure context, have no crypto.randomUUID.
    vi.stubGlobal("crypto", {});
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-11T00:00:00.000Z"));

    const ids = Array.from({ length: 1000 }, () => newId());
    expect(new Set(ids).size).toBe(ids.length);

    vi.useRealTimers();
  });

  it("returns a non-empty string", () => {
    expect(newId().length).toBeGreaterThan(0);
  });
});

import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/svelte";
import Sparkline from "./Sparkline.svelte";

/**
 * "Show a gap when samples were dropped, do not interpolate across it" was until
 * now only checked at the data layer — `MetricsHistory` stores a `null`. Whether
 * the *drawing* honours it is a separate question, and the one the user actually
 * sees: a single polyline through a gap asserts a steady load that was never
 * observed, and there is no way to tell it from real data.
 */

afterEach(cleanup);

const draw = (values: Array<number | null>, props: Record<string, unknown> = {}) =>
  render(Sparkline, { props: { values, ...props } }).container;

describe("Sparkline", () => {
  it("draws one line for an unbroken series", () => {
    const c = draw([1, 2, 3, 4]);
    expect(c.querySelectorAll("polyline")).toHaveLength(1);
  });

  it("breaks the line at a gap instead of drawing through it", () => {
    const c = draw([1, 2, null, 5, 6]);
    const lines = c.querySelectorAll("polyline");
    expect(lines).toHaveLength(2);

    // Neither segment may span the hole: the last point before it and the first
    // point after it must not appear in the same polyline.
    for (const line of lines) {
      const xs = (line.getAttribute("points") ?? "")
        .split(" ")
        .map((p) => Number(p.split(",")[0]));
      expect(Math.max(...xs) - Math.min(...xs)).toBeLessThan(60);
    }
  });

  it("breaks at every gap, not only the first", () => {
    const c = draw([1, null, 3, null, 5, 6]);
    // Runs: [1] (a dot), [3] (a dot), [5,6] (a line).
    expect(c.querySelectorAll("polyline")).toHaveLength(1);
    expect(c.querySelectorAll("circle")).toHaveLength(2);
  });

  it("marks an isolated sample so a lone reading is not invisible", () => {
    // One point between two gaps has no line to draw; dropping it silently would
    // read as "no data" when there was some.
    const c = draw([null, 7, null]);
    expect(c.querySelectorAll("polyline")).toHaveLength(0);
    expect(c.querySelectorAll("circle")).toHaveLength(1);
  });

  it("renders nothing rather than failing on an all-gap series", () => {
    const c = draw([null, null, null]);
    expect(c.querySelectorAll("polyline")).toHaveLength(0);
    expect(c.querySelectorAll("circle")).toHaveLength(0);
    expect(c.querySelector("svg")).toBeTruthy();
  });

  it("survives an empty series", () => {
    const c = draw([]);
    expect(c.querySelector("svg")).toBeTruthy();
    expect(c.querySelectorAll("polyline")).toHaveLength(0);
  });

  it("scales to the data when no ceiling is given", () => {
    // CPU exceeds 100% on multiple cores, so a fixed ceiling would clip the
    // interesting part of the chart flat.
    const low = draw([0, 1]).querySelector("polyline")!.getAttribute("points")!;
    const high = draw([0, 400]).querySelector("polyline")!.getAttribute("points")!;
    // Same shape either way: the peak reaches the top of the box in both.
    expect(low).toBe(high);
  });

  it("honours an explicit ceiling", () => {
    const half = draw([0, 50], { max: 100 }).querySelector("polyline")!.getAttribute("points")!;
    const full = draw([0, 100], { max: 100 }).querySelector("polyline")!.getAttribute("points")!;
    expect(half).not.toBe(full);
  });

  it("does not divide by zero on a flat-zero series", () => {
    const c = draw([0, 0, 0]);
    const points = c.querySelector("polyline")!.getAttribute("points")!;
    expect(points).not.toContain("NaN");
    expect(points).not.toContain("Infinity");
  });
});

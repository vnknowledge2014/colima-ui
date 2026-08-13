import { describe, it, expect } from "vitest";
import { MetricsHistory, DEFAULT_LIMIT } from "./metricsHistory";
import type { MetricSample } from "./api/metrics";

function sample(id: string, ts: number, cpu = 1): MetricSample {
  return {
    ts,
    instance: "colima",
    containerId: id,
    name: `name-${id}`,
    cpuPct: cpu,
    memBytes: 1024,
    memLimitBytes: 4096,
    memPct: 25,
    netRxBytes: 10,
    netTxBytes: 20,
    blockReadBytes: 30,
    blockWriteBytes: 40,
    pids: 5,
  };
}

describe("MetricsHistory", () => {
  it("keeps the most recent sample per container", () => {
    const h = new MetricsHistory();
    h.push([sample("a", 1, 10), sample("b", 1, 20)]);
    h.push([sample("a", 2, 11), sample("b", 2, 21)]);

    const current = h.current().sort((x, y) => x.containerId.localeCompare(y.containerId));
    expect(current.map((s) => [s.containerId, s.cpuPct])).toEqual([
      ["a", 11],
      ["b", 21],
    ]);
  });

  it("caps each series at the limit", () => {
    const h = new MetricsHistory(5);
    for (let i = 0; i < 50; i++) h.push([sample("a", i, i)]);

    const series = h.seriesFor("a", "cpuPct");
    expect(series).toHaveLength(5);
    // Oldest first, and it kept the newest five.
    expect(series).toEqual([45, 46, 47, 48, 49]);
  });

  it("forgets containers that stop appearing", () => {
    // Otherwise an hour of churn accumulates a series per container ever seen,
    // none of which the table displays.
    const h = new MetricsHistory();
    h.push([sample("a", 1), sample("b", 1)]);
    expect(h.size()).toBe(2);

    h.push([sample("a", 2)]);
    expect(h.size()).toBe(1);
    expect(h.seriesFor("b", "cpuPct")).toEqual([]);
  });

  it("stays bounded over a long session with heavy churn", () => {
    // The success criterion: an hour on screen must not grow without bound.
    // 1800 ticks is an hour at a 2s period.
    const h = new MetricsHistory();
    for (let tick = 0; tick < 1800; tick++) {
      // Twenty stable containers plus one that is replaced every tick.
      const batch = Array.from({ length: 20 }, (_, i) => sample(`stable-${i}`, tick));
      batch.push(sample(`ephemeral-${tick}`, tick));
      h.push(batch);
    }
    expect(h.size()).toBe(21);
    expect(h.pointCount()).toBeLessThanOrEqual(21 * DEFAULT_LIMIT);
  });

  it("records a gap instead of joining across dropped samples", () => {
    const h = new MetricsHistory();
    h.push([sample("a", 1, 10)]);
    h.markGap();
    h.push([sample("a", 3, 30)]);

    expect(h.seriesFor("a", "cpuPct")).toEqual([10, null, 30]);
  });

  it("reports a container as stale while its last point is a gap", () => {
    const h = new MetricsHistory();
    h.push([sample("a", 1)]);
    expect(h.isStale("a")).toBe(false);

    h.markGap();
    expect(h.isStale("a")).toBe(true);
    // A gap has no value, so it must not surface as a table row.
    expect(h.current()).toEqual([]);

    h.push([sample("a", 2)]);
    expect(h.isStale("a")).toBe(false);
    expect(h.current()).toHaveLength(1);
  });

  it("caps gaps too, so a disconnected stream cannot grow the series", () => {
    const h = new MetricsHistory(4);
    h.push([sample("a", 1)]);
    for (let i = 0; i < 100; i++) h.markGap();
    expect(h.seriesFor("a", "cpuPct")).toEqual([null, null, null, null]);
  });

  it("returns an empty series for an unknown container", () => {
    const h = new MetricsHistory();
    expect(h.seriesFor("nope", "cpuPct")).toEqual([]);
    expect(h.isStale("nope")).toBe(false);
  });

  it("exposes every chartable field", () => {
    const h = new MetricsHistory();
    h.push([sample("a", 1)]);
    expect(h.seriesFor("a", "memBytes")).toEqual([1024]);
    expect(h.seriesFor("a", "netRxBytes")).toEqual([10]);
    expect(h.seriesFor("a", "blockWriteBytes")).toEqual([40]);
    expect(h.seriesFor("a", "pids")).toEqual([5]);
  });
});

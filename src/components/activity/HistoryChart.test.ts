import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, screen } from "@testing-library/svelte";

import HistoryChart from "./HistoryChart.svelte";
import type { HistorySeries, HistoryPoint } from "../../lib/api/metrics";

/**
 * The one behaviour this chart exists to get right: a hole in the data is drawn
 * as a hole. Interpolating across it draws load nobody observed, and the flat
 * line that results is indistinguishable from a genuinely idle container —
 * which is exactly the reading someone would take away at 3am.
 */

function point(ts: number, cpu: number, id = "abc"): HistoryPoint {
  return {
    ts,
    containerId: id,
    cpuPct: cpu,
    memBytes: 1024,
    memPct: 10,
    netRxBytes: 0,
    netTxBytes: 0,
    blockReadBytes: 0,
    blockWriteBytes: 0,
  };
}

function series(points: HistoryPoint[], over: Partial<HistorySeries> = {}): HistorySeries {
  return {
    resolution: "raw",
    expectedStepMs: 2000,
    points,
    names: { abc: "web" },
    ...over,
  };
}

afterEach(cleanup);

describe("HistoryChart", () => {
  it("draws a continuous run as one path", () => {
    const { container } = render(HistoryChart, {
      series: series([point(0, 10), point(2000, 20), point(4000, 30)]),
      field: "cpuPct",
    });
    expect(container.querySelectorAll("path")).toHaveLength(1);
  });

  it("breaks the line where samples are missing", () => {
    // Two runs either side of a ten-minute hole — the machine was asleep.
    const { container } = render(HistoryChart, {
      series: series([
        point(0, 10),
        point(2000, 20),
        point(600_000, 30),
        point(602_000, 40),
      ]),
      field: "cpuPct",
    });
    expect(container.querySelectorAll("path")).toHaveLength(2);
  });

  it("tolerates a late sample without calling it a gap", () => {
    // The collector's period is nominal; a busy daemon delivers late, and
    // splitting the line there would show holes that are not there.
    const { container } = render(HistoryChart, {
      series: series([point(0, 10), point(2000, 20), point(6000, 30)]),
      field: "cpuPct",
    });
    expect(container.querySelectorAll("path")).toHaveLength(1);
  });

  it("says which resolution it is showing", () => {
    // A one-minute average is a different claim from a two-second sample.
    render(HistoryChart, {
      series: series([point(0, 10)], { resolution: "1m", expectedStepMs: 60_000 }),
      field: "cpuPct",
    });
    expect(screen.getByText(/1-minute averages/i)).toBeInTheDocument();
  });

  it("labels lines with container names, including deleted ones", () => {
    render(HistoryChart, { series: series([point(0, 10)]), field: "cpuPct" });
    expect(screen.getByText("web")).toBeInTheDocument();
  });

  it("says there is nothing rather than drawing an empty chart", () => {
    const { container } = render(HistoryChart, { series: series([]), field: "cpuPct" });
    expect(container.querySelector("svg")).toBeNull();
    expect(screen.getByText(/No history recorded/i)).toBeInTheDocument();
  });
});

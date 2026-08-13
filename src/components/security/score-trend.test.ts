import { describe, expect, it } from "vitest";
import { segmentDelta, segmentRuns } from "./score-trend";
import type { ScanRun } from "../../lib/api/security";

function run(score: number, packVersion: string, scannedAt: number, scanner = "trivy"): ScanRun {
  return {
    imageRef: "nginx:1.25",
    imageDigest: "sha256:aaa",
    score,
    breakdownJson: "{}",
    packVersion,
    scanner,
    scannerVersion: "0.73.0",
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
    scannedAt,
  };
}

describe("segmentRuns", () => {
  it("keeps one ruler's measurements on one line", () => {
    const segments = segmentRuns([run(60, "v1", 1), run(64, "v1", 2), run(71, "v1", 3)]);
    expect(segments).toHaveLength(1);
    expect(segments[0].runs.map((r) => r.score)).toEqual([60, 64, 71]);
  });

  it("breaks the line when the rule pack changes", () => {
    // The jump from 62 to 81 might be a better image or a looser ruler. The
    // chart cannot tell, so it must not claim to.
    const segments = segmentRuns([run(60, "v1", 1), run(62, "v1", 2), run(81, "v2", 3)]);
    expect(segments).toHaveLength(2);
    expect(segments[0].packVersion).toBe("v1");
    expect(segments[1].packVersion).toBe("v2");
    expect(segments[1].runs.map((r) => r.score)).toEqual([81]);
  });

  it("breaks the line when the scanner changes", () => {
    const segments = segmentRuns([run(60, "v1", 1), run(60, "v1", 2, "grype")]);
    expect(segments).toHaveLength(2);
  });

  it("does not rejoin a pack that comes back after a gap", () => {
    // Rolling back to v1 gives three segments. Merging the two v1 stretches
    // would draw a line straight across the period v2 was in force.
    const segments = segmentRuns([run(60, "v1", 1), run(81, "v2", 2), run(63, "v1", 3)]);
    expect(segments).toHaveLength(3);
    expect(segments.map((s) => s.packVersion)).toEqual(["v1", "v2", "v1"]);
  });

  it("has nothing to segment when there is no history", () => {
    expect(segmentRuns([])).toEqual([]);
  });
});

describe("segmentDelta", () => {
  it("reports the change across a segment", () => {
    expect(segmentDelta({ packVersion: "v1", scanner: "trivy", runs: [run(60, "v1", 1), run(72, "v1", 2)] })).toBe(12);
  });

  it("refuses to state a trend from a single point", () => {
    expect(segmentDelta({ packVersion: "v1", scanner: "trivy", runs: [run(60, "v1", 1)] })).toBeNull();
  });
});

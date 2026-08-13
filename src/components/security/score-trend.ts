/**
 * Splitting a score history into segments that may honestly be joined.
 *
 * A score is a measurement, and two measurements only belong on the same line
 * if the same instrument produced them. Change the rule pack — or the scanner —
 * and the number means something different: a jump from 62 to 81 might be an
 * image that improved, or the same image measured by a looser ruler. Drawing
 * one continuous line across that boundary asserts the first, and the chart has
 * no way to know.
 *
 * So the series breaks. Each segment is labelled with the pack that measured
 * it, and the gap is the honest part of the picture.
 */
import type { ScanRun } from "../../lib/api/security";

export interface TrendSegment {
  /** The ruler this segment was measured with. */
  packVersion: string;
  scanner: string;
  runs: ScanRun[];
}

/**
 * Group consecutive runs that share a pack version and scanner.
 *
 * Consecutive, not merely equal: a user who moves to pack v2 and later rolls
 * back to v1 has three segments, not two. Merging the two v1 stretches would
 * draw a line across the v2 period as though it had not happened.
 */
export function segmentRuns(runs: ScanRun[]): TrendSegment[] {
  const segments: TrendSegment[] = [];
  for (const run of runs) {
    const current = segments[segments.length - 1];
    if (current && current.packVersion === run.packVersion && current.scanner === run.scanner) {
      current.runs.push(run);
      continue;
    }
    segments.push({ packVersion: run.packVersion, scanner: run.scanner, runs: [run] });
  }
  return segments;
}

/** The direction of travel within one segment, or null when it has one point. */
export function segmentDelta(segment: TrendSegment): number | null {
  if (segment.runs.length < 2) return null;
  return segment.runs[segment.runs.length - 1].score - segment.runs[0].score;
}

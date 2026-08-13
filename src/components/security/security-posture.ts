/**
 * Aggregating what has been scanned into one posture.
 *
 * Everything here is derived from audits already held in memory — no request is
 * made, and no image is scanned to fill a gap in the summary. A machine's
 * posture is what its user chose to measure, and the summary says how much of
 * the machine that covers.
 */
import type { SecurityAudit, ScoreInputs, Severity } from "../../lib/api/security";

export const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "unknown"];

export type SeverityCounts = Record<Severity, number>;

export interface Posture {
  /** How many images have an audit, out of how many exist locally. */
  scanned: number;
  total: number;
  /**
   * Mean score over scanned images, floored.
   *
   * `null` rather than `0` when nothing has been scanned: zero is a score an
   * image can actually earn, and showing it for "not measured" would be a
   * verdict nobody asked for.
   */
  average: number | null;
  /** The worst scanned image — the one worth looking at first. */
  lowest: { ref: string; score: number } | null;
  severity: SeverityCounts;
  /** From the most recent audit, so the header can say what produced these numbers. */
  inputs: ScoreInputs | null;
  /**
   * True when the audits on screen were not all produced by the same pack,
   * engine or scanner build. Their scores are then not comparable with each
   * other, and the header has to say so rather than average them silently.
   */
  mixed: boolean;
}

/** One line of the image list: what it scored, or why it has no score. */
export interface ImageRow {
  /** Unique per local image — the reference is not, for untagged images. */
  key: string;
  ref: string;
  score: number | null;
  status: "scanned" | "scanning" | "failed" | "unscanned";
}

/** How many of the worst-scoring images the overview shows before it stops. */
export const OVERVIEW_WORST_LIMIT = 5;

/** The shortened image list an overview shows, and what it left out. */
export interface OverviewList {
  rows: ImageRow[];
  /** Scored images below the cut, for the "and N more" line. */
  hidden: number;
}

/**
 * The worst few scored images, then every image with no score.
 *
 * Enumerating all of them is the images section's job; repeating it here made
 * the overview grow with the machine until it summarised nothing. Unscored
 * images are never cut, because they are the part of the picture that has not
 * been measured — a count of them would understate how much is unknown, and the
 * user cannot act on what they cannot see.
 */
export function overviewRows(rows: ImageRow[], limit = OVERVIEW_WORST_LIMIT): OverviewList {
  const scored = rows
    .filter((r) => r.score !== null)
    .sort((a, b) => a.score! - b.score! || a.ref.localeCompare(b.ref));
  const unscored = rows.filter((r) => r.score === null).sort((a, b) => a.ref.localeCompare(b.ref));

  return {
    rows: [...scored.slice(0, limit), ...unscored],
    hidden: Math.max(0, scored.length - limit),
  };
}

function emptyCounts(): SeverityCounts {
  return { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
}

/** The identity of a scoring run — two audits agreeing on this are comparable. */
function inputsKey(inputs: ScoreInputs): string {
  return [inputs.packVersion, inputs.engineVersion, inputs.scanner, inputs.scannerVersion, inputs.level].join("|");
}

export function summarizePosture(audits: Record<string, SecurityAudit>, total: number): Posture {
  // Sorted so that ties — two images with the same lowest score — always resolve
  // to the same one, instead of following object insertion order.
  const refs = Object.keys(audits).sort();
  const severity = emptyCounts();
  let sum = 0;
  let lowest: Posture["lowest"] = null;
  let newest: { at: number; inputs: ScoreInputs } | null = null;
  const keys = new Set<string>();

  for (const ref of refs) {
    const audit = audits[ref];
    const score = audit.score.total;
    sum += score;
    if (!lowest || score < lowest.score) lowest = { ref, score };

    for (const finding of audit.scan.findings) {
      // A severity the scanner invented is still a finding; count it as unknown
      // rather than dropping it out of the total.
      const bucket: Severity = finding.severity in severity ? finding.severity : "unknown";
      severity[bucket] += 1;
    }

    keys.add(inputsKey(audit.score.inputs));
    if (!newest || audit.scan.scannedAt > newest.at) {
      newest = { at: audit.scan.scannedAt, inputs: audit.score.inputs };
    }
  }

  return {
    scanned: refs.length,
    total,
    average: refs.length > 0 ? Math.floor(sum / refs.length) : null,
    lowest,
    severity,
    inputs: newest?.inputs ?? null,
    mixed: keys.size > 1,
  };
}

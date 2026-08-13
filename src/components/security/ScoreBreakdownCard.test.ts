import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, screen } from "@testing-library/svelte";

import ScoreBreakdownCard from "./ScoreBreakdownCard.svelte";
import type { ScoreBreakdown } from "../../lib/api/security";

/**
 * The contract this component exists to keep: a score is never shown bare.
 *
 * The same image scores differently against a newer vulnerability database, and
 * two scanners disagree by more than tenfold on identical input — so the scanner,
 * the database date and the pack version have to be on screen next to the number,
 * not behind a hover.
 */

function breakdown(over: Partial<ScoreBreakdown> = {}): ScoreBreakdown {
  return {
    vulnerabilities: { earned: 20, max: 40, failedRules: [] },
    hardening: { earned: 17, max: 25, failedRules: ["runs-as-root"] },
    provenance: { earned: 14, max: 20, failedRules: ["mutable-tag"] },
    freshness: { earned: 15, max: 15, failedRules: [] },
    total: 66,
    inputs: {
      packVersion: "1.0.0",
      engineVersion: "0.1.10",
      scanner: "trivy",
      scannerVersion: "0.73.0",
      dbSnapshotDate: "2026-08-12T01:10:20.477Z",
      level: "l1",
    },
    ...over,
  };
}

afterEach(cleanup);

describe("ScoreBreakdownCard", () => {
  it("shows what the score is made of, not just the total", () => {
    render(ScoreBreakdownCard, { score: breakdown() });

    expect(screen.getByText("66")).toBeInTheDocument();
    for (const label of ["Vulnerabilities", "Hardening", "Provenance", "Freshness"]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
    // Each component reports its own denominator: 17/25 and 17/40 are very
    // different answers and the bar alone cannot tell them apart.
    expect(screen.getByText("/25")).toBeInTheDocument();
  });

  it("prints the three facts that make a score comparable", () => {
    render(ScoreBreakdownCard, { score: breakdown() });

    expect(screen.getByText(/trivy 0\.73\.0/)).toBeInTheDocument();
    expect(screen.getByText("2026-08-12")).toBeInTheDocument();
    expect(screen.getByText("1.0.0")).toBeInTheDocument();
    expect(screen.getByText("L1")).toBeInTheDocument();
  });

  it("says so rather than inventing a date when the database date is unknown", () => {
    const score = breakdown();
    score.inputs.dbSnapshotDate = undefined;
    render(ScoreBreakdownCard, { score });

    expect(screen.getByText("unknown")).toBeInTheDocument();
  });

  it("explains a shrunken maximum instead of letting it look arbitrary", () => {
    // A rule the build cannot run leaves the denominator; without this line the
    // user sees 17/20 with no idea why the 25 became 20.
    const score = breakdown({
      hardening: { earned: 17, max: 20, failedRules: [] },
    });
    score.inputs.skippedRules = ["from-a-newer-pack"];
    render(ScoreBreakdownCard, { score });

    expect(screen.getByText(/could not be run by this build/i)).toBeInTheDocument();
  });

  it("reports progress accessibly, so the bars are not the only signal", () => {
    const { container } = render(ScoreBreakdownCard, { score: breakdown() });
    const meters = container.querySelectorAll('[role="meter"]');
    expect(meters).toHaveLength(4);
    expect(meters[0].getAttribute("aria-valuenow")).toBe("20");
    expect(meters[0].getAttribute("aria-valuemax")).toBe("40");
  });
});

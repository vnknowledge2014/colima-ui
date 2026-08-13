import { describe, it, expect } from "vitest";

import { summarizePosture, overviewRows } from "./security-posture";
import type { ImageRow } from "./security-posture";
import type { ScoreInputs, SecurityAudit, Severity } from "../../lib/api/security";

function row(ref: string, score: number | null): ImageRow {
  return { key: ref, ref, score, status: score === null ? "unscanned" : "scanned" };
}

function inputs(over: Partial<ScoreInputs> = {}): ScoreInputs {
  return {
    packVersion: "1.0.0",
    engineVersion: "0.1.10",
    scanner: "trivy",
    scannerVersion: "0.73.0",
    dbSnapshotDate: "2026-08-12T01:10:20.477Z",
    level: "l1",
    ...over,
  };
}

function audit(total: number, severities: Severity[] = [], over: Partial<ScoreInputs> = {}, scannedAt = 1): SecurityAudit {
  return {
    scan: {
      imageRef: "img",
      imageDigest: "sha256:x",
      findings: severities.map((severity, i) => ({
        id: `CVE-${i}`,
        package: "openssl",
        installedVersion: "1.0",
        severity,
      })),
      scanner: "trivy",
      scannerVersion: "0.73.0",
      scannedAt,
    },
    evaluation: { results: [], skipped: [] },
    score: {
      vulnerabilities: { earned: 0, max: 40, failedRules: [] },
      hardening: { earned: 0, max: 25, failedRules: [] },
      provenance: { earned: 0, max: 20, failedRules: [] },
      freshness: { earned: 0, max: 15, failedRules: [] },
      total,
      inputs: inputs(over),
    },
  };
}

describe("summarizePosture", () => {
  it("reports no average when nothing has been scanned", () => {
    // Zero is a score an image can earn; "not measured" has to look different.
    const posture = summarizePosture({}, 4);
    expect(posture.average).toBeNull();
    expect(posture.lowest).toBeNull();
    expect(posture.scanned).toBe(0);
    expect(posture.total).toBe(4);
    expect(posture.inputs).toBeNull();
  });

  it("averages scanned images and names the worst one", () => {
    const posture = summarizePosture({ "a:1": audit(80), "b:1": audit(41) }, 5);
    expect(posture.average).toBe(60); // floored from 60.5
    expect(posture.lowest).toEqual({ ref: "b:1", score: 41 });
    expect(posture.scanned).toBe(2);
  });

  it("counts findings by severity across every scanned image", () => {
    const posture = summarizePosture(
      { "a:1": audit(50, ["critical", "high", "high"]), "b:1": audit(90, ["low"]) },
      2,
    );
    expect(posture.severity).toEqual({ critical: 1, high: 2, medium: 0, low: 1, unknown: 0 });
  });

  it("takes provenance from the most recent scan", () => {
    const posture = summarizePosture(
      {
        "a:1": audit(50, [], { packVersion: "1.0.0" }, 10),
        "b:1": audit(50, [], { packVersion: "1.0.0" }, 20),
      },
      2,
    );
    expect(posture.inputs?.packVersion).toBe("1.0.0");
    expect(posture.mixed).toBe(false);
  });

  it("flags audits produced by different builds as not comparable", () => {
    const posture = summarizePosture(
      { "a:1": audit(50, [], { packVersion: "1.0.0" }), "b:1": audit(50, [], { packVersion: "1.1.0" }) },
      2,
    );
    expect(posture.mixed).toBe(true);
  });
});

describe("overviewRows", () => {
  it("keeps only the worst five scores, lowest first", () => {
    const list = overviewRows([90, 10, 70, 30, 50, 20, 40].map((s, i) => row(`img-${i}:1`, s)));

    expect(list.rows.map((r) => r.score)).toEqual([10, 20, 30, 40, 50]);
    expect(list.hidden).toBe(2);
  });

  it("keeps every unscored image, however many are cut from the scored ones", () => {
    const scored = [1, 2, 3, 4, 5, 6, 7, 8].map((s) => row(`scored-${s}:1`, s));
    const unscored = ["a", "b", "c", "d", "e", "f"].map((n) => row(`${n}:1`, null));

    const list = overviewRows([...scored, ...unscored]);

    expect(list.rows.filter((r) => r.score === null)).toHaveLength(unscored.length);
    expect(list.hidden).toBe(3);
  });

  it("puts unscored images after the scored ones, alphabetically", () => {
    const list = overviewRows([row("z:1", null), row("a:1", null), row("m:1", 40)]);

    expect(list.rows.map((r) => r.ref)).toEqual(["m:1", "a:1", "z:1"]);
  });

  it("reports nothing hidden when the machine has five or fewer scored images", () => {
    const list = overviewRows([row("a:1", 10), row("b:1", 20)]);

    expect(list.hidden).toBe(0);
    expect(list.rows).toHaveLength(2);
  });

  it("leaves the caller's array untouched", () => {
    const rows = [row("b:1", 20), row("a:1", 10)];

    overviewRows(rows);

    expect(rows.map((r) => r.ref)).toEqual(["b:1", "a:1"]);
  });
});

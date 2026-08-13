import { describe, it, expect } from "vitest";

import { nextActions } from "./security-actions";
import type { CatalogSuggestions, RulePack, RuleResult, SecurityAudit } from "../../lib/api/security";

function audit(results: Partial<RuleResult>[]): SecurityAudit {
  return {
    scan: {
      imageRef: "img",
      imageDigest: "sha256:x",
      findings: [],
      scanner: "trivy",
      scannerVersion: "0.73.0",
      scannedAt: 1,
    },
    evaluation: {
      results: results.map((r, i) => ({
        ruleId: r.ruleId ?? `rule-${i}`,
        passed: r.passed ?? false,
        severity: r.severity ?? "high",
        component: r.component ?? "hardening",
        weight: r.weight ?? 5,
      })),
      skipped: [],
    },
    score: {
      vulnerabilities: { earned: 0, max: 40, failedRules: [] },
      hardening: { earned: 0, max: 25, failedRules: [] },
      provenance: { earned: 0, max: 20, failedRules: [] },
      freshness: { earned: 0, max: 15, failedRules: [] },
      total: 50,
      inputs: {
        packVersion: "1.0.0",
        engineVersion: "0.1.10",
        scanner: "trivy",
        scannerVersion: "0.73.0",
        level: "l1",
      },
    },
  };
}

function suggestions(images: string[]): CatalogSuggestions {
  return {
    catalogVersion: "1",
    updatedAt: "2026-08-01",
    alternatives: images.map((image) => ({ image, why: "smaller base" })),
  };
}

const pack: RulePack = {
  packVersion: "1.0.0",
  componentMax: { hardening: 25, provenance: 20, freshness: 15 },
  rules: [
    {
      id: "runs-as-root",
      title: "Runs as root",
      rationale: "r",
      remediation: "Add a USER instruction",
      severity: "high",
      component: "hardening",
      weight: 20,
      minLevel: "l1",
      standardRefs: [],
    },
  ],
};

describe("nextActions", () => {
  it("lists only failed rules", () => {
    const list = nextActions({ "a:1": audit([{ ruleId: "ok", passed: true }, { ruleId: "bad" }]) }, {}, null);
    expect(list.items.map((i) => i.title)).toEqual(["bad"]);
  });

  it("ranks by the points a fix returns, and sinks what cannot be measured", () => {
    const list = nextActions(
      { "a:1": audit([{ ruleId: "small", weight: 3 }, { ruleId: "big", weight: 20 }]) },
      { "a:1": suggestions(["alpine"]) },
      null,
    );
    expect(list.items.map((i) => i.estimatedGain)).toEqual([20, 3, null]);
  });

  it("uses the pack's wording when it is loaded, and the rule id when it is not", () => {
    const audits = { "a:1": audit([{ ruleId: "runs-as-root", weight: 20 }]) };
    expect(nextActions(audits, {}, pack).items[0]).toMatchObject({
      title: "Runs as root",
      detail: "Add a USER instruction",
    });
    expect(nextActions(audits, {}, null).items[0]).toMatchObject({ title: "runs-as-root", detail: "" });
  });

  it("reports what the limit left out instead of dropping it silently", () => {
    const results = Array.from({ length: 10 }, (_, i) => ({ ruleId: `r${i}`, weight: i }));
    const list = nextActions({ "a:1": audit(results) }, {}, null, 4);
    expect(list.items).toHaveLength(4);
    expect(list.remaining).toBe(6);
  });

  it("says nothing is left when every rule passed", () => {
    const list = nextActions({ "a:1": audit([{ ruleId: "ok", passed: true }]) }, {}, null);
    expect(list.items).toHaveLength(0);
    expect(list.remaining).toBe(0);
  });
});

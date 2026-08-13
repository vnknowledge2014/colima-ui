import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";

import RuleChecklist from "./RuleChecklist.svelte";
import type { Evaluation, RulePack } from "../../lib/api/security";

/**
 * The checklist is where a score turns into something to do. What matters:
 * failures lead with their remediation, rule text is rendered as text, and a
 * rule the build could not run is not quietly presented as a pass.
 */

const pack: RulePack = {
  packVersion: "1.0.0",
  componentMax: { hardening: 25, provenance: 20, freshness: 15 },
  rules: [
    {
      id: "runs-as-root",
      title: "Container runs as root by default",
      rationale: "Processes start as uid 0.",
      remediation: "Create an unprivileged account and end the Dockerfile with USER.",
      severity: "high",
      component: "hardening",
      weight: 8,
      minLevel: "l1",
      standardRefs: [{ standard: "CIS Docker Benchmark", version: "1.8.0", id: "4.1" }],
    },
    {
      id: "no-digest-pin",
      title: "<script>alert(1)</script>",
      rationale: "Injected on purpose by this test.",
      remediation: "Push to a registry and deploy by digest.",
      severity: "medium",
      component: "provenance",
      weight: 10,
      minLevel: "l1",
      standardRefs: [],
    },
  ],
};

function evaluation(over: Partial<Evaluation> = {}): Evaluation {
  return {
    results: [
      { ruleId: "runs-as-root", passed: false, severity: "high", component: "hardening", weight: 8, evidence: "no USER instruction" },
      { ruleId: "no-digest-pin", passed: true, severity: "medium", component: "provenance", weight: 10 },
    ],
    skipped: [],
    ...over,
  };
}

afterEach(cleanup);

describe("RuleChecklist", () => {
  it("leads a failure with what to do about it", () => {
    render(RuleChecklist, { evaluation: evaluation(), pack });

    expect(screen.getByText("Container runs as root by default")).toBeInTheDocument();
    expect(screen.getByText(/end the Dockerfile with USER/)).toBeInTheDocument();
    expect(screen.getByText("no USER instruction")).toBeInTheDocument();
    // The identifier is a pointer into the standard, not a claim of compliance.
    expect(screen.getByText(/CIS Docker Benchmark v1\.8\.0 §4\.1/)).toBeInTheDocument();
  });

  it("renders pack text as text", () => {
    // The pack will be downloadable one day. `{@html}` on downloadable content
    // is how a rule pack becomes a script.
    const { container } = render(RuleChecklist, {
      evaluation: evaluation({
        results: [
          { ruleId: "no-digest-pin", passed: false, severity: "medium", component: "provenance", weight: 10 },
        ],
      }),
      pack,
    });

    expect(container.querySelector("script")).toBeNull();
    expect(screen.getByText("<script>alert(1)</script>")).toBeInTheDocument();
  });

  it("keeps passing rules out of the way until asked", async () => {
    render(RuleChecklist, { evaluation: evaluation(), pack });

    expect(screen.queryByText("<script>alert(1)</script>")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByText(/Show 1 passing rules/));
    expect(screen.getByText("<script>alert(1)</script>")).toBeInTheDocument();
  });

  it("says when a rule was not run rather than implying it passed", () => {
    render(RuleChecklist, {
      evaluation: evaluation({
        skipped: [{ ruleId: "from-the-future", component: "hardening", weight: 4 }],
      }),
      pack,
    });

    expect(screen.getByText(/newer than this build and were not run/i)).toBeInTheDocument();
  });

  it("still names what failed when the pack is unavailable", () => {
    // The pack is a separate request; losing it must not blank the checklist.
    render(RuleChecklist, { evaluation: evaluation(), pack: null });
    expect(screen.getByText("runs-as-root")).toBeInTheDocument();
  });
});

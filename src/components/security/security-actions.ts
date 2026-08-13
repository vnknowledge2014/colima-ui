/**
 * Turning audits into the shortest list of things worth doing next.
 *
 * The list is ordered by what each item is worth, because "16 rules failed" is
 * not a plan and a person reading it has to rank them by hand. Only what the
 * audit already proved is listed — nothing here is a guess about an image that
 * has not been scanned.
 */
import type { Alternative, CatalogSuggestions, RulePack, SecurityAudit } from "../../lib/api/security";

export interface NextAction {
  imageRef: string;
  kind: "rule" | "alternative";
  /** Stable within a run, so a list item can key on it. */
  id: string;
  title: string;
  detail: string;
  /**
   * Points this would return to the score, or `null` when it cannot be known.
   *
   * A failed rule has a weight, so its gain is exact. Swapping a base image
   * changes the whole scan, and claiming a number for it would be inventing
   * one — those say "not measured" instead of guessing.
   */
  estimatedGain: number | null;
}

export interface NextActionList {
  items: NextAction[];
  /** How many were left out by `limit` — shown, never dropped silently. */
  remaining: number;
}

function ruleAction(imageRef: string, ruleId: string, weight: number, pack: RulePack | null): NextAction {
  const rule = pack?.rules.find((r) => r.id === ruleId);
  return {
    imageRef,
    kind: "rule",
    id: `${imageRef}::${ruleId}`,
    // Without the pack the checklist still knows what failed, just not how to
    // say it in words — the id is worse than a title but better than nothing.
    title: rule?.title ?? ruleId,
    detail: rule?.remediation ?? "",
    estimatedGain: weight,
  };
}

function alternativeAction(imageRef: string, alt: Alternative): NextAction {
  return {
    imageRef,
    kind: "alternative",
    id: `${imageRef}::alt::${alt.image}`,
    title: `${imageRef} → ${alt.image}`,
    detail: alt.why,
    estimatedGain: null,
  };
}

export function nextActions(
  audits: Record<string, SecurityAudit>,
  suggestions: Record<string, CatalogSuggestions>,
  pack: RulePack | null,
  limit = 8,
): NextActionList {
  const all: NextAction[] = [];

  // Sorted refs so an unchanged set of audits always produces the same order.
  for (const imageRef of Object.keys(audits).sort()) {
    for (const result of audits[imageRef].evaluation.results) {
      if (!result.passed) all.push(ruleAction(imageRef, result.ruleId, result.weight, pack));
    }
    for (const alt of suggestions[imageRef]?.alternatives ?? []) {
      all.push(alternativeAction(imageRef, alt));
    }
  }

  all.sort((a, b) => {
    // Unmeasurable gains sink below every measurable one rather than sorting as
    // zero, which would rank them against rules worth nothing.
    if (a.estimatedGain === null && b.estimatedGain === null) return 0;
    if (a.estimatedGain === null) return 1;
    if (b.estimatedGain === null) return -1;
    return b.estimatedGain - a.estimatedGain;
  });

  return { items: all.slice(0, limit), remaining: Math.max(0, all.length - limit) };
}

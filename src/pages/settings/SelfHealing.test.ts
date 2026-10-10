import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";

import SelfHealing from "./SelfHealing.svelte";
import type { HealRule } from "../../lib/api/self-heal";

/**
 * The rules fold away, but nothing that stops the feature folds with them.
 *
 * Five rules with four controls each is the longest thing on the Settings page.
 * Collapsing them is only safe if two things stay true: the master switch — the
 * control that stops every rule at once — is reachable without expanding
 * anything, and the summary still says how many rules can act on their own, so
 * folding hides detail rather than hiding state.
 */

function rule(over: Partial<HealRule> = {}): HealRule {
  return {
    id: 1,
    name: "Container is unhealthy",
    trigger: "unhealthy",
    action: "restart_container",
    mode: "suggest",
    threshold: 5,
    windowSecs: 300,
    maxPerHour: 3,
    enabled: true,
    autoCapable: true,
    ...over,
  };
}

/**
 * Rule names come from the locale, keyed by trigger — `rule.name` is only the
 * fallback for a trigger this build has no wording for. Asserting on the
 * fixture's name would pass even if the lookup broke.
 */
const CRASH_LOOP_LABEL = "Stop a container that is crash-looping";

const rules = [
  rule({ id: 1, mode: "auto" }),
  rule({ id: 2, name: "Crash loop", trigger: "crash_loop", mode: "auto" }),
  rule({ id: 3, name: "Disk full", trigger: "disk_full" }),
];

vi.mock("../../lib/api/self-heal", () => ({
  selfHealApi: {
    listRules: () => Promise.resolve(rules),
    recentLog: () => Promise.resolve([]),
    isEnabled: () => Promise.resolve(true),
    saveRule: () => Promise.resolve(),
    setEnabled: () => Promise.resolve(),
  },
}));

beforeEach(() => {
  render(SelfHealing);
});

afterEach(cleanup);

describe("SelfHealing", () => {
  it("keeps the master switch reachable while the rules are folded", async () => {
    expect(await screen.findByText(/Allow self-healing to act/i)).toBeTruthy();
    // Folded: no rule name on screen yet.
    expect(screen.queryByText(CRASH_LOOP_LABEL)).toBeNull();
  });

  it("says how many rules can act on their own before anything is expanded", async () => {
    expect(await screen.findByText(/3 rules/i)).toBeTruthy();
    expect(screen.getByText(/2 act on their own/i)).toBeTruthy();
  });

  it("shows the rules once expanded, and folds them again", async () => {
    const toggle = await screen.findByRole("button", { expanded: false });

    await fireEvent.click(toggle);
    expect(screen.getByText(CRASH_LOOP_LABEL)).toBeTruthy();

    await fireEvent.click(screen.getByRole("button", { expanded: true }));
    expect(screen.queryByText(CRASH_LOOP_LABEL)).toBeNull();
  });
});

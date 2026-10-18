import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";

/**
 * The editor's job is to stop somebody creating a rule they will end up turning
 * off: one that never fires, or one that pages them every tick. The backtest is
 * how they find out which they have made, so it has to reach the backend with
 * the rule as edited — not the rule as it was when the form opened.
 */

const { alertsApi } = vi.hoisted(() => ({
  alertsApi: {
    saveRule: vi.fn(),
    deleteRule: vi.fn(),
    backtest: vi.fn(),
    listRules: vi.fn(),
    recentEvents: vi.fn(),
  },
}));

vi.mock("../../lib/api/metrics", async () => {
  const actual = await vi.importActual<typeof import("../../lib/api/metrics")>(
    "../../lib/api/metrics"
  );
  return { ...actual, alertsApi };
});

import AlertRuleEditor from "./AlertRuleEditor.svelte";
import type { AlertRule } from "../../lib/api/metrics";

const rule: AlertRule = {
  id: 3,
  name: "web is pegged",
  metric: "cpu_pct",
  threshold: 80,
  durationSecs: 300,
  cooldownSecs: 900,
  enabled: true,
};

const containers = [{ id: "abc", name: "web" }];

beforeEach(() => {
  alertsApi.saveRule.mockResolvedValue(1);
  alertsApi.deleteRule.mockResolvedValue(undefined);
  alertsApi.backtest.mockResolvedValue({
    wouldFire: 11,
    fromMs: 0,
    toMs: 1,
    sampleCount: 5000,
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("AlertRuleEditor", () => {
  it("summarises a rule in the terms it was written in", () => {
    render(AlertRuleEditor, { rules: [rule], containers, onChanged: () => {} });
    expect(screen.getByText("web is pegged")).toBeInTheDocument();
    expect(screen.getByText(/CPU % > 80 · 5m · any container/)).toBeInTheDocument();
  });

  it("backtests the rule as currently edited", async () => {
    render(AlertRuleEditor, { rules: [], containers, onChanged: () => {} });

    await fireEvent.input(screen.getByPlaceholderText("web is pegged"), {
      target: { value: "db memory" },
    });
    await fireEvent.click(screen.getByText(/Test against last 7 days/));

    expect(alertsApi.backtest).toHaveBeenCalledTimes(1);
    const [sent, from, to] = alertsApi.backtest.mock.calls[0];
    expect(sent.name).toBe("db memory");
    // A seven-day window, which is what the button says it is.
    expect(to - from).toBe(7 * 86_400_000);
  });

  it("reports what the rule would have done, in counts", async () => {
    render(AlertRuleEditor, { rules: [], containers, onChanged: () => {} });
    await fireEvent.click(screen.getByText(/Test against last 7 days/));
    expect(await screen.findByText(/Would have fired 11 times/)).toBeInTheDocument();
  });

  it("says when there is no history to test against, rather than showing zero", async () => {
    // Zero would read as "this rule is quiet", which is the opposite of "we
    // have nothing to judge it by".
    alertsApi.backtest.mockResolvedValue({ wouldFire: 0, fromMs: 0, toMs: 0, sampleCount: 0 });
    render(AlertRuleEditor, { rules: [], containers, onChanged: () => {} });
    await fireEvent.click(screen.getByText(/Test against last 7 days/));
    expect(await screen.findByText(/No history stored for that window/)).toBeInTheDocument();
  });

  it("keeps minutes in the form and seconds on the wire", async () => {
    render(AlertRuleEditor, { rules: [], containers, onChanged: () => {} });
    await fireEvent.input(screen.getByPlaceholderText("web is pegged"), {
      target: { value: "any" },
    });
    await fireEvent.click(screen.getByText("Add rule"));

    const sent = alertsApi.saveRule.mock.calls[0][0];
    // Defaults: 5 minutes and 15 minutes, expressed the way the backend wants.
    expect(sent.durationSecs).toBe(300);
    expect(sent.cooldownSecs).toBe(900);
  });

  it("toggles a rule without opening it for editing", async () => {
    render(AlertRuleEditor, { rules: [rule], containers, onChanged: () => {} });
    await fireEvent.click(screen.getByText("Disable"));
    expect(alertsApi.saveRule).toHaveBeenCalledWith({ ...rule, enabled: false });
  });
});

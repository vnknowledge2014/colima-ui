import { describe, expect, it } from "vitest";
import {
  capToolOutput,
  isApprovalRequiredError,
  MAX_SEARCH_ROUNDS,
  normalizeToolCallKey,
} from "./agentCore";

describe("agentCore helpers (DeepSeek-Harness & Harness2 patterns)", () => {
  it("increases MAX_SEARCH_ROUNDS to 5 for multi-step diagnosis and remediation", () => {
    expect(MAX_SEARCH_ROUNDS).toBe(5);
  });

  it("capToolOutput preserves short outputs verbatim and head+tail prunes oversized outputs", () => {
    const shortText = "container nginx is running on port 8080";
    expect(capToolOutput(shortText, 200)).toBe(shortText);

    const head = "HEAD_MARKER_" + "A".repeat(120);
    const middle = "M".repeat(500);
    const tail = "B".repeat(40) + "_TAIL_MARKER";
    const oversized = head + middle + tail;

    const pruned = capToolOutput(oversized, 100);
    expect(pruned.startsWith("HEAD_MARKER_")).toBe(true);
    expect(pruned.endsWith("_TAIL_MARKER")).toBe(true);
    expect(pruned).toContain("chars omitted (head+tail pruned)");
    expect(pruned.length).toBeLessThan(oversized.length);
  });

  it("normalizeToolCallKey collapses whitespace and normalizes kind case", () => {
    expect(normalizeToolCallKey("run", "  docker   ps   -a  ")).toBe("RUN:docker ps -a:");
    expect(normalizeToolCallKey("query", "docker:containers", ' {"all":true} ')).toBe(
      'QUERY:docker:containers:{"all":true}',
    );
  });

  it("isApprovalRequiredError detects shell_sandbox NeedsApproval rejections", () => {
    expect(
      isApprovalRequiredError(
        new Error("Command 'colima' requires user approval. Use execute_approved_command after confirmation."),
      ),
    ).toBe(true);
    expect(isApprovalRequiredError("Command 'kubectl' requires user approval.")).toBe(true);
    expect(isApprovalRequiredError(new Error("Command contains disallowed shell operators"))).toBe(
      false,
    );
  });
});

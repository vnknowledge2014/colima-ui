/**
 * src/lib/aiToolStep.test.ts
 *
 * Vitest test suite for tool step transitions, serialization,
 * envelope parsing, and state machine validation.
 */

import { describe, it, expect } from "vitest";
import {
  createToolStep,
  isValidStatusTransition,
  transitionToolStep,
  serializeToolStep,
  parseToolStep,
  isToolStepContent,
  extractToolStepSummary,
  type ToolStepPayload,
} from "./aiToolStep";

describe("aiToolStep Data Structures & Transitions", () => {
  it("creates a tool step with version 1 and default timestamp", () => {
    const step = createToolStep({
      kind: "query",
      name: "list-containers",
      title: "List Running Containers",
      safety: "SAFE",
      status: "running",
      rawInput: '{"all": true}',
    });

    expect(step.version).toBe(1);
    expect(step.kind).toBe("query");
    expect(step.name).toBe("list-containers");
    expect(step.title).toBe("List Running Containers");
    expect(step.safety).toBe("SAFE");
    expect(step.status).toBe("running");
    expect(step.rawInput).toBe('{"all": true}');
    expect(typeof step.timestamp).toBe("number");
  });

  describe("isValidStatusTransition", () => {
    it("allows legal pending_approval transitions", () => {
      expect(isValidStatusTransition("pending_approval", "running")).toBe(true);
      expect(isValidStatusTransition("pending_approval", "denied")).toBe(true);
      expect(isValidStatusTransition("pending_approval", "failed")).toBe(true);
      expect(isValidStatusTransition("pending_approval", "succeeded")).toBe(false);
    });

    it("allows legal running transitions", () => {
      expect(isValidStatusTransition("running", "succeeded")).toBe(true);
      expect(isValidStatusTransition("running", "failed")).toBe(true);
      expect(isValidStatusTransition("running", "running")).toBe(true);
      expect(isValidStatusTransition("running", "pending_approval")).toBe(false);
      expect(isValidStatusTransition("running", "denied")).toBe(false);
    });

    it("enforces terminal state immutability", () => {
      expect(isValidStatusTransition("succeeded", "running")).toBe(false);
      expect(isValidStatusTransition("succeeded", "failed")).toBe(false);
      expect(isValidStatusTransition("denied", "running")).toBe(false);
      expect(isValidStatusTransition("failed", "running")).toBe(false);
    });
  });

  describe("transitionToolStep", () => {
    it("successfully transitions from pending_approval to running", () => {
      const initial = createToolStep({
        kind: "event",
        name: "stop-vm",
        title: "Stop Colima VM",
        safety: "NORMAL",
        status: "pending_approval",
      });

      const running = transitionToolStep(initial, "running");
      expect(running.status).toBe("running");
    });

    it("successfully transitions from running to succeeded with output", () => {
      const running = createToolStep({
        kind: "command",
        name: "prune",
        title: "Prune Docker Images",
        safety: "DANGEROUS",
        status: "running",
      });

      const succeeded = transitionToolStep(running, "succeeded", {
        output: "Total reclaimed space: 1.2GB",
        exitCode: 0,
      });

      expect(succeeded.status).toBe("succeeded");
      expect(succeeded.output).toBe("Total reclaimed space: 1.2GB");
      expect(succeeded.exitCode).toBe(0);
    });

    it("throws on illegal status transition", () => {
      const step = createToolStep({
        kind: "query",
        name: "test",
        title: "Test",
        safety: "SAFE",
        status: "succeeded",
      });

      expect(() => transitionToolStep(step, "running")).toThrow(
        /Invalid tool step status transition/
      );
    });
  });
});

describe("aiToolStep Serialization & Parsing", () => {
  it("serializes payload with comment envelope and readable markdown summary", () => {
    const payload: ToolStepPayload = {
      version: 1,
      id: "step-123",
      kind: "command",
      name: "docker-prune",
      title: "Prune Unused Containers",
      safety: "DANGEROUS",
      status: "pending_approval",
      rawInput: "docker container prune -f",
    };

    const serialized = serializeToolStep(payload);
    expect(serialized).toContain("<!--AI_TOOL_STEP:");
    expect(serialized).toContain('"version":1');
    expect(serialized).toContain('"status":"pending_approval"');
    expect(serialized).toContain("⏳ Action Required: **Prune Unused Containers**");
    expect(serialized).toContain("`docker container prune -f`");
  });

  it("parses valid serialized content back into a ToolStepPayload", () => {
    const original: ToolStepPayload = {
      version: 1,
      id: "step-456",
      kind: "event",
      name: "start-vm",
      title: "Start VM",
      safety: "NORMAL",
      status: "succeeded",
      output: "VM started in 3.2s",
      exitCode: 0,
    };

    const serialized = serializeToolStep(original);
    const parsed = parseToolStep(serialized);

    expect(parsed).not.toBeNull();
    expect(parsed?.version).toBe(1);
    expect(parsed?.id).toBe("step-456");
    expect(parsed?.name).toBe("start-vm");
    expect(parsed?.status).toBe("succeeded");
    expect(parsed?.output).toBe("VM started in 3.2s");
  });

  it("returns null for non-tool-step content", () => {
    expect(parseToolStep("Just normal assistant reply.")).toBeNull();
    expect(parseToolStep("<!--Some other comment-->")).toBeNull();
    expect(parseToolStep("")).toBeNull();
  });

  it("returns null on malformed JSON inside envelope", () => {
    const broken = "<!--AI_TOOL_STEP:{bad-json}-->\nSummary text";
    expect(parseToolStep(broken)).toBeNull();
  });

  it("returns null on schema validation failure", () => {
    // Missing required fields like title or invalid version
    const invalidSchema = '<!--AI_TOOL_STEP:{"version":2,"status":"running"}-->';
    expect(parseToolStep(invalidSchema)).toBeNull();
  });

  it("detects tool step presence with isToolStepContent", () => {
    const valid = '<!--AI_TOOL_STEP:{"version":1}-->';
    expect(isToolStepContent(valid)).toBe(true);
    expect(isToolStepContent("Plain text")).toBe(false);
  });

  it("extracts summary without envelope using extractToolStepSummary", () => {
    const payload = createToolStep({
      kind: "query",
      name: "status",
      title: "Check System Health",
      safety: "SAFE",
      status: "running",
    });

    const serialized = serializeToolStep(payload);
    const summary = extractToolStepSummary(serialized);

    expect(summary).toBe("⟳ Executing: **Check System Health**…");
    expect(summary).not.toContain("<!--AI_TOOL_STEP:");
  });
});

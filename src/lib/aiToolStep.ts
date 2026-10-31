/**
 * src/lib/aiToolStep.ts
 *
 * Data structures, state machine transitions, and envelope serialization
 * for interactive AI tool execution steps (R2).
 */

export type ToolStepStatus =
  | "pending_approval"
  | "running"
  | "succeeded"
  | "denied"
  | "failed";

export type ToolStepKind =
  | "query"
  | "event"
  | "command"
  | "diagnose"
  | "security"
  | "navigate"
  | "schedule";

export type ToolStepSafety = "SAFE" | "NORMAL" | "DANGEROUS";

export interface ToolStepPayload {
  version: 1;
  id?: string;
  kind: ToolStepKind;
  name: string;
  title: string;
  safety: ToolStepSafety;
  status: ToolStepStatus;
  rawInput?: string;
  output?: string;
  exitCode?: number;
  warning?: string;
  timestamp?: number;
}

export const TOOL_STEP_ENVELOPE_REGEX = /<!--AI_TOOL_STEP:([\s\S]*?)-->/;

const VALID_STATUSES = new Set<ToolStepStatus>([
  "pending_approval",
  "running",
  "succeeded",
  "denied",
  "failed",
]);

const VALID_KINDS = new Set<ToolStepKind>([
  "query",
  "event",
  "command",
  "diagnose",
  "security",
  "navigate",
  "schedule",
]);

const VALID_SAFETIES = new Set<ToolStepSafety>(["SAFE", "NORMAL", "DANGEROUS"]);

/**
 * Validates whether a state transition is legal according to the tool step state machine.
 */
export function isValidStatusTransition(
  current: ToolStepStatus,
  next: ToolStepStatus
): boolean {
  if (current === next) return true;

  switch (current) {
    case "pending_approval":
      return next === "running" || next === "denied" || next === "failed";
    case "running":
      return next === "succeeded" || next === "failed";
    case "succeeded":
    case "denied":
    case "failed":
      return false; // Terminal states cannot transition
    default:
      return false;
  }
}

/**
 * Creates a validated ToolStepPayload with version 1 and current timestamp.
 */
export function createToolStep(
  params: Omit<ToolStepPayload, "version"> & { version?: 1 }
): ToolStepPayload {
  return {
    version: 1,
    timestamp: params.timestamp ?? Date.now(),
    ...params,
  };
}

/**
 * Transitions a tool step to a new status, verifying legal state progression.
 * Throws an error on illegal state transitions.
 */
export function transitionToolStep(
  step: ToolStepPayload,
  nextStatus: ToolStepStatus,
  updates?: Partial<Omit<ToolStepPayload, "version" | "status">>
): ToolStepPayload {
  if (!isValidStatusTransition(step.status, nextStatus)) {
    throw new Error(
      `Invalid tool step status transition: cannot transition from '${step.status}' to '${nextStatus}'`
    );
  }

  return {
    ...step,
    ...updates,
    status: nextStatus,
    timestamp: updates?.timestamp ?? Date.now(),
  };
}

function truncateText(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  return text.slice(0, maxLen - 1) + "…";
}

/**
 * Generates human-readable markdown fallback text representing the tool step.
 */
export function generateToolStepSummary(payload: ToolStepPayload): string {
  switch (payload.status) {
    case "pending_approval":
      return `⏳ Action Required: **${payload.title}**${payload.rawInput ? `\n\`${payload.rawInput}\`` : ""}`;
    case "running":
      return `⟳ Executing: **${payload.title}**…`;
    case "succeeded":
      return `✓ Completed: **${payload.title}**${
        payload.output ? `\n\`\`\`\n${truncateText(payload.output, 200)}\n\`\`\`` : ""
      }`;
    case "denied":
      return `✕ Action Denied: **${payload.title}**`;
    case "failed":
      return `✕ Failed: **${payload.title}**${
        payload.warning ? ` — ${payload.warning}` : payload.output ? `\n\`\`\`\n${truncateText(payload.output, 150)}\n\`\`\`` : ""
      }`;
    default:
      return `**${payload.title}** (${payload.status})`;
  }
}

/**
 * Serializes a tool step into an HTML comment envelope followed by clean markdown summary text.
 */
export function serializeToolStep(payload: ToolStepPayload): string {
  // Unicode-escape '-->' to '--\u003e' so HTML comment delimiters cannot appear inside JSON
  const safeJson = JSON.stringify(payload).replace(/-->/g, "--\\u003e");
  const jsonEnvelope = `<!--AI_TOOL_STEP:${safeJson}-->`;
  const summary = generateToolStepSummary(payload);
  return `${jsonEnvelope}\n${summary}`;
}

function isValidToolStepPayload(obj: unknown): obj is ToolStepPayload {
  if (!obj || typeof obj !== "object") return false;
  const p = obj as Record<string, unknown>;
  return (
    p.version === 1 &&
    typeof p.name === "string" &&
    p.name.length > 0 &&
    typeof p.title === "string" &&
    p.title.length > 0 &&
    VALID_STATUSES.has(p.status as ToolStepStatus) &&
    VALID_SAFETIES.has(p.safety as ToolStepSafety) &&
    VALID_KINDS.has(p.kind as ToolStepKind)
  );
}

/**
 * Extracts and validates a ToolStepPayload from a message content string.
 * Returns null if no envelope is present or if JSON/payload is malformed.
 */
export function parseToolStep(content: string): ToolStepPayload | null {
  if (!content || typeof content !== "string") return null;

  const match = content.match(TOOL_STEP_ENVELOPE_REGEX);
  if (!match || !match[1]) return null;

  try {
    const raw = JSON.parse(match[1]);
    if (!isValidToolStepPayload(raw)) {
      return null;
    }
    return raw;
  } catch {
    return null;
  }
}

/**
 * Fast check whether a message contains a tool step envelope.
 */
export function isToolStepContent(content: string): boolean {
  if (!content || typeof content !== "string") return false;
  return TOOL_STEP_ENVELOPE_REGEX.test(content);
}

/**
 * Extracts the clean summary text by stripping the tool step comment envelope.
 */
export function extractToolStepSummary(content: string): string {
  if (!content || typeof content !== "string") return "";
  return content.replace(TOOL_STEP_ENVELOPE_REGEX, "").trim();
}

/**
 * Formats an internal event or tool identifier (e.g. `list-containers`, `docker:start`)
 * into a concise human-readable card title (`List Containers`, `Docker: Start`).
 */
export function humanizeToolName(rawName: string): string {
  const cleaned = rawName.trim();
  if (!cleaned) return "Tool Action";
  return cleaned
    .split(/([-:_/\s]+)/)
    .map((segment) => {
      if (/^[-:_/\s]+$/.test(segment)) {
        return segment.includes(":") ? ": " : " ";
      }
      if (segment.toLowerCase() === "k8s") return "K8s";
      if (segment.toLowerCase() === "vm" || segment.toLowerCase() === "vms") return segment.toUpperCase();
      if (segment.toLowerCase() === "cli") return "CLI";
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    })
    .join("")
    .replace(/\s+/g, " ")
    .trim();
}

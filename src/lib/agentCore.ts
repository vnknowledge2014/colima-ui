import { colimaApi, dockerApi, composeApi, k8sApi, sysMethods, knowledgeBankApi, sandboxApi, aiApi } from "./api";
import type { ChatMessage } from "./api";
import { chatStream } from "./llmProviders";
import { newId } from "./ids";
import { getCategory, executeEvent } from "./aiEventBus";
import { BUILT_IN_PRESETS } from "./presetStateManager";
import { parseAiTools, stripStreamingPartialTags, rebuildAssistantTurnContent } from "./aiToolParser";
import { findProvider } from "./aiProviderConfig";
import {
  createToolStep,
  transitionToolStep,
  serializeToolStep,
  humanizeToolName,
  type ToolStepSafety,
} from "./aiToolStep";

export interface AgentCallbacks {
  onStatus: (text: string) => void;
  onMessage: (msg: { id: string; role: string; content: string }) => void;
  onMessageUpdate: (id: string, content: string) => void;
  onMessageDelete: (id: string) => void;
  onApprovalRequired: (id: string, command: string) => Promise<boolean>;
  onNavigate: (page: string) => void;
  onTimerSchedule: (id: string, ms: number, prompt: string) => void;
  onCronSchedule: (id: string, expr: string, prompt: string) => void;
  onScheduleCancel: (id: string) => void;
}

export const MAX_SEARCH_ROUNDS = 5;
export const DEFAULT_TOOL_OUTPUT_MAX_CHARS = 5120;

/**
 * Head + tail tool output pruner (DeepSeek-Harness & Harness2 pattern).
 * Preserves the first 80% (headers/context) and last 20% (final exit summary/errors)
 * when a tool output exceeds `maxChars`.
 */
export function capToolOutput(
  text: string,
  maxChars: number = DEFAULT_TOOL_OUTPUT_MAX_CHARS,
): string {
  if (!text || text.length <= maxChars) return text;
  const headChars = Math.floor(maxChars * 0.8);
  const tailChars = Math.max(0, maxChars - headChars);
  const omitted = text.length - headChars - tailChars;
  return `${text.slice(0, headChars)}\n\n[... ${omitted} chars omitted (head+tail pruned) ...]\n\n${text.slice(text.length - tailChars)}`;
}

function canonicalizeJsonValue(val: unknown): unknown {
  if (Array.isArray(val)) {
    return val.map(canonicalizeJsonValue);
  }
  if (val !== null && typeof val === "object") {
    const obj = val as Record<string, unknown>;
    const sorted: Record<string, unknown> = {};
    for (const k of Object.keys(obj).sort()) {
      sorted[k] = canonicalizeJsonValue(obj[k]);
    }
    return sorted;
  }
  return val;
}

/**
 * Normalize a tool invocation key for cross-round repeat detection
 * (`repeat-tool-reminder` guard pattern from `deepseek-harness`).
 */
export function normalizeToolCallKey(kind: string, target: string, payload = ""): string {
  let normalizedPayload = payload.trim();
  if (normalizedPayload) {
    try {
      const parsed: unknown = JSON.parse(normalizedPayload);
      normalizedPayload = JSON.stringify(canonicalizeJsonValue(parsed));
    } catch {
      normalizedPayload = normalizedPayload.replace(/\s+/g, " ");
    }
  }
  return `${kind.toUpperCase()}:${target.trim().replace(/\s+/g, " ")}:${normalizedPayload}`;
}

/**
 * Detect whether a `sandboxApi.execute` error came from `shell_sandbox::classify`
 * returning `Tier::NeedsApproval` (`"Command '<bin>' requires user approval"`).
 */
export function isApprovalRequiredError(err: unknown): boolean {
  const msg = typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
  return msg.toLowerCase().includes("requires user approval");
}

export async function runAgent(
  userMessage: string,
  config: { provider: string; model: string; apiKey: string; endpoint: string },
  callbacks: AgentCallbacks,
  chatHistory: ChatMessage[],
  appContext: string,
  customPresets: unknown,
  signal?: AbortSignal,
): Promise<void> {
  const { provider, model, apiKey, endpoint } = config;

  const aborted = () => signal?.aborted === true;

  // Driven by the provider registry rather than a hardcoded id list, so a
  // provider added there is guarded correctly without editing this function.
  const spec = findProvider(provider);

  if (!model) {
    callbacks.onMessage({ id: newId(), role: "system", content: "⚠️ No AI model selected. Go to **Settings → AI Provider** and pick a model." });
    return;
  }
  if (spec?.needsEndpoint && !endpoint) {
    callbacks.onMessage({ id: newId(), role: "system", content: `⚠️ ${spec.label} requires an **Endpoint URL**. Go to **Settings → AI Provider** and set it (e.g. \`${spec.endpointPlaceholder ?? "https://your-host.com"}\`).` });
    return;
  }
  // A self-hosted gateway on localhost usually has no credential at all, so a
  // key is only required when the request is going to the vendor's own API.
  const keyRequired = spec ? spec.needsKey && !(spec.needsEndpoint && endpoint) : true;
  if (keyRequired && !apiKey) {
    callbacks.onMessage({ id: newId(), role: "system", content: "⚠️ API Key not configured. Go to **Settings → AI Provider** to add your API key." });
    return;
  }
  // ─────────────────────────────────────────────────────────────────────────

  let kbContext = "";
  try {
    const kbResult = await knowledgeBankApi.query(userMessage);
    if (kbResult.context_text) kbContext = kbResult.context_text;
  } catch {
    // Knowledge Bank is best-effort; a missing/unavailable backend must not block the agent.
  }

  let memoryContext = "";
  try {
    const memResult = await knowledgeBankApi.searchMemory(userMessage, 5);
    if (memResult && memResult.length > 0) {
      memoryContext = memResult.join("\n- ");
    }
  } catch {
    // Memory retrieval failure is non-fatal — the agent can still run without it.
  }

  const fullHistory: ChatMessage[] = [
    ...(appContext ? [{ role: "system" as const, content: appContext }] : []),
    ...(kbContext ? [{ role: "system" as const, content: `[Knowledge Bank]\n${kbContext}` }] : []),
    ...(memoryContext ? [{ role: "system" as const, content: `[USER_MEMORY]\n- ${memoryContext}` }] : []),
    ...chatHistory.filter(m => m.role !== "system").slice(-10).map(m => ({ role: m.role as "user" | "assistant", content: m.content })),
    { role: "user", content: userMessage },
  ];

  const seenToolCalls = new Set<string>();

  try {
    for (let round = 0; round < MAX_SEARCH_ROUNDS; round++) {
      // Stopping mid-run must not start the next reasoning round — the agent
      // loop is the expensive part, and a cancelled request that keeps looping
      // still burns tokens and can still execute tools.
      if (aborted()) break;
      callbacks.onStatus(round === 0 ? "Agent is reasoning..." : `Agent Loop ${round + 1}...`);

      let responseText = "";
      const streamId = newId();
      callbacks.onMessage({ id: streamId, role: "assistant", content: "" });

      await chatStream(provider, model, apiKey, fullHistory, endpoint, (chunk) => {
        responseText += chunk;
        const { cleanText } = stripStreamingPartialTags(responseText);
        callbacks.onMessageUpdate(streamId, cleanText);
      }, signal);

      // Keep whatever streamed in before the stop, but do not run the tools the
      // half-finished response may have asked for.
      if (aborted()) {
        if (!responseText) callbacks.onMessageDelete(streamId);
        break;
      }

      // Clean tool calls from UI text
      const parsed = parseAiTools(responseText);
      const {
        cleanText, hasTools, queries, eventApprovals,
        runs, runApprovals, learns, learnReasoning, prefs,
        hasDiagnose, hasQueryState,
        navigates, readReferences, secThreatModels, secVulnScans,
        secTriages, secPatchGens, schedCrons, schedTimers, schedCancels,
        suggests, choicePrompt,
      } = parsed;

      const hasInteractiveContent = (suggests && suggests.length > 0) || Boolean(choicePrompt);

      if (!cleanText && !hasInteractiveContent) {
        callbacks.onMessageDelete(streamId);
      } else {
        const turnContent = rebuildAssistantTurnContent(cleanText, suggests, choicePrompt);
        callbacks.onMessageUpdate(streamId, turnContent);
      }

      if (!hasTools) {
        break;
      }

      let toolContext = "";

      // 1. [QUERY: eventName | payload]
      for (const [, eventName, payload] of queries) {
        if (aborted()) break;
        const evt = eventName.trim();
        const pld = payload ? payload.trim() : "";
        const callKey = normalizeToolCallKey("QUERY", evt, pld);
        if (seenToolCalls.has(callKey)) {
          toolContext += `\n[SYSTEM REMINDER: You already executed [QUERY: ${evt}] with the same payload in an earlier round. Use the previous result above or try a different approach.]\n`;
          continue;
        }
        const category = getCategory(evt);
        
        if (category !== "SAFE") {
          toolContext += `\n(Query denied: ${evt} is not SAFE. Use [EVENT_APPROVE] instead.)\n`;
          continue;
        }
        
        callbacks.onStatus(`🔍 Querying ${evt}...`);
        const stepId = newId();
        let step = createToolStep({
          id: stepId,
          kind: "query",
          name: evt,
          title: humanizeToolName(evt),
          safety: "SAFE",
          status: "running",
          rawInput: pld || undefined,
        });
        callbacks.onMessage({ id: stepId, role: "system", content: serializeToolStep(step) });
        try {
          const parsedPayload = pld ? JSON.parse(pld) : {};
          const result = await executeEvent(evt, parsedPayload);
          seenToolCalls.add(callKey);
          const capped = capToolOutput(String(result));
          toolContext += `\n### Query: ${evt}\n${capped}\n`;
          step = transitionToolStep(step, "succeeded", { output: capToolOutput(String(result), 1200) });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        } catch (e) {
          toolContext += `\n(Query failed: ${e})\n`;
          step = transitionToolStep(step, "failed", { warning: String(e) });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        }
      }

      // 2. [EVENT_APPROVE: eventName | payload]
      for (const [, eventName, payload] of eventApprovals.slice(0, 2)) {
        if (aborted()) break;
        const evt = eventName.trim();
        const pld = payload ? payload.trim() : "";
        const category = getCategory(evt);
        let parsedPayload = {};
        try {
          parsedPayload = pld ? JSON.parse(pld) : {};
        } catch {
          // A malformed payload just means no arguments for this event.
        }

        if (category === "SAFE") {
          callbacks.onStatus(`✅ Auto-executing safe event ${evt}...`);
          const stepId = newId();
          let step = createToolStep({
            id: stepId,
            kind: "event",
            name: evt,
            title: humanizeToolName(evt),
            safety: "SAFE",
            status: "running",
            rawInput: pld || undefined,
          });
          callbacks.onMessage({ id: stepId, role: "system", content: serializeToolStep(step) });
          try {
            const result = await executeEvent(evt, parsedPayload);
            const capped = capToolOutput(String(result));
            toolContext += `\n### Event: ${evt}\n${capped}\n`;
            step = transitionToolStep(step, "succeeded", { output: capToolOutput(String(result), 1200) });
            callbacks.onMessageUpdate(stepId, serializeToolStep(step));
          } catch (e) {
            toolContext += `\n(Event failed: ${e})\n`;
            step = transitionToolStep(step, "failed", { warning: String(e) });
            callbacks.onMessageUpdate(stepId, serializeToolStep(step));
          }
        } else {
          callbacks.onStatus(`⏳ Awaiting approval: ${evt}`);
          const approvalId = newId();
          const safety: ToolStepSafety = category === "DANGEROUS" ? "DANGEROUS" : "NORMAL";
          let step = createToolStep({
            id: approvalId,
            kind: "event",
            name: evt,
            title: humanizeToolName(evt),
            safety,
            status: "pending_approval",
            rawInput: pld || undefined,
            warning: safety === "DANGEROUS"
              ? "Destructive or irreversible state change — confirm before proceeding."
              : undefined,
          });
          callbacks.onMessage({ id: approvalId, role: "system", content: serializeToolStep(step) });
          
          const approved = await callbacks.onApprovalRequired(approvalId, `[EVENT_APPROVE: ${evt} | ${pld}]`);
          if (aborted()) break;

          if (approved) {
            step = transitionToolStep(step, "running");
            callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
            try {
              const result = await executeEvent(evt, parsedPayload);
              // State-mutating action succeeded: invalidate cached read-only tool calls
              seenToolCalls.clear();
              const capped = capToolOutput(String(result));
              toolContext += `\n\n### Approved Event: \`${evt}\`\nResult: ${capped}\n`;
              step = transitionToolStep(step, "succeeded", { output: capToolOutput(String(result), 1200) });
              callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
            } catch (e) {
              toolContext += `\n(Event execution failed: ${e})\n`;
              step = transitionToolStep(step, "failed", { warning: String(e) });
              callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
            }
          } else {
            toolContext += `\n\n### Event DENIED by user: \`${evt}\`\n`;
            step = transitionToolStep(step, "denied");
            callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          }
        }
      }

      // 3. System state, diagnostics, commands & navigation
      if (hasQueryState && !aborted()) {
        callbacks.onStatus("🔍 Reading live system state...");
        const stepId = newId();
        let step = createToolStep({
          id: stepId,
          kind: "query",
          name: "query-app-state",
          title: "Read Live System State",
          safety: "SAFE",
          status: "running",
        });
        callbacks.onMessage({ id: stepId, role: "system", content: serializeToolStep(step) });
        try {
          const hostSpecsPromise = sysMethods.hostSpecs();
          const [instances, containers, composeProjects, k8sContext, hostSpecs] = 
            await Promise.allSettled([
              colimaApi.listInstances(),
              dockerApi.listContainers(),
              composeApi.list(),
              k8sApi.currentContext(),
              hostSpecsPromise,
            ]);
          
          const instanceList = instances.status === "fulfilled" ? instances.value : [];
          const containerList = (containers.status === "fulfilled" ? containers.value : [])
            .map((c) => ({id: c.Id?.slice(0,12), name: c.Names, image: c.Image, status: c.Status}));
          const composeList = composeProjects.status === "fulfilled" ? composeProjects.value : [];
          const k8sCtx = k8sContext.status === "fulfilled" ? k8sContext.value : "none";

          toolContext += `\n### Live System State
Host: ${JSON.stringify(hostSpecs.status === "fulfilled" ? hostSpecs.value : "unknown")}
Instances: ${JSON.stringify(instanceList)}
Containers (summary): ${JSON.stringify(containerList)}
Compose Projects: ${JSON.stringify(composeList)}
K8s Context: ${k8sCtx}
Custom Presets: ${JSON.stringify(customPresets)}
Built-in Presets: ${JSON.stringify(BUILT_IN_PRESETS)}
`;
          step = transitionToolStep(step, "succeeded", {
            output: `Instances: ${instanceList.length}, Containers: ${containerList.length}, Compose: ${composeList.length}, K8s: ${k8sCtx}`,
          });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        } catch (e) {
          toolContext += `\n(Query app state failed: ${e})\n`;
          step = transitionToolStep(step, "failed", { warning: String(e) });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        }
      }

      if (hasDiagnose && !aborted()) {
        callbacks.onStatus("🔬 Collecting diagnostics...");
        const stepId = newId();
        let step = createToolStep({
          id: stepId,
          kind: "diagnose",
          name: "diagnose",
          title: "Collect System Diagnostics",
          safety: "SAFE",
          status: "running",
        });
        callbacks.onMessage({ id: stepId, role: "system", content: serializeToolStep(step) });
        try {
          const diagReport = await knowledgeBankApi.collectDiagnosticLogs("default");
          toolContext += `\n\n### Diagnostic Report\n${capToolOutput(diagReport)}\n`;
          step = transitionToolStep(step, "succeeded", { output: capToolOutput(diagReport, 1200) });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        } catch (e) {
          toolContext += `\n(Diagnostic failed: ${e})\n`;
          step = transitionToolStep(step, "failed", { warning: String(e) });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        }
      }

      for (const [, cmd] of runs.slice(0, 3)) {
        if (aborted()) break;
        const trimmedCmd = cmd.trim();
        const callKey = normalizeToolCallKey("RUN", trimmedCmd);
        if (seenToolCalls.has(callKey)) {
          toolContext += `\n[SYSTEM REMINDER: You already executed [RUN: ${trimmedCmd}] in an earlier round. Use the previous output above or try a different command.]\n`;
          continue;
        }
        callbacks.onStatus(`🔧 Executing: ${trimmedCmd}`);
        const stepId = newId();
        let step = createToolStep({
          id: stepId,
          kind: "command",
          name: "sandbox-exec",
          title: "Execute Command",
          safety: "SAFE",
          status: "running",
          rawInput: trimmedCmd,
        });
        callbacks.onMessage({ id: stepId, role: "system", content: serializeToolStep(step) });
        try {
          const r = await sandboxApi.execute(cmd);
          seenToolCalls.add(callKey);
          const out = capToolOutput(r.stdout || r.stderr || "(no output)");
          toolContext += `\n\n### Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${out}\n\`\`\`\n`;
          step = transitionToolStep(step, r.exit_code === 0 ? "succeeded" : "failed", {
            output: capToolOutput(r.stdout || r.stderr || "(no output)", 1200),
            exitCode: r.exit_code,
          });
          callbacks.onMessageUpdate(stepId, serializeToolStep(step));
        } catch (e) {
          if (isApprovalRequiredError(e)) {
            callbacks.onStatus(`⏳ Awaiting approval: ${trimmedCmd}`);
            step = createToolStep({
              id: stepId,
              kind: "command",
              name: "sandbox-exec",
              title: "Command Approval Required",
              safety: "NORMAL",
              status: "pending_approval",
              rawInput: trimmedCmd,
              warning: "Command requires user approval before execution.",
            });
            callbacks.onMessageUpdate(stepId, serializeToolStep(step));
            const approved = await callbacks.onApprovalRequired(stepId, cmd);
            if (aborted()) break;
            if (approved) {
              step = transitionToolStep(step, "running");
              callbacks.onMessageUpdate(stepId, serializeToolStep(step));
              try {
                const r = await sandboxApi.executeApproved(cmd);
                // State-mutating command succeeded: invalidate cached read-only tool calls
                seenToolCalls.clear();
                const out = capToolOutput(r.stdout || r.stderr || "(no output)");
                toolContext += `\n\n### Approved Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${out}\n\`\`\`\n`;
                step = transitionToolStep(step, r.exit_code === 0 ? "succeeded" : "failed", {
                  output: capToolOutput(r.stdout || r.stderr || "(no output)", 1200),
                  exitCode: r.exit_code,
                });
                callbacks.onMessageUpdate(stepId, serializeToolStep(step));
              } catch (approvedErr) {
                toolContext += `\n(Approved command failed: ${approvedErr})\n`;
                step = transitionToolStep(step, "failed", { warning: String(approvedErr) });
                callbacks.onMessageUpdate(stepId, serializeToolStep(step));
              }
            } else {
              toolContext += `\n\n### Command DENIED by user: \`${cmd}\`\n`;
              step = transitionToolStep(step, "denied");
              callbacks.onMessageUpdate(stepId, serializeToolStep(step));
            }
          } else {
            toolContext += `\n(Command failed: ${e})\n`;
            step = transitionToolStep(step, "failed", { warning: String(e) });
            callbacks.onMessageUpdate(stepId, serializeToolStep(step));
          }
        }
      }

      for (const [, cmd] of runApprovals.slice(0, 2)) {
        if (aborted()) break;
        const trimmedCmd = cmd.trim();
        callbacks.onStatus(`⏳ Awaiting approval: ${trimmedCmd}`);
        const approvalId = newId();
        let step = createToolStep({
          id: approvalId,
          kind: "command",
          name: "run-approve",
          title: "Execute Terminal Command",
          safety: "DANGEROUS",
          status: "pending_approval",
          rawInput: trimmedCmd,
          warning: "Review this shell command carefully before approving execution.",
        });
        callbacks.onMessage({ id: approvalId, role: "system", content: serializeToolStep(step) });
        const approved = await callbacks.onApprovalRequired(approvalId, cmd);
        if (aborted()) break;
        if (approved) {
          step = transitionToolStep(step, "running");
          callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          try {
            const r = await sandboxApi.executeApproved(cmd);
            // State-mutating command succeeded: invalidate cached read-only tool calls
            seenToolCalls.clear();
            const out = capToolOutput(r.stdout || r.stderr || "(no output)");
            toolContext += `\n\n### Approved Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${out}\n\`\`\`\n`;
            step = transitionToolStep(step, r.exit_code === 0 ? "succeeded" : "failed", {
              output: capToolOutput(r.stdout || r.stderr || "(no output)", 1200),
              exitCode: r.exit_code,
            });
            callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          } catch (e) {
            toolContext += `\n(Approved command failed: ${e})\n`;
            step = transitionToolStep(step, "failed", { warning: String(e) });
            callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          }
        } else {
          toolContext += `\n\n### Command DENIED by user: \`${cmd}\`\n`;
          step = transitionToolStep(step, "denied");
          callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
        }
      }

      const securityTasks = [
        ...secThreatModels.map(([, dir, mode]) => ({ tool: "THREAT_MODEL", cmd: "omni", args: ["run", "security-threat-model", dir.trim(), mode ? mode.trim() : ""].filter(Boolean), dangerous: false })),
        ...secVulnScans.map(([, dir]) => ({ tool: "VULN_SCAN", cmd: "omni", args: ["run", "security-vuln-scan", dir.trim()], dangerous: false })),
        ...secTriages.map(([, path]) => ({ tool: "TRIAGE", cmd: "omni", args: ["run", "security-triage", path.trim()], dangerous: false })),
        ...secPatchGens.map(([, path, repo]) => ({ tool: "PATCH_GEN", cmd: "omni", args: ["run", "security-patch-gen", path.trim(), ...(repo ? [repo.trim()] : [])], dangerous: true })),
      ];

      for (const task of securityTasks) {
        if (aborted()) break;
        callbacks.onStatus(`⏳ Awaiting approval: Security ${task.tool}`);
        const approvalId = newId();
        const cmdStr = `${task.cmd} ${task.args.join(" ")}`;
        let step = createToolStep({
          id: approvalId,
          kind: "security",
          name: `security-${task.tool.toLowerCase()}`,
          title: `Security: ${humanizeToolName(task.tool.toLowerCase())}`,
          safety: task.dangerous ? "DANGEROUS" : "NORMAL",
          status: "pending_approval",
          rawInput: cmdStr,
          warning: task.dangerous
            ? "Generates candidate code patches — review diffs carefully before applying."
            : undefined,
        });
        callbacks.onMessage({ id: approvalId, role: "system", content: serializeToolStep(step) });
        
        const approved = await callbacks.onApprovalRequired(approvalId, `[EVENT_APPROVE: cli-exec | {"command":"${task.cmd}","args":${JSON.stringify(task.args)}}]`);
        if (aborted()) break;

        if (approved) {
          step = transitionToolStep(step, "running");
          callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          try {
            const result = await executeEvent("cli-exec", { command: task.cmd, args: task.args });
            toolContext += `\n\n### Security ${task.tool}\nResult: ${result}\n`;
            step = transitionToolStep(step, "succeeded", { output: capToolOutput(String(result), 1200) });
            callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          } catch (e) {
            toolContext += `\n(Execution failed: ${e})\n`;
            step = transitionToolStep(step, "failed", { warning: String(e) });
            callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
          }
        } else {
          toolContext += `\n(User denied Security ${task.tool})\n`;
          step = transitionToolStep(step, "denied");
          callbacks.onMessageUpdate(approvalId, serializeToolStep(step));
        }
      }

      if (aborted()) break;

      for (const [, expr, prompt] of schedCrons.slice(0, 1)) {
        const id = newId("cron");
        callbacks.onCronSchedule(id, expr, prompt);
        toolContext += `\n(Scheduled cron job: ${id} with expr ${expr})\n`;
      }

      for (const [, seconds, prompt] of schedTimers.slice(0, 2)) {
        const id = newId("timer");
        const parsedSecs = Math.min(Math.max(parseInt(seconds, 10) || 1, 1), 86400);
        const ms = parsedSecs * 1000;
        callbacks.onTimerSchedule(id, ms, prompt);
        toolContext += `\n(Scheduled timer: ${id} for ${parsedSecs}s)\n`;
      }

      for (const [, id] of schedCancels) {
        const timerId = id.trim();
        callbacks.onScheduleCancel(timerId);
        toolContext += `\n(Attempted to cancel schedule: ${timerId})\n`;
      }

      for (const [, tab] of navigates) {
        const page = tab.toLowerCase().trim();
        toolContext += `\n(Navigated to ${page})\n`;
        const stepId = newId();
        const step = createToolStep({
          id: stepId,
          kind: "navigate",
          name: "navigate",
          title: `Navigated to ${humanizeToolName(page)}`,
          safety: "SAFE",
          status: "succeeded",
          rawInput: page,
        });
        callbacks.onMessage({ id: stepId, role: "system", content: serializeToolStep(step) });
        callbacks.onNavigate(page);
      }

      for (const [, path] of readReferences.slice(0, 3)) {
        if (aborted()) break;
        callbacks.onStatus(`📚 Reading reference: ${path}`);
        try {
          const refContent = await aiApi.readReference(path.trim());
          toolContext += `\n\n### Reference: ${path}\n${refContent}\n`;
        } catch {
          // readReference endpoint may not be implemented yet — skip gracefully
          toolContext += `\n(Reference ${path} unavailable)\n`;
        }
      }

      for (const [, raw] of learns.slice(0, 3)) {
        if (aborted()) break;
        const trimmed = raw.trim();
        if (!trimmed) continue;
        try {
          const pipeIdx = trimmed.indexOf("|");
          if (pipeIdx !== -1) {
            const pattern = trimmed.slice(0, pipeIdx).trim();
            const solution = trimmed.slice(pipeIdx + 1).trim();
            if (pattern && solution) {
              await knowledgeBankApi.learn(pattern, solution);
            } else {
              await knowledgeBankApi.addMemory("learned", trimmed);
            }
          } else {
            await knowledgeBankApi.addMemory("learned", trimmed);
          }
        } catch {
          // Knowledge Bank persistence is best-effort
        }
      }

      for (const [, raw] of learnReasoning.slice(0, 3)) {
        if (aborted()) break;
        const trimmed = raw.trim();
        if (!trimmed) continue;
        try {
          await knowledgeBankApi.addMemory("reasoning", trimmed);
          const pipeIdx = trimmed.indexOf("|");
          if (pipeIdx !== -1) {
            const problem = trimmed.slice(0, pipeIdx).trim();
            const solution = trimmed.slice(pipeIdx + 1).trim();
            if (problem && solution) {
              await knowledgeBankApi.learn(problem, solution);
            }
          }
        } catch {
          // Knowledge Bank persistence is best-effort
        }
      }

      for (const [, raw] of prefs.slice(0, 3)) {
        if (aborted()) break;
        const trimmed = raw.trim();
        if (!trimmed) continue;
        try {
          await knowledgeBankApi.addMemory("preference", trimmed);
        } catch {
          // Knowledge Bank persistence is best-effort
        }
      }

      if (aborted()) break;

      // Side-effect-only memory tags ([LEARN], [LEARN_REASONING], [REMEMBER_PREFERENCE])
      // do not produce tool output requiring another LLM reasoning round.
      if (!toolContext.trim()) {
        break;
      }

      fullHistory.push(
        { role: "assistant", content: responseText },
        { role: "user", content: `[Tool Results]\n${toolContext}\n\nContinue reasoning.` }
      );
    }
  } catch (e) {
    // An aborted fetch rejects — that is the user pressing Stop, not a failure
    // worth reporting as an assistant message.
    const isAbort = aborted() || (e instanceof DOMException && e.name === "AbortError");
    if (!isAbort) {
      callbacks.onMessage({ id: newId(), role: "assistant", content: `Error: ${e}` });
    }
  } finally {
    callbacks.onStatus("");
  }
}

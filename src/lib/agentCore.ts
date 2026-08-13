import { colimaApi, dockerApi, composeApi, k8sApi, sysMethods, knowledgeBankApi, sandboxApi, aiApi } from "./api";
import type { ChatMessage } from "./api";
import { chatStream } from "./llmProviders";
import { newId } from "./ids";
import { getCategory, executeEvent } from "./aiEventBus";
import { BUILT_IN_PRESETS } from "./presetStateManager";
import { parseAiTools } from "./aiToolParser";

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

export const MAX_SEARCH_ROUNDS = 3;

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

  if (!apiKey && provider !== "ollama-local") {
    callbacks.onMessage({ id: newId(), role: "system", content: "⚠️ API Key not configured." });
    return;
  }
  if (!model) {
    callbacks.onMessage({ id: Date.now().toString(), role: "system", content: "⚠️ No AI model selected. Go to **Settings → AI & Diagnostics** and pick a model." });
    return;
  }
  if (!apiKey && provider !== "ollama-local" && provider !== "ollama-cloud") {
    callbacks.onMessage({ id: Date.now().toString(), role: "system", content: "⚠️ API Key not configured. Go to **Settings → AI & Diagnostics** to add your API key." });
    return;
  }
  if ((provider === "ollama-cloud") && !endpoint) {
    callbacks.onMessage({ id: Date.now().toString(), role: "system", content: "⚠️ Ollama Cloud requires an **Endpoint URL**. Go to **Settings → AI & Diagnostics** and set the endpoint (e.g. `https://your-ollama-host.com`)." });
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
        callbacks.onMessageUpdate(streamId, responseText);
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
        runs, runApprovals, hasDiagnose, hasQueryState,
        navigates, readReferences, secThreatModels, secVulnScans,
        secTriages, secPatchGens, schedCrons, schedTimers, schedCancels
      } = parsed;

      if (!cleanText) {
        callbacks.onMessageDelete(streamId);
      } else {
        callbacks.onMessageUpdate(streamId, cleanText);
      }

      if (!hasTools) {
        // If there were no tools, the response is final
        callbacks.onMessageUpdate(streamId, responseText); // Need to save original text for history (if necessary, depending on UI)
        break;
      }

      let toolContext = "";

      // 1. [QUERY: eventName | payload]
      for (const [, eventName, payload] of queries) {
        const evt = eventName.trim();
        const pld = payload ? payload.trim() : "";
        const category = getCategory(evt);
        
        if (category !== "SAFE") {
          toolContext += `\n(Query denied: ${evt} is not SAFE. Use [EVENT_APPROVE] instead.)\n`;
          continue;
        }
        
        callbacks.onStatus(`🔍 Querying ${evt}...`);
        try {
          const parsedPayload = pld ? JSON.parse(pld) : {};
          const result = await executeEvent(evt, parsedPayload);
          toolContext += `\n### Query: ${evt}\n${result}\n`;
        } catch (e) {
          toolContext += `\n(Query failed: ${e})\n`;
        }
      }

      // 2. [EVENT_APPROVE: eventName | payload]
      for (const [, eventName, payload] of eventApprovals.slice(0, 2)) {
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
          try {
            const result = await executeEvent(evt, parsedPayload);
            toolContext += `\n### Event: ${evt}\n${result}\n`;
          } catch (e) {
            toolContext += `\n(Event failed: ${e})\n`;
          }
        } else {
          callbacks.onStatus(`⏳ Awaiting approval: ${evt}`);
          const approvalId = newId();
          callbacks.onMessage({ id: approvalId, role: "system", content: `⚠️ Action Required:\nTrigger App Event: **${evt}**\n\`${pld}\`` });
          
          const approved = await callbacks.onApprovalRequired(approvalId, `[EVENT_APPROVE: ${evt} | ${pld}]`);

          if (approved) {
            callbacks.onMessage({ id: newId(), role: "system", content: `✅ Approved: \`${evt}\`. Executing...` });
            try {
              const result = await executeEvent(evt, parsedPayload);
              toolContext += `\n\n### Approved Event: \`${evt}\`\nResult: ${result}\n`;
              callbacks.onMessage({ id: newId(), role: "system", content: `🏁 Action Completed.` });
            } catch (e) {
              toolContext += `\n(Event execution failed: ${e})\n`;
            }
          } else {
            toolContext += `\n\n### Event DENIED by user: \`${evt}\`\n`;
            callbacks.onMessage({ id: newId(), role: "system", content: `❌ Denied: \`${evt}\`` });
          }
        }
      }

      // 3. Old Tools handling
      if (hasQueryState) {
        callbacks.onStatus("🔍 Reading live system state...");
        callbacks.onMessage({ id: newId(), role: "system", content: "🔍 Reading live system state..." });
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
          
          toolContext += `\n### Live System State
Host: ${JSON.stringify(hostSpecs.status === "fulfilled" ? hostSpecs.value : "unknown")}
Instances: ${JSON.stringify(instances.status === "fulfilled" ? instances.value : [])}
Containers (summary): ${JSON.stringify(
(containers.status === "fulfilled" ? containers.value : [])
  .map((c) => ({id: c.Id?.slice(0,12), name: c.Names, image: c.Image, status: c.Status}))
)}
Compose Projects: ${JSON.stringify(composeProjects.status === "fulfilled" ? composeProjects.value : [])}
K8s Context: ${k8sContext.status === "fulfilled" ? k8sContext.value : "none"}
Custom Presets: ${JSON.stringify(customPresets)}
Built-in Presets: ${JSON.stringify(BUILT_IN_PRESETS)}
`;
        } catch (e) {
          toolContext += `\n(Query app state failed: ${e})\n`;
        }
      }

      if (hasDiagnose) {
        callbacks.onStatus("🔬 Collecting diagnostics...");
        callbacks.onMessage({ id: newId(), role: "system", content: "🔬 Collecting diagnostics (logs, processes, locks)..." });
        try {
          const diagReport = await knowledgeBankApi.collectDiagnosticLogs("default");
          toolContext += `\n\n### Diagnostic Report\n${diagReport}\n`;
        } catch (e) { toolContext += `\n(Diagnostic failed: ${e})\n`; }
      }

      for (const [, cmd] of runs.slice(0, 3)) {
        callbacks.onStatus(`🔧 Executing: ${cmd}`);
        callbacks.onMessage({ id: newId(), role: "system", content: `🔧 Executing: \`${cmd}\`` });
        try {
          const r = await sandboxApi.execute(cmd);
          toolContext += `\n\n### Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${r.stdout || r.stderr || "(no output)"}\n\`\`\`\n`;
        } catch (e) { toolContext += `\n(Command failed: ${e})\n`; }
      }

      for (const [, cmd] of runApprovals.slice(0, 2)) {
        callbacks.onStatus(`⏳ Awaiting approval: ${cmd}`);
        const approvalId = newId();
        callbacks.onMessage({ id: approvalId, role: "system", content: `⚠️ Action Required:\n\`${cmd}\`` });
        const approved = await callbacks.onApprovalRequired(approvalId, cmd);
        if (approved) {
          try {
            const r = await sandboxApi.executeApproved(cmd);
            toolContext += `\n\n### Approved Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${r.stdout || r.stderr || "(no output)"}\n\`\`\`\n`;
            callbacks.onMessage({ id: newId(), role: "system", content: `✅ Approved & Executed: \`${cmd}\`` });
          } catch (e) { toolContext += `\n(Approved command failed: ${e})\n`; }
        } else {
          toolContext += `\n\n### Command DENIED by user: \`${cmd}\`\n`;
          callbacks.onMessage({ id: newId(), role: "system", content: `❌ Denied: \`${cmd}\`` });
        }
      }

      const securityTasks = [
        ...secThreatModels.map(([, dir, mode]) => ({ tool: "THREAT_MODEL", cmd: "omni", args: ["run", "security-threat-model", dir.trim(), mode ? mode.trim() : ""].filter(Boolean), dangerous: false })),
        ...secVulnScans.map(([, dir]) => ({ tool: "VULN_SCAN", cmd: "omni", args: ["run", "security-vuln-scan", dir.trim()], dangerous: false })),
        ...secTriages.map(([, path]) => ({ tool: "TRIAGE", cmd: "omni", args: ["run", "security-triage", path.trim()], dangerous: false })),
        ...secPatchGens.map(([, path, repo]) => ({ tool: "PATCH_GEN", cmd: "omni", args: ["run", "security-patch-gen", path.trim(), ...(repo ? [repo.trim()] : [])], dangerous: true })),
      ];

      for (const task of securityTasks) {
        callbacks.onStatus(`⏳ Awaiting approval: Security ${task.tool}`);
        const approvalId = newId();
        const warning = task.dangerous ? `\n\n🚨 **WARNING**: This generates code patches. Review carefully before applying.` : "";
        const cmdStr = `${task.cmd} ${task.args.join(" ")}`;
        callbacks.onMessage({ id: approvalId, role: "system", content: `⚠️ Security Action Required:\n**${task.tool}**\n\`${cmdStr}\`${warning}` });
        
        const approved = await callbacks.onApprovalRequired(approvalId, `[EVENT_APPROVE: cli-exec | {"command":"${task.cmd}","args":${JSON.stringify(task.args)}}]`);

        if (approved) {
          callbacks.onMessage({ id: newId(), role: "system", content: `✅ Approved: Security ${task.tool}. Executing...` });
          try {
            const result = await executeEvent("cli-exec", { command: task.cmd, args: task.args });
            toolContext += `\n\n### Security ${task.tool}\nResult: ${result}\n`;
            callbacks.onMessage({ id: newId(), role: "system", content: `🏁 Security ${task.tool} Completed.` });
          } catch (e) {
            toolContext += `\n(Execution failed: ${e})\n`;
          }
        } else {
          toolContext += `\n(User denied Security ${task.tool})\n`;
          callbacks.onMessage({ id: newId(), role: "system", content: `❌ Denied: Security ${task.tool}` });
        }
      }

      for (const [, expr, prompt] of schedCrons) {
        const id = newId("cron");
        callbacks.onCronSchedule(id, expr, prompt);
        toolContext += `\n(Scheduled cron job: ${id} with expr ${expr})\n`;
      }

      for (const [, seconds, prompt] of schedTimers) {
        const id = newId("timer");
        const ms = parseInt(seconds) * 1000;
        callbacks.onTimerSchedule(id, ms, prompt);
        toolContext += `\n(Scheduled timer: ${id} for ${seconds}s)\n`;
      }

      for (const [, id] of schedCancels) {
        const timerId = id.trim();
        callbacks.onScheduleCancel(timerId);
        toolContext += `\n(Attempted to cancel schedule: ${timerId})\n`;
      }

      for (const [, tab] of navigates) {
        const page = tab.toLowerCase().trim();
        toolContext += `\n(Navigated to ${page})\n`;
        callbacks.onMessage({ id: newId(), role: "system", content: `🧭 Navigated to ${page} tab` });
        callbacks.onNavigate(page);
      }

      for (const [, path] of readReferences.slice(0, 3)) {
        callbacks.onStatus(`📚 Reading reference: ${path}`);
        try {
          const refContent = await aiApi.readReference(path.trim());
          toolContext += `\n\n### Reference: ${path}\n${refContent}\n`;
        } catch {
          // readReference endpoint may not be implemented yet — skip gracefully
          toolContext += `\n(Reference ${path} unavailable)\n`;
        }
      }

      fullHistory.push(
        { role: "assistant", content: responseText },
        { role: "user", content: `[Tool Results]\n${toolContext}\n\nContinue reasoning.` }
      );
      // Send the final responseText to the UI to save to its history DB
      callbacks.onMessageUpdate(streamId, responseText);
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

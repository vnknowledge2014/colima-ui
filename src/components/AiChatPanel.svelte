<script module lang="ts">
  const activeSchedules = new Map<string, any>();
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { aiState, initAiHistory, pushAiMessage, clearAiHistory } from "../store/ai.svelte";
  import { uiState } from "../store.svelte";
  import { aiApi, colimaApi, dockerApi, composeApi, k8sApi, sysMethods, knowledgeBankApi, sandboxApi } from "../lib/api";
  import type { ChatMessage } from "../lib/api";
  import { chatStream } from "../lib/llmProviders";
  import { onError } from "../lib/globalToast";
  import { getCategory, executeEvent } from "../lib/aiEventBus";
  import { BUILT_IN_PRESETS } from "../lib/presetStateManager";
  import { setAppSetting, getAppSetting } from "../lib/settingsStore.svelte";
  import { renderMarkdown } from "../lib/markdown";
  import { parseAiTools } from "../lib/aiToolParser";

  onMount(() => {
    initAiHistory();
  });

  const MAX_SEARCH_ROUNDS = 3;

  function renderMarkdownHTML(text: string): string {
    return renderMarkdown(text);
  }

  let panelWidth = $state(parseInt(getAppSetting("ai_panel_width", "400"), 10));
  let isDragging = false;

  $effect(() => {
    setAppSetting("ai_panel_width", panelWidth.toString());
  });

  function handleMouseDown(e: MouseEvent) {
    isDragging = true;
    document.body.style.cursor = "ew-resize";
    const startX = e.clientX;
    const startWidth = panelWidth;

    const handleMouseMove = (e: MouseEvent) => {
      if (!isDragging) return;
      const newWidth = startWidth + (startX - e.clientX);
      if (newWidth > 300 && newWidth < 800) panelWidth = newWidth;
    };

    const handleMouseUp = () => {
      isDragging = false;
      document.body.style.cursor = "default";
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  }

  let userInput = $state("");
  let statusText = $state("");
  let pendingApprovals = $state<{ id: string; command: string; resolve: (v: boolean) => void }[]>([]);
  let appContext = $state("");

  const getProvider = () => getAppSetting("ai_provider", "anthropic");
  const getModel = () => getAppSetting("ai_model", "");
  const isAutoTrigger = () => getAppSetting("ai_diag_auto_trigger") !== "false";
  const getApiKey = () => getAppSetting("ai_api_key", "");
  const getEndpoint = () => getAppSetting("ai_endpoint", "");

  let messagesEndRef = $state<HTMLDivElement>();

  function scrollToBottom() {
    messagesEndRef?.scrollIntoView({ behavior: "smooth" });
  }

  $effect(() => {
    if (aiState.messages.length || aiState.isProcessing) {
      tick().then(scrollToBottom);
    }
  });

  onMount(() => {
    aiApi.getAppContext().then(ctx => appContext = ctx).catch(() => {});

    return onError((errorText) => {
      if (!isAutoTrigger()) return;
      pushAiMessage({
        id: Date.now().toString(), role: "system", content: `⚠️ Error detected: ${errorText}`
      });
      if (!uiState.aiPanelOpen) aiState.errorCount++;
      runAgentLoop(`System error occurred: ${errorText}\n\nPlease analyze this error and help me troubleshoot it.`);
    });
  });

  async function runAgentLoop(userMessage: string) {
    if (!getApiKey() && getProvider() !== "ollama-local") {
      pushAiMessage({ id: Date.now().toString(), role: "system", content: "⚠️ API Key not configured." });
      return;
    }

    aiState.isProcessing = true;
    let kbContext = "";
    try {
      const kbResult = await knowledgeBankApi.query(userMessage);
      if (kbResult.context_text) kbContext = kbResult.context_text;
    } catch {}

    let memoryContext = "";
    try {
      const memResult = await knowledgeBankApi.searchMemory(userMessage, 5);
      if (memResult && memResult.length > 0) {
        memoryContext = memResult.join("\n- ");
      }
    } catch {}

    const chatHistory: ChatMessage[] = [
      ...(appContext ? [{ role: "system" as const, content: appContext }] : []),
      ...(kbContext ? [{ role: "system" as const, content: `[Knowledge Bank]\n${kbContext}` }] : []),
      ...(memoryContext ? [{ role: "system" as const, content: `[USER_MEMORY]\n- ${memoryContext}` }] : []),
      ...aiState.messages.filter(m => m.role !== "system").slice(-10).map(m => ({ role: m.role as "user" | "assistant", content: m.content })),
      { role: "user", content: userMessage },
    ];

    try {
      for (let round = 0; round < MAX_SEARCH_ROUNDS; round++) {
        statusText = round === 0 ? "Agent is reasoning..." : `Agent Loop ${round + 1}...`;

        let responseText = "";
        let streamId = Date.now().toString();
        pushAiMessage({ id: streamId, role: "assistant", content: "" });

        await chatStream(getProvider(), getModel(), getApiKey(), chatHistory, getEndpoint(), (chunk) => {
          responseText += chunk;
          const idx = aiState.messages.findIndex(m => m.id === streamId);
          if (idx !== -1) {
            aiState.messages[idx].content = responseText;
          }
        });

        // Clean tool calls from UI text
        const parsed = parseAiTools(responseText);
        const {
          cleanText, hasTools, queries, eventApprovals,
          runs, runApprovals, hasDiagnose, hasQueryState,
          navigates, readReferences, secThreatModels, secVulnScans,
          secTriages, secPatchGens, schedCrons, schedTimers, schedCancels
        } = parsed;

        const idx = aiState.messages.findIndex(m => m.id === streamId);
        if (idx !== -1) {
          aiState.messages[idx].content = cleanText;
          if (!cleanText) {
            aiState.messages.splice(idx, 1);
          }
        }

        if (!hasTools) break;

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
          
          statusText = `🔍 Querying ${evt}...`;
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
          } catch(e) {}

          if (category === "SAFE") {
            // Auto execute SAFE events even if approved
            statusText = `✅ Auto-executing safe event ${evt}...`;
            try {
              const result = await executeEvent(evt, parsedPayload);
              toolContext += `\n### Event: ${evt}\n${result}\n`;
            } catch (e) {
              toolContext += `\n(Event failed: ${e})\n`;
            }
          } else {
            // NORMAL or DANGEROUS requires approval
            statusText = `⏳ Awaiting approval: ${evt}`;
            const approvalId = Date.now().toString();
            pushAiMessage({ id: approvalId, role: "system", content: `⚠️ Action Required:\nTrigger App Event: **${evt}**\n\`${pld}\`` });
            
            const approved = await new Promise<boolean>((resolve) => {
              pendingApprovals.push({ id: approvalId, command: `[EVENT_APPROVE: ${evt} | ${pld}]`, resolve });
            });

            if (approved) {
              pushAiMessage({ id: Date.now().toString(), role: "system", content: `✅ Approved: \`${evt}\`. Executing...` });
              try {
                const result = await executeEvent(evt, parsedPayload);
                toolContext += `\n\n### Approved Event: \`${evt}\`\nResult: ${result}\n`;
                pushAiMessage({ id: Date.now().toString(), role: "system", content: `🏁 Action Completed.` });
              } catch (e) {
                toolContext += `\n(Event execution failed: ${e})\n`;
              }
            } else {
              toolContext += `\n\n### Event DENIED by user: \`${evt}\`\n`;
              pushAiMessage({ id: Date.now().toString(), role: "system", content: `❌ Denied: \`${evt}\`` });
            }
          }
        }

        // 3. Old Tools handling
        if (hasQueryState) {
          statusText = "🔍 Reading live system state...";
          pushAiMessage({ id: Date.now().toString(), role: "system", content: "🔍 Reading live system state..." });
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
            
            const customPresets = JSON.parse(
              getAppSetting("ColimaCustomProfiles", "[]")
            );

            toolContext += `\n### Live System State
Host: ${JSON.stringify(hostSpecs.status === "fulfilled" ? hostSpecs.value : "unknown")}
Instances: ${JSON.stringify(instances.status === "fulfilled" ? instances.value : [])}
Containers (summary): ${JSON.stringify(
  (containers.status === "fulfilled" ? containers.value : [])
    .map((c: any) => ({id: c.Id?.slice(0,12), name: c.Names, image: c.Image, status: c.Status}))
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
          statusText = "🔬 Collecting diagnostics...";
          pushAiMessage({ id: Date.now().toString(), role: "system", content: "🔬 Collecting diagnostics (logs, processes, locks)..." });
          try {
            const diagReport = await knowledgeBankApi.collectDiagnosticLogs("default");
            toolContext += `\n\n### Diagnostic Report\n${diagReport}\n`;
          } catch (e) { toolContext += `\n(Diagnostic failed: ${e})\n`; }
        }

        for (const [, cmd] of runs.slice(0, 3)) {
          statusText = `🔧 Executing: ${cmd}`;
          pushAiMessage({ id: Date.now().toString(), role: "system", content: `🔧 Executing: \`${cmd}\`` });
          try {
            const r = await sandboxApi.execute(cmd);
            toolContext += `\n\n### Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${r.stdout || r.stderr || "(no output)"}\n\`\`\`\n`;
          } catch (e) { toolContext += `\n(Command failed: ${e})\n`; }
        }

        for (const [, cmd] of runApprovals.slice(0, 2)) {
          statusText = `⏳ Awaiting approval: ${cmd}`;
          const approvalId = Date.now().toString();
          pushAiMessage({ id: approvalId, role: "system", content: `⚠️ Action Required:\n\`${cmd}\`` });
          const approved = await new Promise<boolean>((resolve) => {
            pendingApprovals.push({ id: approvalId, command: cmd, resolve });
          });
          if (approved) {
            try {
              const r = await sandboxApi.executeApproved(cmd);
              toolContext += `\n\n### Approved Command: \`${cmd}\` (exit: ${r.exit_code})\n\`\`\`\n${r.stdout || r.stderr || "(no output)"}\n\`\`\`\n`;
              pushAiMessage({ id: Date.now().toString(), role: "system", content: `✅ Approved & Executed: \`${cmd}\`` });
            } catch (e) { toolContext += `\n(Approved command failed: ${e})\n`; }
          } else {
            toolContext += `\n\n### Command DENIED by user: \`${cmd}\`\n`;
            pushAiMessage({ id: Date.now().toString(), role: "system", content: `❌ Denied: \`${cmd}\`` });
          }
        }

        const securityTasks = [
          ...secThreatModels.map(([, dir, mode]) => ({ tool: "THREAT_MODEL", cmd: "omni", args: ["run", "security-threat-model", dir.trim(), mode ? mode.trim() : ""].filter(Boolean), dangerous: false })),
          ...secVulnScans.map(([, dir]) => ({ tool: "VULN_SCAN", cmd: "omni", args: ["run", "security-vuln-scan", dir.trim()], dangerous: false })),
          ...secTriages.map(([, path]) => ({ tool: "TRIAGE", cmd: "omni", args: ["run", "security-triage", path.trim()], dangerous: false })),
          ...secPatchGens.map(([, path, repo]) => ({ tool: "PATCH_GEN", cmd: "omni", args: ["run", "security-patch-gen", path.trim(), ...(repo ? [repo.trim()] : [])], dangerous: true })),
        ];

        for (const task of securityTasks) {
          statusText = `⏳ Awaiting approval: Security ${task.tool}`;
          const approvalId = Date.now().toString();
          const warning = task.dangerous ? `\n\n🚨 **WARNING**: This generates code patches. Review carefully before applying.` : "";
          const cmdStr = `${task.cmd} ${task.args.join(" ")}`;
          pushAiMessage({ id: approvalId, role: "system", content: `⚠️ Security Action Required:\n**${task.tool}**\n\`${cmdStr}\`${warning}` });
          
          const approved = await new Promise<boolean>((resolve) => {
            pendingApprovals.push({ id: approvalId, command: `[EVENT_APPROVE: cli-exec | {"command":"${task.cmd}","args":${JSON.stringify(task.args)}}]`, resolve });
          });

          if (approved) {
            pushAiMessage({ id: Date.now().toString(), role: "system", content: `✅ Approved: Security ${task.tool}. Executing...` });
            try {
              const result = await executeEvent("cli-exec", { command: task.cmd, args: task.args });
              toolContext += `\n\n### Security ${task.tool}\nResult: ${result}\n`;
              pushAiMessage({ id: Date.now().toString(), role: "system", content: `🏁 Security ${task.tool} Completed.` });
            } catch (e) {
              toolContext += `\n(Execution failed: ${e})\n`;
            }
          } else {
            toolContext += `\n(User denied Security ${task.tool})\n`;
            pushAiMessage({ id: Date.now().toString(), role: "system", content: `❌ Denied: Security ${task.tool}` });
          }
        }

        for (const [, expr, prompt] of schedCrons) {
          const id = `cron-${Date.now()}`;
          // Rudimentary cron mock (runs every 60s for demo)
          const timer = setInterval(() => {
            pushAiMessage({ id: Date.now().toString(), role: "system", content: `⏰ Cron triggered: ${prompt}` });
            runAgentLoop(`[SYSTEM CRON TICK]: ${prompt}`);
          }, 60000);
          activeSchedules.set(id, timer);
          toolContext += `\n(Scheduled cron job: ${id} with expr ${expr})\n`;
        }

        for (const [, seconds, prompt] of schedTimers) {
          const id = `timer-${Date.now()}`;
          const ms = parseInt(seconds) * 1000;
          const timer = setTimeout(() => {
            pushAiMessage({ id: Date.now().toString(), role: "system", content: `⏰ Timer triggered: ${prompt}` });
            runAgentLoop(`[SYSTEM TIMER TICK]: ${prompt}`);
            activeSchedules.delete(id);
          }, ms);
          activeSchedules.set(id, timer);
          toolContext += `\n(Scheduled timer: ${id} for ${seconds}s)\n`;
        }

        for (const [, id] of schedCancels) {
          const timerId = id.trim();
          if (activeSchedules.has(timerId)) {
            const timer = activeSchedules.get(timerId);
            clearTimeout(timer as any);
            clearInterval(timer as any);
            activeSchedules.delete(timerId);
            toolContext += `\n(Cancelled schedule: ${timerId})\n`;
          } else {
            toolContext += `\n(Failed to cancel: ${timerId} not found)\n`;
          }
        }

        for (const [, tab] of navigates) {
          const page = tab.toLowerCase().trim();
          toolContext += `\n(Navigated to ${page})\n`;
          pushAiMessage({ id: Date.now().toString(), role: "system", content: `🧭 Navigated to ${page} tab` });
          uiState.currentPage = page;
        }

        for (const [, path] of readReferences.slice(0, 3)) {
          statusText = `📚 Reading reference: ${path}`;
          try {
            const refContent = await aiApi.readReference(path.trim());
            toolContext += `\n\n### Reference: ${path}\n${refContent}\n`;
          } catch (e) {
            toolContext += `\n(Failed to read reference ${path}: ${e})\n`;
          }
        }

        chatHistory.push(
          { role: "assistant", content: responseText },
          { role: "user", content: `[Tool Results]\n${toolContext}\n\nContinue reasoning.` }
        );
        // Save the final assistant message to DB after streaming finishes
        pushAiMessage({ id: streamId, role: "assistant", content: responseText });
      }
    } catch (e) {
      pushAiMessage({ id: Date.now().toString(), role: "assistant", content: `Error: ${e}` });
    } finally {
      aiState.isProcessing = false;
      statusText = "";
    }
  }

  function handleSend(textOverride?: string) {
    const text = textOverride || userInput.trim();
    if (!text || aiState.isProcessing) return;
    pushAiMessage({ id: Date.now().toString(), role: "user", content: text });
    userInput = "";
    runAgentLoop(text);
  }

  function handleClear() {
    clearAiHistory();
  }
</script>

{#if uiState.aiPanelOpen}
  <div class="ai-panel" style="width: {panelWidth}px">
    <div class="ai-panel-resizer" onmousedown={handleMouseDown} title="Drag to resize"></div>
    <div class="ai-panel-header">
      <div class="ai-panel-title">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon>
        </svg>
        Command Center
        {#if aiState.isProcessing}
          <span class="ai-status-badge">{statusText || "Processing..."}</span>
        {/if}
      </div>
      <div class="ai-panel-actions">
        <button onclick={() => uiState.currentPage = 'settings'} title="Open Settings">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
        </button>
        <button onclick={handleClear} title="Clear chat history">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
        </button>
        <button onclick={() => uiState.aiPanelOpen = false} title="Close Panel">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
    </div>

    <div class="ai-panel-messages">
      {#if aiState.messages.length === 0}
        <div class="ai-panel-empty">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" style="margin-bottom: 16px;">
            <path d="M12 2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2 2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z"></path>
            <path d="M5.636 5.636a2 2 0 0 1 2.828 0l1.414 1.414a2 2 0 0 1 0 2.828 2 2 0 0 1-2.828 0L5.636 7.05a2 2 0 0 1 0-2.828z"></path>
            <path d="M22 12a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2 2 2 0 0 1 2-2h2a2 2 0 0 1 2 2z"></path>
            <path d="M18.364 18.364a2 2 0 0 1-2.828 0l-1.414-1.414a2 2 0 0 1 0-2.828 2 2 0 0 1 2.828 0l1.414 1.414a2 2 0 0 1 0 2.828z"></path>
            <path d="M12 22a2 2 0 0 1-2-2v-2a2 2 0 0 1 2-2 2 2 0 0 1 2 2v2a2 2 0 0 1-2 2z"></path>
            <path d="M5.636 18.364a2 2 0 0 1 0-2.828l1.414-1.414a2 2 0 0 1 2.828 0 2 2 0 0 1 0 2.828l-1.414 1.414a2 2 0 0 1-2.828 0z"></path>
            <path d="M2 12a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2 2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"></path>
            <path d="M5.636 5.636l1.414 1.414"></path>
          </svg>
          <div style="font-weight: 600; font-size: var(--text-lg); color: var(--text-primary);">Ready for commands</div>
          <div style="font-size: 13px; color: var(--text-secondary); margin-top: 8px; max-width: 80%;">
            I can manage instances, containers, troubleshoot errors, and write Dockerfiles.
          </div>
          <div class="ai-suggestion-pills">
            <button class="ai-pill" onclick={() => handleSend("Check system status")}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 12h-4l-3 9L9 3l-3 9H2"></path></svg> Check system status
            </button>
            <button class="ai-pill" onclick={() => handleSend("List running VMs")}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg> List running VMs
            </button>
            <button class="ai-pill" onclick={() => handleSend("Prune unused Docker resources")}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"></path><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg> Optimize Docker
            </button>
          </div>
        </div>
      {/if}
      {#each aiState.messages as msg}
        <div class="ai-msg ai-msg-{msg.role}">
          {#if msg.role === "system"}
            <span class="ai-system-msg">{@html renderMarkdownHTML(msg.content)}</span>
          {:else}
            <div class="ai-msg-content">{@html renderMarkdownHTML(msg.content)}</div>
            {#if msg.role === "assistant" && !aiState.isProcessing && msg === aiState.messages[aiState.messages.length - 1]}
              <div class="ai-followup-pills">
                <button class="ai-pill" onclick={() => handleSend("Explain more detail")}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"></path><line x1="12" y1="17" x2="12.01" y2="17"></line></svg> Explain
                </button>
                <button class="ai-pill" onclick={() => handleSend("Check logs")}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg> Check Logs
                </button>
              </div>
            {/if}
          {/if}
          
          {#if pendingApprovals.some(a => a.id === msg.id)}
            <div class="ai-approval-buttons">
              <button class="btn btn-primary" onclick={() => {
                const item = pendingApprovals.find(a => a.id === msg.id);
                if (item) item.resolve(true);
                pendingApprovals = pendingApprovals.filter(a => a.id !== msg.id);
              }}>Allow Execution</button>
              <button class="btn btn-danger" onclick={() => {
                const item = pendingApprovals.find(a => a.id === msg.id);
                if (item) item.resolve(false);
                pendingApprovals = pendingApprovals.filter(a => a.id !== msg.id);
              }}>Deny</button>
            </div>
          {/if}
        </div>
      {/each}
      {#if aiState.isProcessing}
        <div class="ai-msg ai-msg-assistant" style="padding: 6px 14px; align-self: flex-start;">
          <div class="ai-typing-dots">
            <span></span><span></span><span></span>
          </div>
        </div>
      {/if}
      <div bind:this={messagesEndRef}></div>
    </div>

    <div class="ai-panel-input">
      <textarea
        bind:value={userInput}
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); }
        }}
        placeholder="Ask AI to do something..."
        disabled={aiState.isProcessing}
        rows={2}
      ></textarea>
      <button onclick={() => handleSend()} disabled={aiState.isProcessing || !userInput.trim()} title="Send message">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"></line><polygon points="22 2 15 22 11 13 2 9 22 2"></polygon></svg>
      </button>
    </div>
  </div>
{/if}


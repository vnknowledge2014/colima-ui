<script module lang="ts">
  const activeSchedules = new Map<string, any>();
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { aiState, initAiHistory, pushAiMessage, clearAiHistory } from "../store/ai.svelte";
  import { uiState } from "../store.svelte";
  import { aiApi } from "../lib/api";
  import { onError } from "../lib/globalToast";
  import { setAppSetting, getAppSetting } from "../lib/settingsStore.svelte";
  import { renderMarkdown } from "../lib/markdown";
  import { runAgent, type AgentCallbacks } from "../lib/agentCore";
  import { BUILT_IN_PRESETS } from "../lib/presetStateManager";

  onMount(() => {
    initAiHistory();
  });

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
      handleSend(`System error occurred: ${errorText}\n\nPlease analyze this error and help me troubleshoot it.`);
    });
  });

  async function handleSend(textOverride?: string) {
    const text = textOverride || userInput.trim();
    if (!text || aiState.isProcessing) return;
    
    pushAiMessage({ id: Date.now().toString(), role: "user", content: text });
    userInput = "";
    aiState.isProcessing = true;
    
    const callbacks: AgentCallbacks = {
      onStatus: (t) => { statusText = t; },
      onMessage: (msg) => { pushAiMessage(msg); },
      onMessageUpdate: (id, content) => {
        const idx = aiState.messages.findIndex(m => m.id === id);
        if (idx !== -1) aiState.messages[idx].content = content;
      },
      onMessageDelete: (id) => {
        const idx = aiState.messages.findIndex(m => m.id === id);
        if (idx !== -1) aiState.messages.splice(idx, 1);
      },
      onApprovalRequired: (id, command) => {
        return new Promise<boolean>((resolve) => {
          pendingApprovals.push({ id, command, resolve });
        });
      },
      onNavigate: (page) => {
        uiState.currentPage = page;
      },
      onTimerSchedule: (id, ms, prompt) => {
        const timer = setTimeout(() => {
          pushAiMessage({ id: Date.now().toString(), role: "system", content: `⏰ Timer triggered: ${prompt}` });
          handleSend(`[SYSTEM TIMER TICK]: ${prompt}`);
          activeSchedules.delete(id);
        }, ms);
        activeSchedules.set(id, timer);
      },
      onCronSchedule: (id, expr, prompt) => {
        const timer = setInterval(() => {
          pushAiMessage({ id: Date.now().toString(), role: "system", content: `⏰ Cron triggered: ${prompt}` });
          handleSend(`[SYSTEM CRON TICK]: ${prompt}`);
        }, 60000);
        activeSchedules.set(id, timer);
      },
      onScheduleCancel: (id) => {
        if (activeSchedules.has(id)) {
          const timer = activeSchedules.get(id);
          clearTimeout(timer as any);
          clearInterval(timer as any);
          activeSchedules.delete(id);
        }
      }
    };
    
    const config = {
      provider: getProvider(),
      model: getModel(),
      apiKey: getApiKey(),
      endpoint: getEndpoint()
    };
    
    const customPresets = JSON.parse(getAppSetting("ColimaCustomProfiles", "[]"));
    
    try {
      await runAgent(text, config, callbacks, [...aiState.messages], appContext, customPresets);
    } catch (e: any) {
      // Surface agent errors as a chat bubble instead of crashing / leaking to toast
      const msg = e?.message || String(e) || "Unknown error occurred";
      pushAiMessage({
        id: Date.now().toString(),
        role: "assistant",
        content: `⚠️ **Agent error:** ${msg}\n\nIf this persists, check your AI provider settings.`,
      });
    } finally {
      aiState.isProcessing = false;
      statusText = "";
    }
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
          </svg>
          <p>Hi! I'm your Colima AI Assistant.</p>
          <p style="font-size: var(--text-xs); color: var(--text-muted); margin-top: 8px;">
            I can control containers, troubleshoot issues, read the knowledge bank, and manage your VMs.
          </p>
        </div>
      {/if}

      {#each aiState.messages as msg (msg.id)}
        <div class="ai-msg {msg.role}">
          {#if msg.role === "system"}
            <div class="ai-msg-system">
              {@html renderMarkdownHTML(msg.content)}
            </div>
          {:else if msg.role === "user"}
            <div class="ai-msg-user">
              {@html renderMarkdownHTML(msg.content)}
            </div>
          {:else}
            <div class="ai-msg-assistant">
              {@html renderMarkdownHTML(msg.content)}
            </div>
          {/if}
        </div>
      {/each}

      {#each pendingApprovals as approval (approval.id)}
        <div class="ai-msg system">
          <div class="ai-msg-system" style="border: 1px solid var(--accent-red); background: rgba(248, 81, 73, 0.1);">
            <div style="font-weight: 600; margin-bottom: 8px; color: var(--accent-red);">Action Approval Required</div>
            <pre style="margin: 0 0 12px 0; background: var(--bg-app); padding: 8px; border-radius: 4px; font-size: var(--text-xs); overflow-x: auto;">{approval.command}</pre>
            <div style="display: flex; gap: 8px;">
              <button 
                class="btn" 
                style="background: var(--accent-green); color: #fff; border: none; padding: 6px 12px; font-size: var(--text-xs); border-radius: 4px; cursor: pointer;"
                onclick={() => {
                  approval.resolve(true);
                  pendingApprovals = pendingApprovals.filter(a => a.id !== approval.id);
                }}
              >Approve</button>
              <button 
                class="btn btn-ghost"
                style="padding: 6px 12px; font-size: var(--text-xs); border-radius: 4px; border: 1px solid var(--border-primary); cursor: pointer; color: var(--text-primary);"
                onclick={() => {
                  approval.resolve(false);
                  pendingApprovals = pendingApprovals.filter(a => a.id !== approval.id);
                }}
              >Deny</button>
            </div>
          </div>
        </div>
      {/each}

      <div bind:this={messagesEndRef} style="height: 1px;"></div>
    </div>

    <div class="ai-panel-input">
      <textarea
        bind:value={userInput}
        placeholder="Type a message... (Enter to send)"
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            handleSend();
          }
        }}
        disabled={aiState.isProcessing}
      ></textarea>
      <button 
        class="ai-send-btn" 
        disabled={!userInput.trim() || aiState.isProcessing}
        onclick={() => handleSend()}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"></line><polygon points="22 2 15 22 11 13 2 9 22 2"></polygon></svg>
      </button>
    </div>
  </div>
{/if}

<style>
  /* AI Chat Panel */
  .ai-panel {
    position: fixed;
    top: var(--header-height);
    right: 0;
    bottom: 0;
    background: var(--bg-sidebar);
    border-left: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    z-index: 100;
    box-shadow: var(--shadow-lg);
  }

  .ai-panel-resizer {
    position: absolute;
    left: -4px;
    top: 0;
    bottom: 0;
    width: 8px;
    cursor: ew-resize;
    z-index: 101;
  }
  .ai-panel-resizer:hover {
    background: rgba(88, 166, 255, 0.2);
  }

  .ai-panel-header {
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    border-bottom: 1px solid var(--border-primary);
    background: var(--bg-sidebar);
  }

  .ai-panel-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ai-status-badge {
    font-size: 10px;
    background: rgba(88, 166, 255, 0.1);
    color: var(--accent-blue);
    padding: 2px 6px;
    border-radius: 10px;
    border: 1px solid rgba(88, 166, 255, 0.2);
    animation: pulse 2s infinite;
  }

  .ai-panel-actions {
    display: flex;
    gap: 4px;
  }
  .ai-panel-actions button {
    background: transparent;
    border: none;
    color: var(--text-muted);
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .ai-panel-actions button:hover {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }

  .ai-panel-messages {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .ai-panel-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-secondary);
    text-align: center;
    padding: 24px;
  }

  .ai-msg {
    display: flex;
    flex-direction: column;
  }

  .ai-msg-user {
    align-self: flex-end;
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    padding: 10px 14px;
    border-radius: 12px 12px 0 12px;
    color: var(--text-primary);
    font-size: var(--text-sm);
    max-width: 85%;
    line-height: 1.5;
  }

  .ai-msg-assistant {
    align-self: flex-start;
    padding: 10px 14px;
    border-radius: 12px 12px 12px 0;
    color: var(--text-primary);
    font-size: var(--text-sm);
    max-width: 95%;
    line-height: 1.6;
    background: rgba(88, 166, 255, 0.05);
    border: 1px solid rgba(88, 166, 255, 0.2);
  }

  .ai-msg-system {
    align-self: center;
    background: var(--bg-app);
    border: 1px dashed var(--border-primary);
    padding: 8px 12px;
    border-radius: 8px;
    color: var(--text-secondary);
    font-size: 12px;
    max-width: 90%;
    line-height: 1.5;
  }

  .ai-panel-input {
    padding: 12px;
    border-top: 1px solid var(--border-primary);
    background: var(--bg-sidebar);
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }

  .ai-panel-input textarea {
    flex: 1;
    background: var(--bg-input);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    color: var(--text-primary);
    font-size: var(--text-sm);
    resize: none;
    height: 44px;
    min-height: 44px;
    max-height: 120px;
    font-family: inherit;
    line-height: 1.4;
  }
  .ai-panel-input textarea:focus {
    outline: none;
    border-color: var(--accent-blue);
    box-shadow: 0 0 0 1px var(--accent-blue);
  }
  .ai-panel-input textarea:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .ai-send-btn {
    width: 44px;
    height: 44px;
    border-radius: var(--radius-md);
    background: var(--accent-blue);
    color: #fff;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .ai-send-btn:hover:not(:disabled) {
    background: #3a8ee6;
  }
  .ai-send-btn:disabled {
    background: var(--border-primary);
    color: var(--text-muted);
    cursor: not-allowed;
  }

  @keyframes pulse {
    0% { opacity: 0.6; }
    50% { opacity: 1; }
    100% { opacity: 0.6; }
  }
</style>

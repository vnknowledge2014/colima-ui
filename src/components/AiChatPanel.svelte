<script module lang="ts">
  const activeSchedules = new Map<string, number>();
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    aiState,
    initAiHistory,
    pushAiMessage,
    clearAiHistory,
    createConversation,
    switchConversation,
    renameConversation,
    deleteConversation,
    refreshConversations,
    type AiMessage,
  } from "../store/ai.svelte";
  import { t } from "../lib/i18n.svelte";
  import { uiState, openSettingsSection } from "../store.svelte";
  import { closeNotificationPanel } from "../store/notifications.svelte";
  import { aiApi } from "../lib/api";
  import { onError } from "../lib/globalToast";
  import { setAppSetting, getAppSetting } from "../lib/settingsStore.svelte";
  import { renderMarkdown } from "../lib/markdown";
  import { newId } from "../lib/ids";
  import { runAgent, type AgentCallbacks } from "../lib/agentCore";

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
  let showHistory = $state(false);
  let inputEl = $state<HTMLTextAreaElement>();
  let pendingApprovals = $state<{ id: string; command: string; resolve: (v: boolean) => void }[]>([]);
  let appContext = $state("");
  let renamingId = $state("");
  let renameDraft = $state("");
  /**
   * Typed while the agent was still running. The composer stays editable during
   * a run, so Enter has to do something — dropping the text silently reads as a
   * dead input. Sent automatically once the current run finishes.
   */
  let queuedMessage = $state("");

  /** Cancels the in-flight agent run when the user presses Stop. */
  let abortController: AbortController | null = null;

  const INPUT_MIN_HEIGHT = 36;
  const INPUT_MAX_HEIGHT = 140;

  /** Textarea's own laid-out width. See the effect below for why. */
  let inputWidth = $state(0);

  // Auto-grow the input like IDE chat boxes (Cursor): it grows with the text
  // up to a cap, then scrolls internally. Reset to single-line when emptied.
  function autoGrowTextarea() {
    if (!inputEl) return;
    if (!inputEl.value) {
      inputEl.style.height = `${INPUT_MIN_HEIGHT}px`;
      return;
    }
    // Measuring a box that has no width yet wraps a single line into dozens,
    // so scrollHeight comes back past the cap and the composer sticks at its
    // maximum height. Leave the height alone until there is a real width.
    if (!inputEl.clientWidth) return;
    inputEl.style.height = "auto";
    inputEl.style.height = `${Math.min(inputEl.scrollHeight, INPUT_MAX_HEIGHT)}px`;
  }

  /**
   * Keep the height derived from the *current* text and the *current* width.
   *
   * `inputWidth` is bound to the element's real laid-out width rather than to
   * `panelWidth`: the panel state changes before layout settles, so measuring
   * off it can size the box from a width the textarea does not have yet. Going
   * through the measured value also makes this self-healing — if one run lands
   * too early, the width arriving later re-runs it with correct numbers.
   */
  $effect(() => {
    void userInput;
    if (inputWidth > 0) autoGrowTextarea();
  });

  /**
   * True while an IME is assembling a character.
   *
   * Tracked ourselves in addition to `KeyboardEvent.isComposing` because the
   * Enter that commits an IME candidate arrives as a *second* keydown with
   * `isComposing === false` in Chromium — so that flag alone still sends the
   * message the user was only trying to confirm. Cleared a tick after
   * `compositionend` so that trailing keydown is still covered.
   */
  let isComposing = false;

  function handleCompositionStart() {
    isComposing = true;
  }

  function handleCompositionEnd() {
    setTimeout(() => { isComposing = false; }, 0);
  }

  function handleInputKeydown(e: KeyboardEvent) {
    if (e.key !== "Enter" || e.shiftKey) return;
    // Vietnamese Telex/VNI, Japanese and Chinese input all commit with Enter,
    // so sending on that keypress swallows the word being typed.
    if (isComposing || e.isComposing || e.keyCode === 229) return;
    e.preventDefault();
    handleSend();
  }

  /**
   * Settle every approval the agent is still waiting on, as denied.
   *
   * `onApprovalRequired` hands back a promise that only the Approve/Deny
   * buttons resolve, so an approval left unsettled makes `runAgent` await
   * forever and pins `isProcessing` true for the rest of the session. Every
   * path that abandons a run has to drain this queue.
   *
   * Hiding the panel is deliberately *not* one of those paths: the run keeps
   * going in the background and the bubble badge counts the waiting prompt, so
   * the user can come back and answer it.
   */
  function releasePendingApprovals() {
    for (const approval of pendingApprovals) approval.resolve(false);
    pendingApprovals = [];
  }

  function handleStop() {
    abortController?.abort();
    releasePendingApprovals();
    // Stopping means "I no longer want this exchange" — carrying on with the
    // follow-up the user queued for it would be the opposite.
    queuedMessage = "";
  }

  /** Hides the panel. The agent run, if any, carries on behind the bubble. */
  function hidePanel() {
    uiState.aiPanelOpen = false;
  }

  function showPanel() {
    // The notification centre is a fixed overlay with a backdrop; opened, it would
    // sit on top of this panel and swallow every click meant for it. One side
    // panel at a time, enforced from both directions.
    closeNotificationPanel();
    uiState.aiPanelOpen = true;
  }

  // Anything visible is read. Done as an effect rather than inside `showPanel`
  // because the sidebar toggles the same flag directly, and a second entry
  // point would otherwise leave the badge stuck on.
  $effect(() => {
    if (uiState.aiPanelOpen && aiState.errorCount > 0) aiState.errorCount = 0;
  });

  /**
   * What the bubble badge counts: errors that arrived while hidden, plus any
   * action the agent is blocked on. Both need the user back in the panel.
   */
  const unreadCount = $derived(aiState.errorCount + pendingApprovals.length);

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
        id: newId(), role: "system", content: `⚠️ Error detected: ${errorText}`
      });
      if (!uiState.aiPanelOpen) aiState.errorCount++;
      handleSend(`System error occurred: ${errorText}\n\nPlease analyze this error and help me troubleshoot it.`);
    });
  });

  async function handleSend(textOverride?: string) {
    const text = textOverride || userInput.trim();
    if (!text) return;

    if (aiState.isProcessing) {
      // Only what the user typed is held back. Automatic sends (error triggers,
      // timers) are dropped as before rather than queued, so the agent never
      // wakes up to a backlog of machine-generated prompts.
      if (!textOverride) {
        queuedMessage = text;
        userInput = "";
        if (inputEl) inputEl.style.height = `${INPUT_MIN_HEIGHT}px`;
      }
      return;
    }
    
    pushAiMessage({ id: newId(), role: "user", content: text });
    userInput = "";
    if (inputEl) inputEl.style.height = `${INPUT_MIN_HEIGHT}px`;
    aiState.isProcessing = true;
    abortController = new AbortController();

    const callbacks: AgentCallbacks = {
      onStatus: (t) => { statusText = t; },
      onMessage: (msg) => { pushAiMessage(msg as AiMessage); },
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
          pushAiMessage({ id: newId(), role: "system", content: `⏰ Timer triggered: ${prompt}` });
          handleSend(`[SYSTEM TIMER TICK]: ${prompt}`);
          activeSchedules.delete(id);
        }, ms);
        activeSchedules.set(id, timer);
      },
      // The cron expression is unused: the schedule below is a fixed 60s tick.
      onCronSchedule: (id, _expr, prompt) => {
        const timer = setInterval(() => {
          pushAiMessage({ id: newId(), role: "system", content: `⏰ Cron triggered: ${prompt}` });
          handleSend(`[SYSTEM CRON TICK]: ${prompt}`);
        }, 60000);
        activeSchedules.set(id, timer);
      },
      onScheduleCancel: (id) => {
        if (activeSchedules.has(id)) {
          const timer = activeSchedules.get(id);
          if (timer !== undefined) {
            clearTimeout(timer);
            clearInterval(timer);
          }
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

    const customPresets: unknown = (() => {
      try {
        return JSON.parse(getAppSetting("ColimaCustomProfiles", "[]")) as unknown;
      } catch {
        return [];
      }
    })();

    try {
      await runAgent(
        text, config, callbacks, [...aiState.messages], appContext, customPresets,
        abortController.signal
      );
    } finally {
      // isProcessing gates the send button — leaving it true locks the panel.
      aiState.isProcessing = false;
      statusText = "";
      abortController = null;
      // The list is ordered by last activity and shows a message count.
      refreshConversations();
    }

    // Outside the `finally` so the queued turn starts from a settled state
    // rather than nesting inside the run that was just cleaned up.
    if (queuedMessage) {
      const next = queuedMessage;
      queuedMessage = "";
      handleSend(next);
    }
  }

  function handleClear() {
    clearAiHistory();
  }

  async function handleNewChat() {
    handleStop();
    await createConversation();
    showHistory = false;
    inputEl?.focus();
  }

  async function handleSwitch(id: string) {
    if (id === aiState.activeConversationId) {
      showHistory = false;
      return;
    }
    handleStop();
    await switchConversation(id);
    showHistory = false;
  }

  function startRename(id: string, currentTitle: string) {
    renamingId = id;
    renameDraft = currentTitle;
  }

  async function commitRename() {
    const id = renamingId;
    const title = renameDraft.trim();
    renamingId = "";
    if (id && title) await renameConversation(id, title);
  }

  /** Falls back to the first user message so untitled threads stay identifiable. */
  function conversationLabel(conv: { title: string; preview: string }): string {
    if (conv.title) return conv.title;
    const preview = conv.preview.replace(/[#*`>]/g, "").trim();
    if (preview) return preview.length > 48 ? `${preview.slice(0, 48)}…` : preview;
    return t("ai.untitled_chat", { default: "New chat" });
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
        {t("ai.title", { default: "Command Center" })}
        {#if aiState.isProcessing}
          <span class="ai-status-badge">{statusText || t("ai.processing", { default: "Processing..." })}</span>
        {/if}
      </div>
      <div class="ai-panel-actions">
        <button onclick={handleNewChat} title={t("ai.new_chat", { default: "New chat" })}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
        </button>
        <button
          onclick={() => openSettingsSection("ai")}
          title={t("ai.open_settings", { default: "Open AI settings" })}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
        </button>
        <button
          onclick={() => { showHistory = !showHistory; if (showHistory) refreshConversations(); }}
          title={t("ai.chat_history", { default: "Chat history" })}
          class:active={showHistory}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>
        </button>
        <button onclick={handleClear} title={t("ai.clear_chat", { default: "Clear this chat" })}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
        </button>
        <!-- Minimise, not close: a chevron rather than an X, because the run
             and the conversation both survive. -->
        <button onclick={hidePanel} title={t("ai.hide_panel", { default: "Hide chat (keeps running)" })}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="13 17 18 12 13 7"></polyline><polyline points="6 17 11 12 6 7"></polyline></svg>
        </button>
      </div>
    </div>

    {#if showHistory}
      <div class="ai-history-dropdown">
        <div class="ai-history-dropdown-title">
          {t("ai.conversations", { default: "Conversations" })} · {aiState.conversations.length}
        </div>
        {#each aiState.conversations as conv (conv.id)}
          <div class="ai-history-row" class:active={conv.id === aiState.activeConversationId}>
            {#if renamingId === conv.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="ai-history-rename"
                bind:value={renameDraft}
                autofocus
                onblur={commitRename}
                onkeydown={(e) => {
                  if (e.key === "Enter") { e.preventDefault(); commitRename(); }
                  if (e.key === "Escape") renamingId = "";
                }}
              />
            {:else}
              <button class="ai-history-item" onclick={() => handleSwitch(conv.id)}>
                <span class="ai-history-item-title">{conversationLabel(conv)}</span>
                <span class="ai-history-item-meta">
                  {conv.message_count} {t("ai.messages", { default: "messages" })}
                </span>
              </button>
              <div class="ai-history-row-actions">
                <button
                  title={t("ai.rename", { default: "Rename" })}
                  onclick={() => startRename(conv.id, conv.title)}
                  aria-label={t("ai.rename", { default: "Rename" })}
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"></path><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z"></path></svg>
                </button>
                <button
                  title={t("ai.delete", { default: "Delete" })}
                  onclick={() => deleteConversation(conv.id)}
                  aria-label={t("ai.delete", { default: "Delete" })}
                >
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                </button>
              </div>
            {/if}
          </div>
        {/each}
        <button class="ai-history-new" onclick={handleNewChat}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          {t("ai.new_chat", { default: "New chat" })}
        </button>
      </div>
    {/if}

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
          <p>{t("ai.empty_title", { default: "Hi! I'm your Colima AI Assistant." })}</p>
          <p class="ai-panel-empty-hint">
            {t("ai.empty_hint", {
              default: "I can control containers, troubleshoot issues, read the knowledge bank, and manage your VMs.",
            })}
          </p>
        </div>
      {/if}

      {#each aiState.messages as msg (msg.id)}
        <div class="ai-msg {msg.role}" id="ai-msg-{msg.id}">
          {#if msg.role === "system"}
            <div class="ai-msg-system">
              {@html renderMarkdownHTML(msg.content)}
            </div>
          {:else if msg.role === "user"}
            <div class="ai-msg-label">{t("ai.role_you", { default: "You" })}</div>
            <div class="ai-msg-user">
              {@html renderMarkdownHTML(msg.content)}
            </div>
          {:else}
            <div class="ai-msg-label">{t("ai.role_assistant", { default: "Colima AI" })}</div>
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
      {#if queuedMessage}
        <div class="ai-queued-chip">
          <span class="ai-queued-chip-label">{t("ai.queued", { default: "Will send when done" })}</span>
          <span class="ai-queued-chip-text">{queuedMessage}</span>
          <button
            onclick={() => queuedMessage = ""}
            title={t("ai.cancel_queued", { default: "Cancel queued message" })}
            aria-label={t("ai.cancel_queued", { default: "Cancel queued message" })}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
          </button>
        </div>
      {/if}
      <div class="ai-input-pill">
        <textarea
          bind:this={inputEl}
          bind:value={userInput}
          placeholder={t("ai.input_placeholder", { default: "Type a message…" })}
          title={t("ai.input_hint", { default: "Enter to send · Shift+Enter for a new line" })}
          bind:clientWidth={inputWidth}
          onkeydown={handleInputKeydown}
          oncompositionstart={handleCompositionStart}
          oncompositionend={handleCompositionEnd}
        ></textarea>
        {#if aiState.isProcessing}
          <button
            class="ai-send-btn ai-stop-btn"
            aria-label={t("ai.stop", { default: "Stop generating" })}
            title={t("ai.stop", { default: "Stop generating" })}
            onclick={handleStop}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none"><rect x="6" y="6" width="12" height="12" rx="2"></rect></svg>
          </button>
        {:else}
          <button
            class="ai-send-btn"
            aria-label={t("ai.send", { default: "Send message" })}
            disabled={!userInput.trim()}
            onclick={() => handleSend()}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"></line><polygon points="22 2 15 22 11 13 2 9 22 2"></polygon></svg>
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- Always rendered, faded out while the panel is open, so hiding and
     restoring the chat animates instead of popping. It is the only way back to
     a hidden panel besides the sidebar toggle — which is why hiding is now safe
     to do without settling the agent's pending work. -->
<button
  class="ai-bubble-trigger"
  class:ai-bubble-trigger-hidden={uiState.aiPanelOpen}
  onclick={showPanel}
  aria-hidden={uiState.aiPanelOpen}
  tabindex={uiState.aiPanelOpen ? -1 : 0}
  title={aiState.isProcessing
    ? t("ai.bubble_working", { default: "Colima AI is working — click to open" })
    : t("ai.bubble_open", { default: "Open Colima AI" })}
>
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon>
  </svg>
  {#if unreadCount > 0}
    <span class="ai-bubble-badge">{unreadCount > 9 ? "9+" : unreadCount}</span>
  {/if}
  {#if aiState.isProcessing}
    <span class="ai-bubble-working" aria-hidden="true"></span>
  {/if}
</button>

<style>
  /* AI Chat Panel — a docked column, not an overlay.

     It used to be `position: fixed; top: var(--header-height); right: 0`, which
     covered whatever was underneath: on any page with header actions the
     rightmost buttons were sliced in half and became unclickable. It also
     assumed every page has a header of exactly --header-height, which the
     Terminal and Dashboard do not.

     As a flex sibling of `.main-content` it takes its own column and the
     content shrinks to fit, so nothing is ever hidden behind it. `flex-shrink:
     0` keeps the user's chosen width from being squeezed by long content, and
     `min-width: 0` on the messages list lets long tokens wrap instead of
     forcing the column wider. */
  .ai-panel {
    position: relative;
    height: 100vh;
    flex-shrink: 0;
    background: var(--bg-sidebar);
    border-left: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .ai-panel-resizer {
    position: absolute;
    left: -4px;
    top: 0;
    bottom: 0;
    width: 8px;
    cursor: ew-resize;
    z-index: 10;
  }
  .ai-panel-resizer:hover {
    background: rgba(88, 166, 255, 0.2);
  }

  /* Shares the header tokens with the sidebar and the page header so all three
     bottom borders form one line. Docked as a column, a panel header even a few
     pixels off its neighbours reads as a rendering fault — and it drifted
     exactly that way while each bar carried its own hard-coded height. */
  .ai-panel-header {
    height: var(--header-height);
    padding: var(--header-pad-top) 16px var(--header-pad-bottom);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
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
  .ai-panel-actions button.active {
    background: rgba(88, 166, 255, 0.15);
    color: var(--accent-blue);
  }

  .ai-history-dropdown {
    position: absolute;
    top: calc(var(--header-height) + 8px);
    right: 12px;
    width: min(320px, calc(100% - 24px));
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    z-index: 60;
    max-height: 50vh;
    overflow-y: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ai-history-dropdown-title {
    font-size: 10px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 6px 8px;
  }
  /* Each row is a switch button plus its rename/delete actions. The actions
     stay hidden until hover so the list reads as titles, not as a toolbar. */
  .ai-history-row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius-sm);
    padding-right: 4px;
  }
  .ai-history-row:hover {
    background: var(--bg-card-hover);
  }
  .ai-history-row.active {
    background: rgba(88, 166, 255, 0.12);
  }

  .ai-history-item {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: left;
    padding: 7px 8px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-primary);
    cursor: pointer;
    font: inherit;
  }
  .ai-history-item-title {
    font-size: var(--text-xs);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ai-history-item-meta {
    font-size: 10px;
    color: var(--text-muted);
  }

  .ai-history-row-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--transition-fast);
  }
  .ai-history-row:hover .ai-history-row-actions,
  .ai-history-row:focus-within .ai-history-row-actions {
    opacity: 1;
  }
  .ai-history-row-actions button {
    background: transparent;
    border: none;
    color: var(--text-muted);
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .ai-history-row-actions button:hover {
    background: var(--bg-app);
    color: var(--text-primary);
  }

  .ai-history-rename {
    flex: 1;
    min-width: 0;
    margin: 4px;
    padding: 5px 8px;
    font: inherit;
    font-size: var(--text-xs);
    color: var(--text-primary);
    background: var(--bg-input);
    border: 1px solid var(--accent-blue);
    border-radius: var(--radius-sm);
    outline: none;
  }

  .ai-history-new {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    padding: 7px 8px;
    border: none;
    border-top: 1px solid var(--border-subtle);
    border-radius: 0 0 var(--radius-sm) var(--radius-sm);
    background: transparent;
    color: var(--accent-blue);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .ai-history-new:hover {
    background: var(--bg-card-hover);
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

  .ai-msg-label {
    font-size: 10px;
    color: var(--text-muted);
    margin-bottom: 4px;
  }
  .ai-msg.user .ai-msg-label {
    align-self: flex-end;
  }

  .ai-msg-user {
    align-self: flex-end;
    background: #1f6feb;
    color: #fff;
    padding: 10px 14px;
    border-radius: 14px 14px 4px 14px;
    font-size: var(--text-sm);
    max-width: 85%;
    line-height: 1.5;
  }

  .ai-msg-assistant {
    align-self: flex-start;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    padding: 10px 14px;
    border-radius: 14px 14px 14px 4px;
    color: var(--text-primary);
    font-size: var(--text-sm);
    max-width: 95%;
    line-height: 1.6;
  }

  .ai-msg-system {
    align-self: center;
    background: var(--bg-app);
    border: 1px solid var(--border-subtle);
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
  }

  .ai-queued-chip {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
    padding: 6px 6px 6px 10px;
    border: 1px dashed var(--border-primary);
    border-radius: var(--radius-md);
    background: var(--bg-card);
    font-size: var(--text-xs);
  }
  .ai-queued-chip-label {
    color: var(--accent-blue);
    white-space: nowrap;
  }
  .ai-queued-chip-text {
    flex: 1;
    min-width: 0;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ai-queued-chip button {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    cursor: pointer;
  }
  .ai-queued-chip button:hover {
    background: var(--bg-app);
    color: var(--text-primary);
  }

  .ai-input-pill {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    background: var(--bg-input);
    border: 1px solid var(--border-primary);
    border-radius: 22px;
    padding: 6px 6px 6px 14px;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast),
      background var(--transition-fast);
  }
  .ai-input-pill:hover {
    border-color: #484f58;
  }
  .ai-input-pill:focus-within {
    border-color: var(--accent-blue);
    box-shadow: 0 0 0 2px rgba(88, 166, 255, 0.18);
    background: var(--bg-card);
  }
  .ai-input-pill textarea {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    padding: 8px 0;
    color: var(--text-primary);
    font-size: var(--text-sm);
    resize: none;
    overflow-y: auto;
    /* Keep in step with INPUT_MIN_HEIGHT / INPUT_MAX_HEIGHT: `autoGrowTextarea`
       sets the height inline, and a lower cap here would clip the box it grew. */
    height: 36px;
    min-height: 36px;
    max-height: 140px;
    font-family: inherit;
    line-height: 1.5;
    caret-color: var(--accent-blue);
  }
  .ai-input-pill textarea::placeholder {
    color: var(--text-muted);
    opacity: 1;
  }
  .ai-input-pill textarea:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .ai-panel-empty-hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin-top: 8px;
    max-width: 34ch;
  }

  .ai-send-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: linear-gradient(135deg, #58a6ff, #1f6feb);
    color: #fff;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background var(--transition-fast), transform var(--transition-fast),
      box-shadow var(--transition-fast);
    flex-shrink: 0;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  }
  .ai-send-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, #6cb2ff, #2f7cf0);
    transform: scale(1.05);
    box-shadow: 0 2px 8px rgba(31, 111, 235, 0.4);
  }
  .ai-send-btn:active:not(:disabled) {
    transform: scale(0.92);
  }
  /* Red rather than blue: it interrupts rather than sends, and it occupies the
     exact spot the send button was in a moment ago. */
  .ai-stop-btn {
    background: linear-gradient(135deg, #f85149, #da3633);
  }
  .ai-stop-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, #ff6b63, #e5484d);
    box-shadow: 0 2px 8px rgba(218, 54, 51, 0.4);
  }

  .ai-send-btn:disabled {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    cursor: not-allowed;
    transform: none;
  }

  @keyframes pulse {
    0% { opacity: 0.6; }
    50% { opacity: 1; }
    100% { opacity: 0.6; }
  }

  /* Floating trigger — the way back to a hidden chat. */
  .ai-bubble-trigger {
    position: fixed;
    bottom: 20px;
    right: 20px;
    z-index: 9999;
    width: 48px;
    height: 48px;
    border-radius: var(--radius-full);
    background: linear-gradient(135deg, #58a6ff, #bc8cff);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 16px rgba(88, 166, 255, 0.3);
    transition: transform var(--transition-fast), box-shadow var(--transition-fast),
      opacity var(--transition-fast);
  }
  .ai-bubble-trigger:hover {
    transform: scale(1.08);
    box-shadow: 0 6px 24px rgba(88, 166, 255, 0.4);
  }
  /* Faded rather than unmounted so showing and hiding both animate. */
  .ai-bubble-trigger-hidden {
    opacity: 0;
    pointer-events: none;
    transform: scale(0.6);
  }
  .ai-bubble-trigger svg {
    width: 22px;
    height: 22px;
    color: #fff;
  }

  .ai-bubble-badge {
    position: absolute;
    top: -4px;
    right: -4px;
    min-width: 18px;
    height: 18px;
    border-radius: var(--radius-full);
    background: var(--accent-red);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 4px;
    animation: ai-badge-pop 0.3s ease;
  }

  @keyframes ai-badge-pop {
    0% { transform: scale(0); }
    70% { transform: scale(1.2); }
    100% { transform: scale(1); }
  }

  /* Expanding ring: says "still working" without a number, so it reads
     differently from the badge, which means "waiting for you". */
  .ai-bubble-working {
    position: absolute;
    inset: 0;
    border-radius: var(--radius-full);
    border: 2px solid rgba(88, 166, 255, 0.6);
    animation: ai-bubble-ring 1.6s ease-out infinite;
  }

  @keyframes ai-bubble-ring {
    0% { transform: scale(1); opacity: 0.7; }
    100% { transform: scale(1.5); opacity: 0; }
  }
</style>

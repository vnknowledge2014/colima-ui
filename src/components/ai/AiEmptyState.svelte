<script lang="ts">
  import type { StarterPrompt } from "../../lib/aiSuggestions";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    starterPrompts?: StarterPrompt[];
    isConfigured?: boolean;
    onSelectPrompt?: (prompt: string) => void;
    onInsertPrompt?: (prompt: string) => void;
    onUseOllama?: () => void;
    onOpenSettings?: () => void;
  }

  let {
    starterPrompts = [],
    isConfigured = true,
    onSelectPrompt,
    onInsertPrompt,
    onUseOllama,
    onOpenSettings,
  }: Props = $props();

  function tr(key: string, fallback: string): string {
    const res = t(key);
    return res && res !== key ? res : fallback;
  }

  function handleSelect(prompt: string) {
    if (!prompt) return;
    onSelectPrompt?.(prompt);
  }

  function handleInsert(e: MouseEvent | KeyboardEvent, prompt: string) {
    e.stopPropagation();
    if (!prompt) return;
    onInsertPrompt?.(prompt);
  }

  function handleCardKeyDown(e: KeyboardEvent, prompt: string) {
    if (e.key === "Enter" || e.key === " ") {
      if (e.shiftKey) {
        e.preventDefault();
        handleInsert(e, prompt);
      } else {
        e.preventDefault();
        handleSelect(prompt);
      }
    }
  }
</script>

<div class="ai-empty-state" role="region" aria-label="AI Assistant Welcome Screen">
  <!-- Brand & Greeting Header -->
  <div class="ai-empty-header">
    <div class="ai-empty-avatar" aria-hidden="true">
      <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2 2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z"></path>
        <path d="M5.636 5.636a2 2 0 0 1 2.828 0l1.414 1.414a2 2 0 0 1 0 2.828 2 2 0 0 1-2.828 0L5.636 7.05a2 2 0 0 1 0-2.828z"></path>
        <path d="M22 12a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2 2 2 0 0 1 2-2h2a2 2 0 0 1 2 2z"></path>
        <path d="M18.364 18.364a2 2 0 0 1-2.828 0l-1.414-1.414a2 2 0 0 1 0-2.828 2 2 0 0 1 2.828 0l1.414 1.414a2 2 0 0 1 0 2.828z"></path>
        <path d="M12 22a2 2 0 0 1-2-2v-2a2 2 0 0 1 2-2 2 2 0 0 1 2 2v2a2 2 0 0 1-2 2z"></path>
        <path d="M5.636 18.364a2 2 0 0 1 0-2.828l1.414-1.414a2 2 0 0 1 2.828 0 2 2 0 0 1 0 2.828l-1.414 1.414a2 2 0 0 1-2.828 0z"></path>
        <path d="M2 12a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2 2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"></path>
      </svg>
    </div>
    <h2 class="ai-empty-title">{t("ai.empty_title", { default: "Hi! I'm your Colima AI Assistant." })}</h2>
    <p class="ai-empty-subtitle">
      {t("ai.empty_hint", {
        default: "I can control containers, troubleshoot issues, read the knowledge bank, and manage your VMs.",
      })}
    </p>
  </div>

  <!-- Onboarding Card (when unconfigured) -->
  {#if !isConfigured}
    <div class="ai-onboarding-card" role="alert">
      <div class="ai-onboarding-header">
        <svg class="ai-onboarding-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--accent-yellow)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"></path>
          <line x1="12" y1="9" x2="12" y2="13"></line>
          <line x1="12" y1="17" x2="12.01" y2="17"></line>
        </svg>
        <span class="ai-onboarding-title">{tr("ai.onboarding_title", "AI Provider Not Configured")}</span>
      </div>
      <p class="ai-onboarding-desc">
        {tr("ai.onboarding_desc", "Connect to a local Ollama instance without API keys, or configure Claude, OpenAI, or Gemini in Settings to start.")}
      </p>
      <div class="ai-onboarding-actions">
        <button type="button" class="btn-primary-action" onclick={() => onUseOllama?.()}>
          {tr("ai.use_local_ollama", "Use Local Ollama")}
        </button>
        <button type="button" class="btn-secondary-action" onclick={() => onOpenSettings?.()}>
          {tr("ai.configure_provider", "Configure Provider")}
        </button>
      </div>
    </div>
  {/if}

  <!-- Live Context Starter Prompts -->
  {#if starterPrompts.length > 0}
    <div class="ai-starters-section">
      <div class="ai-starters-label">
        <span>{tr("ai.suggested_prompts", "Suggested Prompts")}</span>
      </div>
      <div class="ai-starters-grid">
        {#each starterPrompts as prompt (prompt.id)}
          <div
            class="ai-starter-card"
            class:is-warning={prompt.category === "warning"}
            role="button"
            tabindex="0"
            onclick={() => handleSelect(prompt.prompt)}
            onkeydown={(e) => handleCardKeyDown(e, prompt.prompt)}
            aria-label="Starter prompt: {prompt.title}"
          >
            <div class="ai-starter-icon-area">
              {#if prompt.icon === "warning"}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="8" x2="12" y2="12"></line>
                  <line x1="12" y1="16" x2="12.01" y2="16"></line>
                </svg>
              {:else if prompt.icon === "container"}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-cyan)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"></path>
                  <path d="m3.3 7 8.7 5 8.7-5"></path>
                  <path d="M12 22V12"></path>
                </svg>
              {:else if prompt.icon === "vm"}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-purple)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
                  <line x1="8" y1="21" x2="16" y2="21"></line>
                  <line x1="12" y1="17" x2="12" y2="21"></line>
                </svg>
              {:else if prompt.icon === "security"}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-yellow)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path>
                </svg>
              {:else if prompt.icon === "compose"}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="3" y="3" width="7" height="7"></rect>
                  <rect x="14" y="3" width="7" height="7"></rect>
                  <rect x="14" y="14" width="7" height="7"></rect>
                  <rect x="3" y="14" width="7" height="7"></rect>
                </svg>
              {:else if prompt.icon === "k8s"}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <polygon points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5 12 2"></polygon>
                  <circle cx="12" cy="12" r="3"></circle>
                </svg>
              {:else}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="12" r="10"></circle>
                  <polygon points="12 8 8 12 12 16 12 8"></polygon>
                </svg>
              {/if}
            </div>
            <div class="ai-starter-body">
              <span class="ai-starter-title">{prompt.title}</span>
              <span class="ai-starter-prompt-preview">{prompt.prompt}</span>
            </div>
            <button
              type="button"
              class="ai-starter-insert-btn"
              onclick={(e) => handleInsert(e, prompt.prompt)}
              title="Insert into chat input"
              aria-label="Insert '{prompt.title}' into chat input"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="17" y1="17" x2="7" y2="7"></line>
                <polyline points="17 7 7 7 7 17"></polyline>
              </svg>
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .ai-empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32px 16px 20px 16px;
    max-width: 580px;
    margin: 0 auto;
    width: 100%;
  }

  .ai-empty-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    margin-bottom: 24px;
  }

  .ai-empty-avatar {
    width: 60px;
    height: 60px;
    border-radius: var(--radius-lg);
    background: rgba(96, 165, 250, 0.08);
    border: 1px solid var(--border-accent);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 14px;
    box-shadow: var(--shadow-glow-blue);
  }

  .ai-empty-title {
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 8px 0;
  }

  .ai-empty-subtitle {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
    max-width: 440px;
  }

  .ai-onboarding-card {
    width: 100%;
    background: rgba(251, 191, 36, 0.04);
    border: 1px solid var(--accent-yellow);
    border-radius: var(--radius-md);
    padding: 14px 16px;
    margin-bottom: 24px;
    box-shadow: var(--shadow-sm);
  }

  .ai-onboarding-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .ai-onboarding-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--accent-yellow);
  }

  .ai-onboarding-desc {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    line-height: 1.4;
    margin: 0 0 12px 0;
  }

  .ai-onboarding-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .btn-primary-action {
    background: var(--accent-blue);
    color: var(--bg-app);
    border: none;
    border-radius: var(--radius-sm);
    padding: 6px 12px;
    font-size: var(--text-xs);
    font-weight: 600;
    cursor: pointer;
    transition: opacity var(--transition-fast);
  }

  .btn-primary-action:hover {
    opacity: 0.9;
  }

  .btn-secondary-action {
    background: var(--bg-card);
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    padding: 6px 12px;
    font-size: var(--text-xs);
    font-weight: 500;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }

  .btn-secondary-action:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-accent);
  }

  .ai-starters-section {
    width: 100%;
  }

  .ai-starters-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 10px;
  }

  .ai-starters-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }

  .ai-starter-card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
    outline: none;
    text-align: left;
    position: relative;
  }

  .ai-starter-card:hover {
    background: var(--surface-selected);
    border-color: var(--border-selected);
  }

  .ai-starter-card:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 1px;
  }

  .ai-starter-card.is-warning {
    border-color: rgba(248, 113, 113, 0.4);
    background: rgba(248, 113, 113, 0.04);
  }

  .ai-starter-card.is-warning:hover {
    border-color: var(--accent-red);
    background: rgba(248, 113, 113, 0.08);
  }

  .ai-starter-icon-area {
    display: flex;
    align-items: center;
    justify-content: center;
    padding-top: 2px;
    flex-shrink: 0;
  }

  .ai-starter-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-grow: 1;
    min-width: 0;
  }

  .ai-starter-title {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ai-starter-prompt-preview {
    font-size: var(--text-xs);
    color: var(--text-muted);
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .ai-starter-insert-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    padding: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast), background var(--transition-fast);
    flex-shrink: 0;
  }

  .ai-starter-insert-btn:hover {
    color: var(--accent-blue);
    background: rgba(96, 165, 250, 0.12);
  }
</style>

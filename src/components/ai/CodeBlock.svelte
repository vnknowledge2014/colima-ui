<script lang="ts">
  import { t } from "../../lib/i18n.svelte";
  import { globalToast } from "../../lib/globalToast";
  import { dashboardState, openTerminalSession } from "../../store.svelte";

  interface Props {
    code: string;
    language?: string;
    isShell?: boolean;
    onInsert?: (code: string) => void;
    class?: string;
  }

  let {
    code,
    language = "",
    isShell = false,
    onInsert,
    class: className = "",
  }: Props = $props();

  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const displayLang = $derived(language.trim() || (isShell ? "shell" : "text"));

  // Helper for locale lookup with English fallback
  function tr(key: string, fallback: string): string {
    const res = t(key);
    return res && res !== key ? res : fallback;
  }

  async function handleCopy() {
    try {
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(code);
      }
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (err) {
      console.error("Failed to copy code to clipboard:", err);
      globalToast("error", tr("ai.copy_failed", "Failed to copy to clipboard"));
    }
  }

  function handleInsert() {
    if (onInsert) {
      onInsert(code);
    }
  }

  async function handleOpenTerminal() {
    try {
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(code);
      }
    } catch (err) {
      console.error("Failed to copy command during terminal open:", err);
    }

    globalToast(
      "info",
      tr("ai.terminal_command_copied", "Command copied. Paste in terminal to run."),
      { record: false }
    );

    const instances = dashboardState.colimaInstances || [];
    const runningColima = instances.find(
      (i) => (i.status || "").toLowerCase() === "running"
    );
    const targetProfile = runningColima
      ? (runningColima.name === "colima" || runningColima.name === "default"
          ? "default"
          : runningColima.name)
      : "default";

    const target = { kind: "colima" as const, profile: targetProfile };
    openTerminalSession(target);
  }
</script>

<div class="ai-code-block {className}">
  <div class="ai-code-header">
    <div class="ai-code-header-left">
      <span class="ai-code-lang">{displayLang}</span>
      {#if isShell}
        <span class="ai-code-shell-badge">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
            <polyline points="4 17 10 11 4 5" />
            <line x1="12" y1="19" x2="20" y2="19" />
          </svg>
          shell
        </span>
      {/if}
    </div>
    <div class="ai-code-actions">
      <button
        type="button"
        class="ai-code-action-btn"
        class:copied
        onclick={handleCopy}
        title={copied ? tr("ai.copied", "Copied!") : tr("ai.copy_code", "Copy code")}
        aria-label={copied ? tr("ai.copied", "Copied!") : tr("ai.copy_code", "Copy code")}
      >
        {#if copied}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <span>{tr("ai.copied", "Copied!")}</span>
        {:else}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
          <span>{tr("ai.copy_code", "Copy")}</span>
        {/if}
      </button>

      {#if onInsert}
        <button
          type="button"
          class="ai-code-action-btn"
          onclick={handleInsert}
          title={tr("ai.insert_into_chat", "Insert into chat")}
          aria-label={tr("ai.insert_into_chat", "Insert into chat")}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <polyline points="9 10 4 15 9 20" />
            <path d="M20 4v7a4 4 0 0 1-4 4H4" />
          </svg>
          <span>{tr("ai.insert_into_chat", "Insert")}</span>
        </button>
      {/if}

      {#if isShell}
        <button
          type="button"
          class="ai-code-action-btn ai-code-term-btn"
          onclick={handleOpenTerminal}
          title={tr("ai.open_terminal", "Open Terminal (copies & switches)")}
          aria-label={tr("ai.open_terminal", "Open Terminal")}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <polyline points="4 17 10 11 4 5" />
            <line x1="12" y1="19" x2="20" y2="19" />
          </svg>
          <span>{tr("ai.open_terminal", "Terminal")}</span>
        </button>
      {/if}
    </div>
  </div>
  <pre class="ai-code-pre" tabindex="0"><code>{code}</code></pre>
</div>

<style>
  .ai-code-block {
    margin: 8px 0;
    border-radius: var(--radius-md);
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    overflow: hidden;
  }

  .ai-code-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 10px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-subtle);
  }

  .ai-code-header-left {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .ai-code-lang {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    text-transform: lowercase;
  }

  .ai-code-shell-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    font-size: 0.65rem;
    font-weight: 500;
    background: var(--surface-selected);
    color: var(--accent-blue);
    border: 1px solid var(--border-accent);
  }

  .ai-code-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .ai-code-action-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    background: transparent;
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }

  .ai-code-action-btn:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
    border-color: var(--border-accent);
  }

  .ai-code-action-btn.copied {
    color: var(--accent-green);
    border-color: color-mix(in srgb, var(--accent-green) 35%, transparent);
    background: color-mix(in srgb, var(--accent-green) 10%, transparent);
  }

  .ai-code-term-btn {
    color: var(--accent-blue);
    border-color: var(--border-selected);
  }

  .ai-code-term-btn:hover {
    background: color-mix(in srgb, var(--accent-blue) 12%, transparent);
    color: var(--accent-blue);
    border-color: var(--accent-blue);
  }

  .ai-code-pre {
    margin: 0;
    padding: 10px 12px;
    background: var(--surface-inset);
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: 1.5;
    color: var(--text-primary);
  }

  .ai-code-pre:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: -2px;
  }

  .ai-code-pre code {
    font-family: inherit;
    font-size: inherit;
    color: inherit;
    white-space: pre;
  }
</style>

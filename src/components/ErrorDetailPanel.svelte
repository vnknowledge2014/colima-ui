<script lang="ts">
  import {
    errorLogState,
    closeErrorLog,
    clearErrorLog,
    formatEntryForClipboard,
    type ErrorLogEntry,
  } from "../store/errorLog.svelte";
  import { errorTitle, errorHint } from "../lib/errors";
  import { t } from "../lib/i18n.svelte";
  import { globalToast } from "../lib/globalToast";
  import { openHelpArticle } from "../store.svelte";

  let copiedId = $state<number | null>(null);

  async function copy(entry: ErrorLogEntry) {
    try {
      await navigator.clipboard.writeText(formatEntryForClipboard(entry));
      copiedId = entry.id;
      setTimeout(() => { if (copiedId === entry.id) copiedId = null; }, 2000);
    } catch {
      // Clipboard can be unavailable (permissions, insecure context). Say so
      // rather than leaving the button looking broken.
      globalToast("info", t('errors.copy_failed', { default: 'Could not access the clipboard' }));
    }
  }

  function formatTime(ts: number): string {
    return new Date(ts).toLocaleTimeString();
  }
</script>

<!-- Escape is bound at the window rather than on the backdrop: the backdrop is
     never focused, so a handler there would only fire after a click — which
     already closes the panel. Must sit at the top level; Svelte rejects
     `<svelte:window>` inside a block. -->
<svelte:window onkeydown={(e) => { if (e.key === 'Escape' && errorLogState.isOpen) closeErrorLog(); }} />

{#if errorLogState.isOpen}
  <div class="error-panel-backdrop" aria-hidden="true" onclick={closeErrorLog}></div>

  <aside class="error-panel" aria-label={t('errors.panel_title', { default: 'Error details' })}>
    <header class="error-panel-header">
      <h2>{t('errors.panel_title', { default: 'Error details' })}</h2>
      <div class="error-panel-header-actions">
        {#if errorLogState.entries.length > 0}
          <button onclick={clearErrorLog}>{t('errors.clear', { default: 'Clear' })}</button>
        {/if}
        <button aria-label={t('common.close', { default: 'Close' })} onclick={closeErrorLog}>×</button>
      </div>
    </header>

    {#if errorLogState.entries.length === 0}
      <p class="error-panel-empty">{t('errors.none_this_session', { default: 'No errors this session.' })}</p>
    {:else}
      <ul class="error-list">
        {#each errorLogState.entries as entry (entry.id)}
          <li class="error-entry">
            <div class="error-entry-head">
              <span class="error-entry-title">
                {#if entry.action}{entry.action} — {/if}{errorTitle(entry.error)}
              </span>
              <span class="error-entry-time">{formatTime(entry.timestamp)}</span>
            </div>

            <p class="error-entry-detail">{entry.error.detail}</p>

            {#if errorHint(entry.error)}
              <p class="error-entry-hint">{errorHint(entry.error)}</p>
            {/if}

            {#if entry.error.command}
              <pre class="error-entry-command">{entry.error.command}</pre>
            {/if}

            {#if entry.error.docId}
              <button
                class="error-entry-doc"
                onclick={() => {
                  // Closing first: the panel is fixed over the page it would
                  // otherwise cover.
                  closeErrorLog();
                  openHelpArticle(entry.error.docId!);
                }}
              >
                {t('errors.how_to_fix', { default: 'How to fix this' })} →
              </button>
            {/if}

            <div class="error-entry-meta">
              <code>{entry.error.code}</code>
              {#if entry.error.exitCode !== undefined}
                <code>exit {entry.error.exitCode}</code>
              {/if}
              <button onclick={() => copy(entry)}>
                {copiedId === entry.id
                  ? t('errors.copied', { default: 'Copied' })
                  : t('errors.copy', { default: 'Copy' })}
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </aside>
{/if}

<style>
  .error-panel-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    border: none;
    padding: 0;
    z-index: 900;
  }
  .error-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(520px, 100vw);
    background: var(--bg-secondary, #161b22);
    border-left: 1px solid var(--border-color, #30363d);
    display: flex;
    flex-direction: column;
    z-index: 901;
    overflow: hidden;
  }
  .error-panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #30363d);
  }
  .error-panel-header h2 {
    font-size: 1rem;
    margin: 0;
  }
  .error-panel-header-actions {
    display: flex;
    gap: 8px;
  }
  .error-panel-header-actions button {
    background: none;
    border: none;
    color: var(--text-secondary, #8b949e);
    cursor: pointer;
    font: inherit;
  }
  .error-panel-header-actions button:hover {
    color: var(--text-primary, #e6edf3);
  }
  .error-panel-empty {
    padding: 24px 16px;
    color: var(--text-secondary, #8b949e);
  }
  .error-list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }
  .error-entry {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #30363d);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .error-entry-head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: baseline;
  }
  .error-entry-title {
    font-weight: 600;
  }
  .error-entry-time {
    font-size: 0.8em;
    color: var(--text-secondary, #8b949e);
    white-space: nowrap;
  }
  .error-entry-detail {
    margin: 0;
    font-size: 0.9em;
    word-break: break-word;
  }
  .error-entry-hint {
    margin: 0;
    font-size: 0.85em;
    color: var(--text-secondary, #8b949e);
  }
  .error-entry-command {
    margin: 0;
    padding: 6px 8px;
    background: var(--bg-primary, #0d1117);
    border-radius: 4px;
    font-size: 0.8em;
    overflow-x: auto;
  }
  .error-entry-doc {
    align-self: flex-start;
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-blue);
    cursor: pointer;
    font: inherit;
    font-size: 0.85em;
  }
  .error-entry-doc:hover {
    text-decoration: underline;
  }
  .error-entry-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8em;
    color: var(--text-secondary, #8b949e);
  }
  .error-entry-meta button {
    margin-left: auto;
    background: none;
    border: 1px solid var(--border-color, #30363d);
    border-radius: 4px;
    padding: 2px 8px;
    color: inherit;
    cursor: pointer;
    font: inherit;
  }
  .error-entry-meta button:hover {
    color: var(--text-primary, #e6edf3);
  }
</style>

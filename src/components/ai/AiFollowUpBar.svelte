<script lang="ts">
  interface Props {
    suggestions?: [string, string][] | string[][];
    disabled?: boolean;
    onSend?: (prompt: string) => void;
    onInsert?: (prompt: string) => void;
  }

  let {
    suggestions = [],
    disabled = false,
    onSend,
    onInsert,
  }: Props = $props();

  // Normalize suggestions: limit to MAX_SUGGESTIONS (3), ensure [label, prompt] format
  const normalizedSuggestions = $derived(
    (suggestions || []).slice(0, 3).map((item) => {
      const label = (item[0] || "").trim();
      const prompt = (item[1] || label).trim();
      return { label, prompt };
    }).filter((s) => s.label.length > 0)
  );

  function handleSend(prompt: string) {
    if (disabled || !prompt) return;
    onSend?.(prompt);
  }

  function handleInsert(e: MouseEvent | KeyboardEvent, prompt: string) {
    e.stopPropagation();
    if (disabled || !prompt) return;
    onInsert?.(prompt);
  }

  function handleKeyDown(e: KeyboardEvent, prompt: string) {
    if (disabled) return;
    if (e.key === "Enter" || e.key === " ") {
      if (e.shiftKey) {
        e.preventDefault();
        handleInsert(e, prompt);
      } else {
        e.preventDefault();
        handleSend(prompt);
      }
    }
  }
</script>

{#if normalizedSuggestions.length > 0}
  <div class="ai-follow-up-bar" role="toolbar" aria-label="Follow-up suggestions">
    <div class="ai-chips-scroll-container">
      {#each normalizedSuggestions as chip, i (`${i}-${chip.label}`)}
        <div class="ai-chip-wrapper" class:disabled>
          <button
            type="button"
            class="ai-chip-btn"
            {disabled}
            onclick={() => handleSend(chip.prompt)}
            onkeydown={(e) => handleKeyDown(e, chip.prompt)}
            title="Send: {chip.prompt}"
            aria-label="Send suggestion: {chip.label}"
          >
            <span class="ai-chip-label">{chip.label}</span>
          </button>
          <button
            type="button"
            class="ai-chip-insert-btn"
            {disabled}
            onclick={(e) => handleInsert(e, chip.prompt)}
            title="Insert into chat input (Shift+Click)"
            aria-label="Insert '{chip.label}' into chat composer"
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="17" y1="17" x2="7" y2="7"></line>
              <polyline points="17 7 7 7 7 17"></polyline>
            </svg>
          </button>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .ai-follow-up-bar {
    width: 100%;
    margin-top: 8px;
    margin-bottom: 4px;
  }

  .ai-chips-scroll-container {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 2px 2px 6px 2px;
    scrollbar-width: none;
    -ms-overflow-style: none;
    scroll-snap-type: x mandatory;
    -webkit-overflow-scrolling: touch;
  }

  .ai-chips-scroll-container::-webkit-scrollbar {
    display: none;
  }

  .ai-chip-wrapper {
    display: inline-flex;
    align-items: center;
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-full);
    transition: background var(--transition-fast), border-color var(--transition-fast);
    flex-shrink: 0;
    scroll-snap-align: start;
    box-shadow: var(--shadow-sm);
  }

  .ai-chip-wrapper:hover:not(.disabled) {
    background: var(--surface-selected);
    border-color: var(--border-selected);
  }

  .ai-chip-wrapper.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .ai-chip-btn {
    background: transparent;
    border: none;
    color: var(--text-primary);
    font-size: var(--text-xs);
    font-family: var(--font-sans);
    padding: 5px 8px 5px 12px;
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
    border-top-left-radius: var(--radius-full);
    border-bottom-left-radius: var(--radius-full);
    outline: none;
  }

  .ai-chip-btn:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 1px;
  }

  .ai-chip-btn:disabled {
    cursor: not-allowed;
  }

  .ai-chip-label {
    display: inline-block;
  }

  .ai-chip-insert-btn {
    background: transparent;
    border: none;
    border-left: 1px solid var(--border-subtle);
    color: var(--text-muted);
    padding: 5px 10px 5px 7px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-top-right-radius: var(--radius-full);
    border-bottom-right-radius: var(--radius-full);
    transition: color var(--transition-fast), background var(--transition-fast);
    outline: none;
  }

  .ai-chip-insert-btn:hover:not(:disabled) {
    color: var(--accent-blue);
    background: rgba(96, 165, 250, 0.12);
  }

  .ai-chip-insert-btn:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 1px;
  }

  .ai-chip-insert-btn:disabled {
    cursor: not-allowed;
  }
</style>

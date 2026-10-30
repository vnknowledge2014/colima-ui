<script lang="ts">
  import type { ParsedChoicePrompt, ChoiceOption } from "../../lib/aiToolParser";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    choice: ParsedChoicePrompt;
    isLatest?: boolean;
    onSelect: (choiceValue: string) => void;
    onInsert?: (text: string) => void;
  }

  let {
    choice,
    isLatest = true,
    onSelect,
    onInsert,
  }: Props = $props();

  let selectedValues = $state<string[]>([]);
  let focusedIndex = $state<number>(0);
  let customInput = $state<string>("");
  let showCustomInput = $state<boolean>(false);
  let submittedChoice = $state<string | null>(null);

  const isMulti = $derived(choice.multi);
  const canInteract = $derived(isLatest && submittedChoice === null);
  const totalOptions = $derived(choice.options.length);

  function tr(key: string, fallback: string, params?: Record<string, string | number>): string {
    const res = t(key);
    let str = res && res !== key ? res : fallback;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        str = str.replace(new RegExp(`{${k}}`, "g"), String(v));
      }
    }
    return str;
  }

  function isSelected(val: string): boolean {
    return selectedValues.includes(val);
  }

  function handleSingleSelect(option: ChoiceOption) {
    if (!canInteract) return;
    const value = option.value || option.label;
    submittedChoice = value;
    onSelect(value);
  }

  function handleToggleMulti(option: ChoiceOption) {
    if (!canInteract) return;
    const value = option.value || option.label;
    if (selectedValues.includes(value)) {
      selectedValues = selectedValues.filter((v) => v !== value);
    } else {
      selectedValues = [...selectedValues, value];
    }
  }

  function handleMultiSubmit() {
    if (!canInteract) return;
    const allPicks = [...selectedValues];
    if (showCustomInput && customInput.trim()) {
      allPicks.push(customInput.trim());
    }
    if (allPicks.length === 0) return;
    const summary = allPicks.join(", ");
    submittedChoice = summary;
    onSelect(summary);
  }

  function handleInsertOption(option: ChoiceOption, e: MouseEvent) {
    e.stopPropagation();
    const value = option.value || option.label;
    onInsert?.(value);
  }

  function handleInsertMulti() {
    const allPicks = [...selectedValues];
    if (showCustomInput && customInput.trim()) {
      allPicks.push(customInput.trim());
    }
    if (allPicks.length === 0) return;
    onInsert?.(allPicks.join(", "));
  }

  function handleCustomKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && customInput.trim() && canInteract) {
      e.preventDefault();
      if (isMulti) {
        handleMultiSubmit();
      } else {
        const val = customInput.trim();
        submittedChoice = val;
        onSelect(val);
      }
    }
  }

  function handleOptionsKeydown(e: KeyboardEvent) {
    if (!canInteract) return;
    if (e.key === "ArrowDown" || e.key === "ArrowRight") {
      e.preventDefault();
      focusedIndex = (focusedIndex + 1) % totalOptions;
    } else if (e.key === "ArrowUp" || e.key === "ArrowLeft") {
      e.preventDefault();
      focusedIndex = (focusedIndex - 1 + totalOptions) % totalOptions;
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const currentOpt = choice.options[focusedIndex];
      if (!currentOpt) return;
      if (isMulti) {
        handleToggleMulti(currentOpt);
      } else {
        handleSingleSelect(currentOpt);
      }
    }
  }
</script>

<div
  class="choice-card"
  class:disabled={!canInteract}
  role={isMulti ? "group" : "radiogroup"}
  aria-label={choice.question}
>
  <!-- Card Header -->
  <div class="choice-header">
    <div class="choice-icon" aria-hidden="true">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
    </div>
    <div class="choice-question">{choice.question}</div>
    {#if isMulti}
      <span class="choice-badge multi-badge">
        {tr("ai.choice_multi", "Multi-select")}
      </span>
    {/if}
    {#if !isLatest}
      <span class="choice-badge historical-badge">
        {tr("ai.choice_historical", "Previous turn")}
      </span>
    {/if}
  </div>

  <!-- Options List -->
  <div
    class="choice-options"
    tabindex="0"
    onkeydown={handleOptionsKeydown}
    role="presentation"
  >
    {#each choice.options as opt, idx (opt.value || opt.label)}
      {@const active = isMulti ? isSelected(opt.value || opt.label) : submittedChoice === (opt.value || opt.label)}
      <div
        class="choice-option-row"
        class:selected={active}
        class:focused={canInteract && focusedIndex === idx}
        role={isMulti ? "checkbox" : "radio"}
        aria-checked={active}
        aria-disabled={!canInteract}
        tabindex={canInteract && focusedIndex === idx ? 0 : -1}
        onclick={() => {
          if (isMulti) {
            handleToggleMulti(opt);
          } else {
            handleSingleSelect(opt);
          }
        }}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            if (isMulti) {
              handleToggleMulti(opt);
            } else {
              handleSingleSelect(opt);
            }
          }
        }}
      >
        <div class="choice-selection-indicator" aria-hidden="true">
          {#if isMulti}
            <div class="checkbox-box" class:checked={active}>
              {#if active}
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
              {/if}
            </div>
          {:else}
            <div class="radio-circle" class:checked={active}>
              {#if active}
                <div class="radio-inner-dot"></div>
              {/if}
            </div>
          {/if}
        </div>

        <div class="choice-label-wrap">
          <div class="choice-label">{opt.label}</div>
          {#if opt.description}
            <div class="choice-desc">{opt.description}</div>
          {/if}
        </div>

        {#if onInsert && canInteract}
          <button
            type="button"
            class="choice-insert-btn"
            title={tr("ai.insert_into_input", "Insert into prompt composer")}
            onclick={(e) => handleInsertOption(opt, e)}
            aria-label={tr("ai.insert_into_input", "Insert into prompt composer")}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="7" y1="17" x2="17" y2="7"/><polyline points="7 7 17 7 17 17"/></svg>
          </button>
        {/if}
      </div>
    {/each}
  </div>

  <!-- Custom Response Input (Expandable) -->
  {#if canInteract}
    <div class="custom-choice-wrap">
      {#if !showCustomInput}
        <button
          type="button"
          class="custom-toggle-btn"
          onclick={() => showCustomInput = true}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          <span>{tr("ai.choice_custom_other", "Type other response…")}</span>
        </button>
      {:else}
        <div class="custom-input-box">
          <input
            type="text"
            bind:value={customInput}
            placeholder={tr("ai.choice_custom_placeholder", "Type custom answer and press Enter…")}
            onkeydown={handleCustomKeydown}
          />
          {#if !isMulti}
            <button
              type="button"
              class="btn-custom-send"
              disabled={!customInput.trim()}
              onclick={() => {
                const val = customInput.trim();
                if (!val) return;
                submittedChoice = val;
                onSelect(val);
              }}
            >
              {tr("ai.send", "Send")}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Multi-select Action Bar -->
  {#if isMulti && canInteract}
    <div class="multi-actions-bar">
      <button
        type="button"
        class="choice-submit-btn"
        disabled={selectedValues.length === 0 && !customInput.trim()}
        onclick={handleMultiSubmit}
      >
        {selectedValues.length > 0
          ? tr("ai.choice_submit_count", "Submit ({count})", { count: selectedValues.length })
          : tr("ai.choice_submit", "Submit Selection")}
      </button>

      {#if onInsert}
        <button
          type="button"
          class="choice-insert-all-btn"
          disabled={selectedValues.length === 0 && !customInput.trim()}
          onclick={handleInsertMulti}
        >
          {tr("ai.insert", "Insert")}
        </button>
      {/if}
    </div>
  {/if}

  <!-- Confirmation if previously submitted -->
  {#if submittedChoice}
    <div class="choice-submitted-note">
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
      <span>{tr("ai.choice_submitted_feedback", "Selected: {choice}", { choice: submittedChoice })}</span>
    </div>
  {/if}
</div>

<style>
  .choice-card {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    margin: 8px 0;
    overflow: hidden;
    transition: opacity var(--transition-fast), border-color var(--transition-fast);
  }

  .choice-card.disabled {
    opacity: 0.68;
  }

  .choice-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-subtle);
  }

  .choice-icon {
    color: var(--accent-blue);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .choice-question {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
    flex: 1;
  }

  .choice-badge {
    font-size: var(--text-xs);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
  }

  .multi-badge {
    background: rgba(96, 165, 250, 0.12);
    color: var(--accent-blue);
    border: 1px solid rgba(96, 165, 250, 0.25);
  }

  .historical-badge {
    background: var(--surface-inset);
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
  }

  .choice-options {
    display: flex;
    flex-direction: column;
    padding: 6px;
    gap: 4px;
  }

  .choice-options:focus {
    outline: none;
  }

  .choice-option-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-inset);
    border: 1px solid transparent;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }

  .choice-option-row:hover:not([aria-disabled="true"]) {
    background: var(--bg-card-hover);
    border-color: var(--border-accent);
  }

  .choice-option-row.focused:not([aria-disabled="true"]) {
    outline: 2px solid var(--accent-blue);
    outline-offset: -1px;
  }

  .choice-option-row.selected {
    background: var(--surface-selected);
    border-color: var(--border-selected);
  }

  .choice-card.disabled .choice-option-row {
    cursor: not-allowed;
  }

  .choice-selection-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .checkbox-box {
    width: 16px;
    height: 16px;
    border-radius: 3px;
    border: 1px solid var(--border-primary);
    background: var(--bg-card);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--bg-app);
  }

  .checkbox-box.checked {
    background: var(--accent-blue);
    border-color: var(--accent-blue);
  }

  .radio-circle {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid var(--border-primary);
    background: var(--bg-card);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .radio-circle.checked {
    border-color: var(--accent-blue);
  }

  .radio-inner-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-blue);
  }

  .choice-label-wrap {
    flex: 1;
    min-width: 0;
  }

  .choice-label {
    font-size: var(--text-sm);
    color: var(--text-primary);
    font-weight: 500;
  }

  .choice-desc {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .choice-insert-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.6;
    transition: opacity var(--transition-fast), color var(--transition-fast);
  }

  .choice-insert-btn:hover {
    opacity: 1;
    color: var(--accent-blue);
    background: var(--surface-hover);
  }

  .custom-choice-wrap {
    padding: 6px 10px;
    border-top: 1px solid var(--border-subtle);
  }

  .custom-toggle-btn {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: var(--text-xs);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: var(--radius-sm);
  }

  .custom-toggle-btn:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }

  .custom-input-box {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .custom-input-box input {
    flex: 1;
    background: var(--surface-inset);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    padding: 6px 10px;
    font-size: var(--text-xs);
    color: var(--text-primary);
  }

  .custom-input-box input:focus {
    outline: 2px solid var(--accent-blue);
    border-color: transparent;
  }

  .btn-custom-send {
    background: var(--accent-blue);
    color: var(--bg-app);
    border: none;
    border-radius: var(--radius-sm);
    padding: 6px 12px;
    font-size: var(--text-xs);
    font-weight: 600;
    cursor: pointer;
  }

  .btn-custom-send:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .multi-actions-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border-subtle);
  }

  .choice-submit-btn {
    background: var(--accent-blue);
    color: var(--bg-app);
    border: none;
    border-radius: var(--radius-sm);
    padding: 6px 14px;
    font-size: var(--text-xs);
    font-weight: 600;
    cursor: pointer;
    transition: filter var(--transition-fast);
  }

  .choice-submit-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .choice-submit-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .choice-insert-all-btn {
    background: transparent;
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    padding: 6px 12px;
    font-size: var(--text-xs);
    cursor: pointer;
  }

  .choice-insert-all-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .choice-submitted-note {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: rgba(52, 211, 153, 0.08);
    border-top: 1px solid rgba(52, 211, 153, 0.2);
    color: var(--accent-green);
    font-size: var(--text-xs);
    font-weight: 500;
  }
</style>

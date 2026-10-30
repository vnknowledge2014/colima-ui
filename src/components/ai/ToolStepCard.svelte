<script lang="ts">
  import type { ToolStepPayload } from "../../lib/aiToolStep";
  import { t } from "../../lib/i18n.svelte";

  interface Props {
    step: ToolStepPayload;
    onApprove?: (id: string) => void;
    onDeny?: (id: string) => void;
  }

  let {
    step,
    onApprove,
    onDeny,
  }: Props = $props();

  let isPayloadOpen = $state(false);
  let copiedInput = $state(false);
  let copiedOutput = $state(false);

  const stepId = $derived(step.id || step.name || "tool-step");
  const isPending = $derived(step.status === "pending_approval");
  const isRunning = $derived(step.status === "running");
  const isSucceeded = $derived(step.status === "succeeded");
  const isDenied = $derived(step.status === "denied");
  const isFailed = $derived(step.status === "failed");

  // Format rawInput as pretty JSON if valid
  const formattedInput = $derived.by(() => {
    if (!step.rawInput) return "";
    try {
      const parsed = JSON.parse(step.rawInput);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return step.rawInput;
    }
  });

  const hasPayload = $derived(Boolean(step.rawInput || step.output || step.exitCode !== undefined));

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

  async function copyToClipboard(text: string, type: "input" | "output") {
    try {
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
      }
      if (type === "input") {
        copiedInput = true;
        setTimeout(() => { copiedInput = false; }, 2000);
      } else {
        copiedOutput = true;
        setTimeout(() => { copiedOutput = false; }, 2000);
      }
    } catch (err) {
      console.error("Failed to copy text:", err);
    }
  }

  function handleApprove() {
    onApprove?.(stepId);
  }

  function handleDeny() {
    onDeny?.(stepId);
  }
</script>

<div
  class="tool-step-card status-{step.status} safety-{step.safety.toLowerCase()}"
  role="region"
  aria-label={tr("ai.tool_step_label", "Tool execution step: {title}", { title: step.title })}
>
  <!-- Card Header -->
  <div class="tool-step-header">
    <div class="tool-step-kind-icon" aria-hidden="true">
      {#if step.kind === "query"}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      {:else if step.kind === "event"}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
      {:else if step.kind === "command"}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
      {:else if step.kind === "diagnose"}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
      {:else if step.kind === "security"}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
      {:else if step.kind === "navigate"}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76"/></svg>
      {:else}
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
      {/if}
    </div>

    <div class="tool-step-title-wrap">
      <div class="tool-step-title">{step.title}</div>
      <div class="tool-step-name">{step.name}</div>
    </div>

    <div class="tool-step-badges">
      <!-- Safety Badge -->
      <span class="tool-badge safety-badge safety-{step.safety.toLowerCase()}">
        {step.safety}
      </span>

      <!-- Status Badge -->
      <span class="tool-badge status-badge status-{step.status}" aria-live="polite">
        {#if isRunning}
          <svg class="spin" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10" stroke-dasharray="32" stroke-dashoffset="12"/></svg>
          {tr("ai.status_running", "Running…")}
        {:else if isPending}
          <span class="pulse-dot"></span>
          {tr("ai.status_pending", "Pending Approval")}
        {:else if isSucceeded}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
          {tr("ai.status_completed", "Completed")}
        {:else if isDenied}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          {tr("ai.status_denied", "Denied")}
        {:else if isFailed}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          {tr("ai.status_failed", "Failed")}
        {/if}
      </span>
    </div>
  </div>

  <!-- Warning Banner -->
  {#if step.warning}
    <div class="tool-step-warning" role="alert">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
      <span>{step.warning}</span>
    </div>
  {/if}

  <!-- Action Area: Approve / Deny buttons when pending approval -->
  {#if isPending}
    <div class="tool-step-actions">
      <button
        type="button"
        class="tool-btn btn-approve"
        onclick={handleApprove}
        aria-label={tr("ai.approve_action", "Approve {title}", { title: step.title })}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
        <span>{tr("ai.approve", "Approve")}</span>
      </button>

      <button
        type="button"
        class="tool-btn btn-deny"
        onclick={handleDeny}
        aria-label={tr("ai.deny_action", "Deny {title}", { title: step.title })}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        <span>{tr("ai.deny", "Deny")}</span>
      </button>
    </div>
  {/if}

  <!-- Collapsible Payload Section -->
  {#if hasPayload}
    <div class="tool-step-payload-section">
      <button
        type="button"
        class="tool-payload-toggle"
        onclick={() => isPayloadOpen = !isPayloadOpen}
        aria-expanded={isPayloadOpen}
      >
        <svg
          class="chevron"
          class:open={isPayloadOpen}
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
        >
          <polyline points="6 9 12 15 18 9"/>
        </svg>
        <span>{tr("ai.tool_details", "Parameters & Output")}</span>
        {#if step.exitCode !== undefined}
          <span class="exit-code-badge" class:error={step.exitCode !== 0}>
            exit {step.exitCode}
          </span>
        {/if}
      </button>

      {#if isPayloadOpen}
        <div class="tool-payload-body">
          {#if formattedInput}
            <div class="payload-block">
              <div class="payload-header">
                <span>{tr("ai.input_params", "Input Parameters")}</span>
                <button
                  type="button"
                  class="copy-btn"
                  onclick={() => copyToClipboard(formattedInput, "input")}
                  title={copiedInput ? tr("ai.copied", "Copied!") : tr("ai.copy", "Copy")}
                >
                  {#if copiedInput}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                    <span style="color: var(--accent-green);">{tr("ai.copied", "Copied!")}</span>
                  {:else}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                    <span>{tr("ai.copy", "Copy")}</span>
                  {/if}
                </button>
              </div>
              <pre tabindex="0"><code>{formattedInput}</code></pre>
            </div>
          {/if}

          {#if step.output}
            <div class="payload-block">
              <div class="payload-header">
                <span>{tr("ai.output_result", "Output")}</span>
                <button
                  type="button"
                  class="copy-btn"
                  onclick={() => copyToClipboard(step.output!, "output")}
                  title={copiedOutput ? tr("ai.copied", "Copied!") : tr("ai.copy", "Copy")}
                >
                  {#if copiedOutput}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5" aria-hidden="true"><polyline points="20 6 9 17 4 12"/></svg>
                    <span style="color: var(--accent-green);">{tr("ai.copied", "Copied!")}</span>
                  {:else}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                    <span>{tr("ai.copy", "Copy")}</span>
                  {/if}
                </button>
              </div>
              <pre tabindex="0"><code>{step.output}</code></pre>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool-step-card {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    margin: 8px 0;
    overflow: hidden;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .tool-step-card.status-pending_approval {
    border-color: rgba(251, 191, 36, 0.4);
    box-shadow: 0 0 12px rgba(251, 191, 36, 0.1);
  }

  .tool-step-card.status-pending_approval.safety-dangerous {
    border-color: rgba(248, 113, 113, 0.4);
    box-shadow: 0 0 12px rgba(248, 113, 113, 0.15);
  }

  .tool-step-card.status-running {
    border-color: rgba(96, 165, 250, 0.4);
  }

  .tool-step-card.status-succeeded {
    border-color: rgba(52, 211, 153, 0.3);
  }

  .tool-step-card.status-failed {
    border-color: rgba(248, 113, 113, 0.4);
  }

  .tool-step-card.status-denied {
    border-color: var(--border-subtle);
    opacity: 0.85;
  }

  .tool-step-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: var(--bg-secondary);
  }

  .tool-step-kind-icon {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    background: var(--surface-inset);
    color: var(--accent-blue);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .tool-step-title-wrap {
    flex: 1;
    min-width: 0;
  }

  .tool-step-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tool-step-name {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .tool-step-badges {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .tool-badge {
    font-size: var(--text-xs);
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .safety-badge.safety-safe {
    background: rgba(52, 211, 153, 0.12);
    color: var(--accent-green);
    border: 1px solid rgba(52, 211, 153, 0.25);
  }

  .safety-badge.safety-normal {
    background: rgba(96, 165, 250, 0.12);
    color: var(--accent-blue);
    border: 1px solid rgba(96, 165, 250, 0.25);
  }

  .safety-badge.safety-dangerous {
    background: rgba(248, 113, 113, 0.12);
    color: var(--accent-red);
    border: 1px solid rgba(248, 113, 113, 0.25);
    font-weight: 600;
  }

  .status-badge.status-pending_approval {
    background: rgba(251, 191, 36, 0.15);
    color: var(--accent-yellow);
    border: 1px solid rgba(251, 191, 36, 0.3);
  }

  .status-badge.status-running {
    background: rgba(96, 165, 250, 0.15);
    color: var(--accent-blue);
    border: 1px solid rgba(96, 165, 250, 0.3);
  }

  .status-badge.status-succeeded {
    background: rgba(52, 211, 153, 0.15);
    color: var(--accent-green);
    border: 1px solid rgba(52, 211, 153, 0.3);
  }

  .status-badge.status-denied {
    background: rgba(90, 98, 117, 0.2);
    color: var(--text-muted);
    border: 1px solid var(--border-subtle);
  }

  .status-badge.status-failed {
    background: rgba(248, 113, 113, 0.15);
    color: var(--accent-red);
    border: 1px solid rgba(248, 113, 113, 0.3);
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent-yellow);
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.85); }
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .tool-step-warning {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    background: rgba(251, 191, 36, 0.08);
    border-left: 3px solid var(--accent-yellow);
    color: var(--text-primary);
    font-size: var(--text-xs);
  }

  .tool-step-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: var(--bg-card);
    border-top: 1px solid var(--border-subtle);
  }

  .tool-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    font-size: var(--text-xs);
    font-weight: 600;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: filter var(--transition-fast), background-color var(--transition-fast);
  }

  .tool-btn:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 2px;
  }

  .btn-approve {
    background: var(--accent-green);
    color: var(--bg-app);
    border: none;
  }

  .btn-approve:hover {
    filter: brightness(1.1);
  }

  .btn-deny {
    background: transparent;
    color: var(--text-primary);
    border: 1px solid var(--border-primary);
  }

  .btn-deny:hover {
    background: rgba(248, 113, 113, 0.1);
    border-color: var(--accent-red);
    color: var(--accent-red);
  }

  .tool-step-payload-section {
    border-top: 1px solid var(--border-subtle);
  }

  .tool-payload-toggle {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: var(--text-xs);
    cursor: pointer;
    text-align: left;
  }

  .tool-payload-toggle:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }

  .tool-payload-toggle:focus-visible {
    outline: 2px solid var(--accent-blue);
  }

  .chevron {
    transition: transform var(--transition-fast);
  }

  .chevron.open {
    transform: rotate(180deg);
  }

  .exit-code-badge {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 0.65rem;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: rgba(52, 211, 153, 0.15);
    color: var(--accent-green);
  }

  .exit-code-badge.error {
    background: rgba(248, 113, 113, 0.15);
    color: var(--accent-red);
  }

  .tool-payload-body {
    padding: 8px 14px 12px;
    background: var(--surface-inset);
    border-top: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .payload-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .payload-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    font-weight: 500;
  }

  .copy-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: var(--text-xs);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
  }

  .copy-btn:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }

  pre {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-primary);
    overflow-x: auto;
    max-height: 200px;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>

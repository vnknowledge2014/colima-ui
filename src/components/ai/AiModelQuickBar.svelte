<script lang="ts">
  import { t } from "../../lib/i18n.svelte";
  import { dashboardState } from "../../store.svelte";
  import { aiApi } from "../../lib/api";
  import {
    AI_PROVIDERS,
    findProvider,
    getActiveProvider,
    getProviderField,
    activateProvider,
    setProviderField,
  } from "../../lib/aiProviderConfig";

  interface Props {
    activeProviderId?: string;
    activeModel?: string;
    liveContextEnabled?: boolean;
    currentPage?: string;
    onSelectModel?: (provider: string, model: string) => void;
    onToggleLiveContext?: (enabled: boolean) => void;
    onOpenSettings?: () => void;
    class?: string;
  }

  let {
    activeProviderId = "",
    activeModel = "",
    liveContextEnabled = true,
    currentPage = "dashboard",
    onSelectModel,
    onToggleLiveContext,
    onOpenSettings,
    class: className = "",
  }: Props = $props();

  function tr(key: string, fallback: string, params?: Record<string, string | number>): string {
    const res = t(key, params);
    return res && res !== key ? res : fallback;
  }

  // Reactive provider & model state
  let popoverOpen = $state(false);
  let selectedProvider = $state(getActiveProvider());
  let modelInput = $state(getProviderField(getActiveProvider(), "model"));
  let availableModels = $state<string[]>([]);
  let isFetchingModels = $state(false);
  let fetchError = $state<string | null>(null);

  let popoverEl = $state<HTMLDivElement>();
  let pillBtnEl = $state<HTMLButtonElement>();

  // Curated popular presets per provider for instant 1-click selection
  const POPULAR_PRESETS: Record<string, string[]> = {
    "ollama-local": ["llama3.2", "qwen2.5-coder:7b", "deepseek-r1:8b", "mistral", "phi3"],
    "anthropic": ["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022", "claude-3-opus-20240229"],
    "openai": ["gpt-4o", "gpt-4o-mini", "o1", "o3-mini"],
    "gemini": ["gemini-1.5-pro", "gemini-1.5-flash", "gemini-2.0-flash"],
    "openai-compatible": ["openai/gpt-4o", "meta-llama/llama-3.1-70b", "deepseek/deepseek-r1"],
    "ollama-cloud": ["gpt-oss:120b", "llama3.3:70b"],
  };

  const effectiveProvider = $derived(activeProviderId || selectedProvider || getActiveProvider());
  const providerSpec = $derived(findProvider(effectiveProvider));
  const selectedProviderSpec = $derived(findProvider(selectedProvider));
  const effectiveModel = $derived(activeModel || getProviderField(effectiveProvider, "model") || "default");

  const isConfigured = $derived.by(() => {
    if (!providerSpec) return false;
    const rawModel = (activeModel || getProviderField(effectiveProvider, "model")).trim();
    if (!rawModel) return false;
    const endpoint = getProviderField(effectiveProvider, "endpoint").trim();
    if (providerSpec.needsEndpoint && !endpoint) return false;
    const keyRequired = providerSpec.needsKey && !(providerSpec.needsEndpoint && Boolean(endpoint));
    if (keyRequired) {
      const key = getProviderField(effectiveProvider, "api_key").trim();
      return Boolean(key);
    }
    return true;
  });

  const runningVm = $derived.by(() => {
    const instances = dashboardState.colimaInstances || [];
    return instances.find((i) => (i.status || "").toLowerCase() === "running");
  });

  const displayPageName = $derived.by(() => {
    const raw = (currentPage || "dashboard").trim();
    return raw.charAt(0).toUpperCase() + raw.slice(1);
  });

  const currentPresets = $derived(POPULAR_PRESETS[selectedProvider] || []);

  // Track previous prop values to avoid circular reactivity locks
  let prevProvider = $state("");
  let prevModel = $state("");

  // Sync internal state if external props change
  $effect(() => {
    if (activeProviderId && activeProviderId !== prevProvider) {
      prevProvider = activeProviderId;
      selectedProvider = activeProviderId;
    }
  });

  $effect(() => {
    if (activeModel && activeModel !== prevModel) {
      prevModel = activeModel;
      modelInput = activeModel;
    }
  });

  // Handle outside click and Escape key dismissal
  $effect(() => {
    if (!popoverOpen) return;
    function handleOutsideClick(e: MouseEvent) {
      const target = e.target as Node;
      if (
        popoverEl &&
        !popoverEl.contains(target) &&
        pillBtnEl &&
        !pillBtnEl.contains(target)
      ) {
        popoverOpen = false;
      }
    }
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        popoverOpen = false;
        pillBtnEl?.focus();
      }
    }
    window.addEventListener("mousedown", handleOutsideClick);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleOutsideClick);
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  function togglePopover() {
    popoverOpen = !popoverOpen;
    if (popoverOpen) {
      selectedProvider = effectiveProvider;
      modelInput = effectiveModel;
      fetchError = null;
    }
  }

  function handleSelectProviderTab(provId: string) {
    selectedProvider = provId;
    modelInput = getProviderField(provId, "model") || POPULAR_PRESETS[provId]?.[0] || "";
    availableModels = [];
    fetchError = null;
  }

  function applyModelChoice(targetProvider: string, targetModel: string) {
    const trimmed = targetModel.trim();
    if (!trimmed) return;

    if (targetProvider !== getActiveProvider()) {
      activateProvider(targetProvider);
    }
    setProviderField(targetProvider, "model", trimmed);

    onSelectModel?.(targetProvider, trimmed);
    popoverOpen = false;
  }

  async function handleFetchModels() {
    if (isFetchingModels) return;
    isFetchingModels = true;
    fetchError = null;
    try {
      const apiKey = getProviderField(selectedProvider, "api_key");
      const endpoint = getProviderField(selectedProvider, "endpoint");
      const raw = await aiApi.listModels(selectedProvider, apiKey, endpoint);
      const parsed: string[] = JSON.parse(typeof raw === "string" ? raw : "[]");
      availableModels = [...new Set(parsed)];
      if (availableModels.length === 0) {
        fetchError = selectedProviderSpec?.needsKey && !apiKey
          ? tr("ai.quickbar_key_missing", "API key not configured")
          : tr("ai.quickbar_no_models", "No models returned");
      }
    } catch (e: unknown) {
      availableModels = [];
      fetchError = e instanceof Error ? e.message : String(e);
    } finally {
      isFetchingModels = false;
    }
  }

  function handleToggleLiveContext() {
    onToggleLiveContext?.(!liveContextEnabled);
  }

  function handleSettingsClick() {
    popoverOpen = false;
    onOpenSettings?.();
  }
</script>

<div
  class="ai-model-quick-bar {className}"
  role="toolbar"
  aria-label={tr("ai.quickbar_title", "Model & Context Bar")}
>
  <!-- Left Side: Provider & Model Pill -->
  <div class="ai-quickbar-left">
    <button
      bind:this={pillBtnEl}
      type="button"
      class="ai-model-pill"
      class:open={popoverOpen}
      onclick={togglePopover}
      aria-expanded={popoverOpen}
      aria-haspopup="dialog"
      title={tr("ai.quickbar_provider_model", "Provider & Model: {provider} / {model}", {
        provider: providerSpec?.label || effectiveProvider,
        model: effectiveModel,
      })}
    >
      <span
        class="ai-status-dot"
        class:connected={isConfigured}
        class:warning={!isConfigured}
        aria-hidden="true"
      ></span>
      <span class="ai-provider-badge">{providerSpec?.label.split(" ")[0] || effectiveProvider}</span>
      <span class="ai-model-label">{effectiveModel}</span>
      <svg
        class="ai-chevron-icon"
        class:rotated={popoverOpen}
        width="11"
        height="11"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <polyline points="6 9 12 15 18 9"></polyline>
      </svg>
    </button>
  </div>

  <!-- Right Side: Live Context Toggle, Active VM Status, Settings Shortcut -->
  <div class="ai-quickbar-right">
    <!-- Live UI Context Toggle Chip -->
    <button
      type="button"
      class="ai-context-chip"
      class:active={liveContextEnabled}
      onclick={handleToggleLiveContext}
      aria-pressed={liveContextEnabled}
      title={liveContextEnabled
        ? tr("ai.quickbar_live_context_enabled", "Live UI context active: current page state included in prompts (Click to disable)")
        : tr("ai.quickbar_live_context_disabled", "Live UI context disabled: click to include current page state in prompts")}
    >
      <svg
        class="ai-pin-icon"
        width="11"
        height="11"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"></path>
        <circle cx="12" cy="10" r="3"></circle>
      </svg>
      <span class="ai-context-text">{displayPageName}</span>
    </button>

    <!-- Active VM Status Indicator -->
    <div
      class="ai-vm-indicator"
      title={runningVm
        ? tr("ai.quickbar_vm_status", "Colima VM: {name} ({status})", { name: runningVm.name, status: runningVm.status })
        : tr("ai.quickbar_vm_none", "No active Colima VM running")}
    >
      <span class="ai-vm-dot" class:running={Boolean(runningVm)} aria-hidden="true"></span>
      <span class="ai-vm-label">{runningVm ? runningVm.name : "no vm"}</span>
    </div>

    <!-- Settings Shortcut -->
    <button
      type="button"
      class="ai-settings-btn"
      onclick={handleSettingsClick}
      title={tr("ai.quickbar_open_settings", "Open AI settings")}
      aria-label={tr("ai.quickbar_open_settings", "Open AI settings")}
    >
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3"></circle>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
      </svg>
    </button>
  </div>

  <!-- Provider & Model Selection Popover -->
  {#if popoverOpen}
    <div
      bind:this={popoverEl}
      class="ai-model-popover"
      role="dialog"
      aria-label={tr("ai.quickbar_select_model", "Select Model")}
    >
      <!-- Popover Header -->
      <div class="ai-popover-header">
        <span class="ai-popover-title">{tr("ai.quickbar_select_provider", "Select AI Provider")}</span>
        <button
          type="button"
          class="ai-popover-close"
          onclick={() => { popoverOpen = false; }}
          aria-label="Close"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      <!-- Provider Tabs -->
      <div class="ai-provider-tabs">
        {#each AI_PROVIDERS as prov (prov.id)}
          <button
            type="button"
            class="ai-provider-tab"
            class:active={selectedProvider === prov.id}
            onclick={() => handleSelectProviderTab(prov.id)}
          >
            {prov.label.split(" ")[0]}
          </button>
        {/each}
      </div>

      <!-- API Key Status / Warning -->
      {#if selectedProviderSpec?.needsKey && !getProviderField(selectedProvider, "api_key")}
        <div class="ai-popover-warning">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--accent-yellow)" stroke-width="2">
            <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"></path>
            <line x1="12" y1="9" x2="12" y2="13"></line>
            <line x1="12" y1="17" x2="12.01" y2="17"></line>
          </svg>
          <span class="ai-warning-text">{tr("ai.quickbar_key_missing", "API key not configured")}</span>
          <button type="button" class="ai-configure-link" onclick={handleSettingsClick}>
            {tr("ai.quickbar_configure_key", "Configure in Settings")}
          </button>
        </div>
      {/if}

      <!-- Model Input & Fetch Controls -->
      <div class="ai-model-input-row">
        <input
          type="text"
          class="ai-model-input"
          bind:value={modelInput}
          placeholder={tr("ai.quickbar_model_placeholder", "Enter model name (e.g. llama3.2)...")}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              applyModelChoice(selectedProvider, modelInput);
            }
          }}
        />
        <button
          type="button"
          class="ai-fetch-btn"
          disabled={isFetchingModels}
          onclick={handleFetchModels}
          title={tr("ai.quickbar_fetch_models", "Fetch Models")}
        >
          {#if isFetchingModels}
            <span class="ai-spinner" aria-hidden="true"></span>
          {:else}
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="23 4 23 10 17 10"></polyline>
              <polyline points="1 20 1 14 7 14"></polyline>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
            </svg>
          {/if}
        </button>
      </div>

      {#if fetchError}
        <div class="ai-fetch-error">{fetchError}</div>
      {/if}

      <!-- Popular Presets -->
      {#if currentPresets.length > 0}
        <div class="ai-presets-section">
          <div class="ai-presets-header">{tr("ai.quickbar_popular_models", "Popular Presets")}</div>
          <div class="ai-presets-grid">
            {#each currentPresets as preset (preset)}
              <button
                type="button"
                class="ai-preset-chip"
                class:active={effectiveModel === preset && effectiveProvider === selectedProvider}
                onclick={() => applyModelChoice(selectedProvider, preset)}
              >
                {preset}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Fetched Models List -->
      {#if availableModels.length > 0}
        <div class="ai-fetched-section">
          <div class="ai-presets-header">{tr("ai.quickbar_available_models", "Available Models")} ({availableModels.length})</div>
          <div class="ai-fetched-list">
            {#each availableModels as modelName (modelName)}
              <button
                type="button"
                class="ai-model-option"
                class:active={effectiveModel === modelName && effectiveProvider === selectedProvider}
                onclick={() => applyModelChoice(selectedProvider, modelName)}
              >
                <span class="ai-model-opt-name">{modelName}</span>
                {#if effectiveModel === modelName && effectiveProvider === selectedProvider}
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" stroke-width="2.5">
                    <polyline points="20 6 9 17 4 12"></polyline>
                  </svg>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Popover Footer Action -->
      <div class="ai-popover-footer">
        <button
          type="button"
          class="ai-apply-btn"
          disabled={!modelInput.trim()}
          onclick={() => applyModelChoice(selectedProvider, modelInput)}
        >
          {tr("ai.apply", "Apply")}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .ai-model-quick-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 6px 12px;
    background: var(--bg-card);
    border-bottom: 1px solid var(--border-subtle);
    font-size: var(--text-xs);
    position: relative;
    z-index: 20;
    user-select: none;
  }

  .ai-quickbar-left,
  .ai-quickbar-right {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  /* Model & Provider Pill */
  .ai-model-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 7px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: var(--text-xs);
    cursor: pointer;
    transition: all var(--transition-fast);
    max-width: 170px;
    text-align: left;
  }

  .ai-model-pill:hover,
  .ai-model-pill.open {
    background: var(--bg-card-hover);
    border-color: var(--border-accent);
  }

  .ai-status-dot {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--text-muted);
    flex-shrink: 0;
  }

  .ai-status-dot.connected {
    background: var(--status-running);
    box-shadow: 0 0 6px var(--accent-green-glow);
  }

  .ai-status-dot.warning {
    background: var(--accent-yellow);
  }

  .ai-provider-badge {
    color: var(--text-muted);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .ai-model-label {
    font-family: var(--font-mono);
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    font-weight: 500;
  }

  .ai-chevron-icon {
    color: var(--text-muted);
    flex-shrink: 0;
    transition: transform var(--transition-fast);
  }

  .ai-chevron-icon.rotated {
    transform: rotate(180deg);
  }

  /* Context Toggle Chip */
  .ai-context-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 7px;
    background: transparent;
    border: 1px dashed var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font-size: var(--text-xs);
    cursor: pointer;
    transition: all var(--transition-fast);
    max-width: 110px;
  }

  .ai-context-chip:hover {
    border-color: var(--border-primary);
    color: var(--text-secondary);
  }

  .ai-context-chip.active {
    background: var(--surface-selected);
    border: 1px solid var(--border-selected);
    color: var(--accent-blue);
    font-weight: 500;
  }

  .ai-pin-icon {
    flex-shrink: 0;
  }

  .ai-context-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* VM Status Indicator */
  .ai-vm-indicator {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text-muted);
    font-size: 11px;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
  }

  .ai-vm-dot {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--status-stopped);
    flex-shrink: 0;
  }

  .ai-vm-dot.running {
    background: var(--status-running);
  }

  .ai-vm-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 65px;
  }

  /* Settings Button */
  .ai-settings-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .ai-settings-btn:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-primary);
    color: var(--text-primary);
  }

  /* Popover Dialog */
  .ai-model-popover {
    position: absolute;
    top: calc(100% + 4px);
    left: 8px;
    width: 310px;
    max-width: calc(100% - 16px);
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    z-index: 50;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ai-popover-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .ai-popover-title {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .ai-popover-close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px;
    border-radius: var(--radius-sm);
  }

  .ai-popover-close:hover {
    color: var(--text-primary);
  }

  /* Provider Tabs */
  .ai-provider-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }

  .ai-provider-tab {
    padding: 3px 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .ai-provider-tab:hover {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }

  .ai-provider-tab.active {
    background: var(--surface-selected);
    border-color: var(--border-selected);
    color: var(--accent-blue);
    font-weight: 500;
  }

  /* Warning Banner */
  .ai-popover-warning {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    background: var(--surface-selected);
    border: 1px solid var(--accent-yellow-glow);
    border-radius: var(--radius-sm);
    font-size: 11px;
  }

  .ai-warning-text {
    color: var(--accent-yellow);
    flex: 1;
  }

  .ai-configure-link {
    background: none;
    border: none;
    color: var(--accent-blue);
    text-decoration: underline;
    font-size: 11px;
    cursor: pointer;
    padding: 0;
  }

  /* Model Input & Fetch */
  .ai-model-input-row {
    display: flex;
    gap: 4px;
  }

  .ai-model-input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    background: var(--bg-input);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .ai-model-input:focus {
    outline: none;
    border-color: var(--border-accent);
  }

  .ai-fetch-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .ai-fetch-btn:hover:not(:disabled) {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }

  .ai-fetch-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .ai-spinner {
    width: 12px;
    height: 12px;
    border: 1.5px solid var(--border-primary);
    border-top-color: var(--accent-blue);
    border-radius: var(--radius-full);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .ai-fetch-error {
    font-size: 11px;
    color: var(--accent-red);
    padding: 2px 4px;
  }

  /* Presets */
  .ai-presets-section,
  .ai-fetched-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ai-presets-header {
    font-size: 10px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .ai-presets-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .ai-preset-chip {
    padding: 3px 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .ai-preset-chip:hover {
    background: var(--bg-card-hover);
    color: var(--text-primary);
  }

  .ai-preset-chip.active {
    background: var(--surface-selected);
    border-color: var(--border-selected);
    color: var(--accent-green);
    font-weight: 500;
  }

  /* Fetched List */
  .ai-fetched-list {
    max-height: 140px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: var(--radius-sm);
    padding: 2px;
  }

  .ai-model-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 11px;
    cursor: pointer;
    text-align: left;
  }

  .ai-model-option:hover {
    background: var(--bg-card-hover);
  }

  .ai-model-option.active {
    color: var(--accent-green);
    font-weight: 500;
  }

  .ai-model-opt-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-popover-footer {
    display: flex;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .ai-apply-btn {
    padding: 4px 10px;
    background: var(--accent-blue);
    border: none;
    border-radius: var(--radius-sm);
    color: var(--bg-app);
    font-size: var(--text-xs);
    font-weight: 500;
    cursor: pointer;
    transition: opacity var(--transition-fast);
  }

  .ai-apply-btn:hover:not(:disabled) {
    opacity: 0.9;
  }

  .ai-apply-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Narrow Layout Breakpoint (< 340px) */
  @media (max-width: 340px) {
    .ai-provider-badge {
      display: none;
    }
    .ai-vm-label {
      display: none;
    }
    .ai-model-pill {
      max-width: 120px;
    }
  }
</style>
